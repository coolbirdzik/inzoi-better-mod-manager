mod portable;
pub use portable::apply_from_cli;
mod credentials;
mod curseforge;
mod download_catalogue;
mod download_storage;
mod folder_opener;
mod library;
mod metadata;
mod mod_updates;
mod nexus;
mod thumbnails;
mod update_check_log;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use walkdir::WalkDir;

type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Serialize)]
pub struct SourceFolder {
    path: String,
    scan_subfolders: bool,
}
impl From<String> for SourceFolder {
    fn from(path: String) -> Self {
        Self {
            path,
            scan_subfolders: true,
        }
    }
}
impl<'de> Deserialize<'de> for SourceFolder {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum SavedSource {
            Legacy(String),
            Folder {
                path: String,
                #[serde(default = "default_scan_subfolders")]
                scan_subfolders: bool,
            },
        }
        Ok(match SavedSource::deserialize(deserializer)? {
            SavedSource::Legacy(path) => path.into(),
            SavedSource::Folder {
                path,
                scan_subfolders,
            } => Self {
                path,
                scan_subfolders,
            },
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(deserialize_with = "deserialize_folder_path")]
    mods_dir: String,
    library_dir: String,
    #[serde(default = "default_scan_subfolders")]
    scan_subfolders: bool,
    #[serde(default)]
    source_dirs: Vec<SourceFolder>,
    #[serde(default)]
    downloads_dir: String,
}
fn deserialize_folder_path<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    let path = String::deserialize(deserializer)?;
    Ok(if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path
    })
}
fn default_scan_subfolders() -> bool {
    true
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Mod {
    id: String,
    name: String,
    author: String,
    version: String,
    description: String,
    path: String,
    thumbnail: Option<String>,
    managed: bool,
    enabled: bool,
    source: String,
    project: String,
    #[serde(default)]
    deployed_path: Option<String>,
    #[serde(default)]
    library_root: Option<String>,
    #[serde(default)]
    metadata: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    archive: bool,
    #[serde(default)]
    previous_version: Option<String>,
    #[serde(default)]
    superseded_by: Option<String>,
    #[serde(default)]
    added_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Store {
    settings: Settings,
    mods: Vec<Mod>,
    #[serde(default)]
    thumbnails: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    tags: std::collections::BTreeMap<String, Vec<String>>,
}
struct State {
    file: PathBuf,
    store: Mutex<Store>,
    pending: Mutex<Vec<NexusLink>>,
    requests: Mutex<std::collections::HashMap<String, nexus::Request>>,
    mod_updates: Mutex<std::collections::HashMap<String, mod_updates::Plan>>,
}
#[derive(Serialize)]
struct Snapshot {
    settings: Settings,
    mods: Vec<Mod>,
    warnings: Vec<String>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn write_store(state: &State, s: &Store) -> Result<()> {
    let temp = state.file.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec_pretty(s).map_err(err)?).map_err(err)?;
    fs::rename(temp, &state.file).map_err(err)
}
fn safe_project(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && !s.chars().any(|c| "<>:\"/\\|?*".contains(c))
        && !s.ends_with(['.', ' '])
}
fn value<'a>(v: &'a serde_json::Value, k: &str) -> Option<&'a serde_json::Value> {
    v.as_object()?
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(k))
        .map(|(_, v)| v)
}
fn field(v: &serde_json::Value, k: &str) -> String {
    value(v, k).and_then(|x| x.as_str()).unwrap_or("").into()
}
fn manifest_value(bytes: &[u8]) -> Result<serde_json::Value> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        let bytes = &bytes[2..];
        if bytes.len() % 2 != 0 {
            return Err("Truncated UTF-16 manifest".into());
        }
        let units = bytes.chunks_exact(2).map(|b| {
            if little {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        });
        let text: std::result::Result<String, _> = char::decode_utf16(units).collect();
        return serde_json::from_str(&text.map_err(err)?).map_err(err);
    }
    serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)).map_err(err)
}
fn enable_manifest(folder: &Path) -> Result<()> {
    let path = folder.join("mod_manifest.json");
    let mut v = manifest_value(&fs::read(&path).map_err(err)?)?;
    let key = v
        .as_object()
        .ok_or("Manifest must be an object")?
        .keys()
        .find(|k| k.eq_ignore_ascii_case("bEnable"))
        .cloned()
        .unwrap_or("bEnable".into());
    v[key] = serde_json::Value::Bool(true);
    fs::write(path, serde_json::to_vec_pretty(&v).map_err(err)?).map_err(err)
}
// Creation time is stable when activation or metadata edits touch the folder.
// Platforms without birth times fall back to the existing modification time.
fn path_added_at(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()))
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn read_mod(folder: &Path, id: String, managed: bool) -> Result<Mod> {
    let manifest = folder.join("mod_manifest.json");
    let v: serde_json::Value = manifest_value(&fs::read(manifest).map_err(err)?)?;
    let project = field(&v, "ProjectName");
    if !safe_project(&project) {
        return Err("Invalid ProjectName in manifest".into());
    }
    let thumbnail = [
        "thumbnail.png",
        "Thumbnail.png",
        "thumbnail.jpg",
        "preview.png",
        "icon.png",
    ]
    .iter()
    .map(|n| folder.join(n))
    .find(|p| p.is_file())
    .map(|p| p.to_string_lossy().into_owned());
    let mut m = Mod {
        id,
        name: field(&v, "FriendlyName"),
        author: field(&v, "Author"),
        version: field(&v, "Version"),
        description: field(&v, "Description"),
        path: folder.to_string_lossy().into_owned(),
        thumbnail,
        managed,
        enabled: !managed
            && value(&v, "bEnable")
                .and_then(|x| x.as_bool())
                .unwrap_or(true),
        source: field(&v, "Url"),
        project,
        deployed_path: None,
        library_root: None,
        metadata: metadata::read(folder, v),
        tags: vec![],
        archive: false,
        previous_version: None,
        superseded_by: None,
        added_at: path_added_at(folder),
    };
    if m.thumbnail.is_none() {
        m.thumbnail = thumbnails::from_nexus(&m.metadata)
            .or_else(|| thumbnails::from_curseforge(&m.metadata));
    }
    Ok(m)
}
#[cfg(test)]
fn scan(root: &Path) -> (Vec<Mod>, Vec<String>) {
    scan_with_options(root, true)
}
fn scan_with_options(root: &Path, recursive: bool) -> (Vec<Mod>, Vec<String>) {
    let mut mods = vec![];
    let mut warnings = vec![];
    if !root.exists() {
        return (
            mods,
            vec![format!("Mod folder does not exist: {}", root.display())],
        );
    }
    for e in WalkDir::new(root)
        .follow_links(false)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || (entry.file_name() != ".trash"
                    && !entry.file_name().to_string_lossy().starts_with(".inzoimm-"))
        })
    {
        match e {
            Ok(e) if e.file_type().is_file() && e.file_name() == "mod_manifest.json" => {
                let p = e.path().parent().unwrap();
                match read_mod(p, p.to_string_lossy().into_owned(), false) {
                    Ok(m) => mods.push(m),
                    Err(e) => warnings.push(format!("{}: {}", p.display(), e)),
                }
            }
            Err(e) => warnings.push(e.to_string()),
            _ => {}
        }
    }
    mods.sort_by_key(|m| m.name.to_lowercase());
    (mods, warnings)
}
fn index_managed_folder(store: &mut Store) -> Vec<String> {
    let library = Path::new(&store.settings.library_dir);
    if !library.exists() {
        return vec![];
    }
    let root = match library.canonicalize() {
        Ok(root) => root,
        Err(e) => return vec![e.to_string()],
    };
    if let Ok(game) = Path::new(&store.settings.mods_dir).canonicalize() {
        if root.starts_with(&game) || game.starts_with(&root) {
            return vec!["Managed mods folder and game Mods folder must be separate".into()];
        }
    }
    let (candidates, mut warnings) = scan_with_options(&root, store.settings.scan_subfolders);
    let known: std::collections::HashSet<_> = store
        .mods
        .iter()
        .filter_map(|m| Path::new(&m.path).canonicalize().ok())
        .collect();
    for mut candidate in candidates {
        let path = match Path::new(&candidate.path).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                warnings.push(e.to_string());
                continue;
            }
        };
        if known.contains(&path) {
            continue;
        }
        if !path.starts_with(&root) {
            warnings.push(format!(
                "Mod path is outside the managed folder: {}",
                path.display()
            ));
            continue;
        }
        if store
            .mods
            .iter()
            .any(|m| m.project.eq_ignore_ascii_case(&candidate.project))
        {
            warnings.push(format!(
                "Duplicate managed project skipped: {} ({})",
                candidate.project,
                path.display()
            ));
            continue;
        }
        candidate.id = uuid::Uuid::new_v4().to_string();
        candidate.managed = true;
        candidate.enabled = false;
        candidate.library_root = Some(root.to_string_lossy().into_owned());
        store.mods.push(candidate);
    }
    warnings
}
#[tauri::command(async)]
fn snapshot(app: tauri::AppHandle, state: tauri::State<State>) -> Result<Snapshot> {
    let mut s = state.store.lock().map_err(err)?;
    let old_mods = s.mods.clone();
    let managed_warnings = index_managed_folder(&mut s);
    if s.mods.len() != old_mods.len() {
        if let Err(error) = write_store(&state, &s) {
            s.mods = old_mods;
            return Err(error);
        }
    }
    let (mut mods, mut warnings) = library::catalogue(&s);
    warnings.extend(managed_warnings);
    if let Ok(local) = app.path().local_data_dir() {
        let metadata = local.join("inZOI/User_data/88849/library.json");
        if metadata.is_file() {
            match thumbnails::from_game_library(&metadata) {
                Ok(images) => thumbnails::apply(&mut mods, &images),
                Err(error) => warnings.push(format!("Game thumbnail metadata: {error}")),
            }
            if let Ok(details) = metadata::from_game_library(&metadata) {
                for m in &mut mods {
                    if let Some(details) = details.get(&m.project.to_lowercase()) {
                        m.metadata.insert("Game library".into(), details.clone());
                    }
                }
            }
        }
    }
    for root in [&s.settings.mods_dir, &s.settings.library_dir] {
        let _ = app.asset_protocol_scope().allow_directory(root, true);
    }
    let _ = app
        .asset_protocol_scope()
        .allow_directory(nexus::downloads_root(&s.settings), true);
    for root in &s.settings.source_dirs {
        let _ = app.asset_protocol_scope().allow_directory(&root.path, true);
    }
    for root in s.mods.iter().filter_map(|m| m.library_root.as_ref()) {
        let _ = app.asset_protocol_scope().allow_directory(root, true);
    }
    let thumbnail_dir = state.file.parent().ok_or("Missing app data folder")?;
    for m in &mut mods {
        if let Some(relative) = s.thumbnails.get(&m.project.to_lowercase()) {
            let path = thumbnail_dir.join(relative);
            if path.is_file() {
                let _ = app.asset_protocol_scope().allow_file(&path);
                m.thumbnail = Some(path.to_string_lossy().into_owned());
            }
        }
    }
    Ok(Snapshot {
        settings: s.settings.clone(),
        mods,
        warnings,
    })
}
fn set_mod_thumbnail_core(id: String, path: String, state: &State) -> Result<()> {
    let mut s = state.store.lock().map_err(err)?;
    let project = s
        .mods
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.project.clone())
        .or_else(|| {
            library::scan_sources(&s.settings)
                .0
                .into_iter()
                .find(|m| m.id == id)
                .map(|m| m.project)
        })
        .ok_or("Mod no longer exists")?;
    let source = Path::new(&path);
    let extension = source
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
        return Err("Choose a PNG, JPEG or WebP image".into());
    }
    let metadata = fs::metadata(source).map_err(err)?;
    if !metadata.is_file() || metadata.len() > 20 * 1024 * 1024 {
        return Err("Thumbnail must be an image file smaller than 20 MB".into());
    }
    let dir = state.file.parent().ok_or("Missing app data folder")?;
    fs::create_dir_all(dir.join("thumbnails")).map_err(err)?;
    let relative = format!("thumbnails/{}.{}", uuid::Uuid::new_v4(), extension);
    let target = dir.join(&relative);
    fs::copy(source, &target).map_err(err)?;
    let key = project.to_lowercase();
    let previous = s.thumbnails.insert(key.clone(), relative);
    if let Err(e) = write_store(state, &s) {
        if let Some(previous) = previous {
            s.thumbnails.insert(key, previous);
        } else {
            s.thumbnails.remove(&key);
        }
        let _ = fs::remove_file(target);
        return Err(e);
    }
    if let Some(previous) = previous {
        let _ = fs::remove_file(dir.join(previous));
    }
    Ok(())
}
#[tauri::command(async)]
fn set_mod_thumbnail(id: String, path: String, state: tauri::State<State>) -> Result<()> {
    set_mod_thumbnail_core(id, path, &state)
}
#[tauri::command(async)]
fn save_settings(settings: Settings, state: tauri::State<State>) -> Result<()> {
    save_settings_core(settings, &state)
}
fn save_settings_core(settings: Settings, state: &State) -> Result<()> {
    library::validate_sources(&settings)?;
    if !Path::new(&settings.mods_dir).is_absolute()
        || !Path::new(&settings.library_dir).is_absolute()
    {
        return Err("Use absolute folder paths".into());
    }
    let mut s = state.store.lock().map_err(err)?;
    let library = PathBuf::from(&settings.library_dir);
    let game = PathBuf::from(&settings.mods_dir);
    let resolved_library = library.canonicalize().unwrap_or(library);
    let resolved_game = game.canonicalize().unwrap_or(game);
    if resolved_library.starts_with(&resolved_game) || resolved_game.starts_with(&resolved_library)
    {
        return Err("Managed mods folder and game Mods folder must be separate".into());
    }
    if s.mods.iter().any(|m| m.enabled) && settings.mods_dir != s.settings.mods_dir {
        return Err("Disable managed mods before changing the game mod folder".into());
    }
    let old = s.settings.clone();
    let old_mods = s.mods.clone();
    if s.settings.library_dir != settings.library_dir {
        let previous_root = s.settings.library_dir.clone();
        for m in &mut s.mods {
            if m.library_root.is_none() {
                m.library_root = Some(previous_root.clone());
            }
        }
    }
    if s.settings.mods_dir != settings.mods_dir {
        for m in &mut s.mods {
            m.deployed_path = None;
        }
    }
    s.settings = settings;
    index_managed_folder(&mut s);
    if let Err(e) = write_store(&state, &s) {
        s.settings = old;
        s.mods = old_mods;
        return Err(e);
    }
    Ok(())
}
fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    for e in WalkDir::new(src).follow_links(false) {
        let e = e.map_err(err)?;
        if e.file_type().is_symlink() {
            return Err("Symbolic links are not accepted in imported mods".into());
        }
        let rel = e.path().strip_prefix(src).map_err(err)?;
        let out = dest.join(rel);
        if e.file_type().is_dir() {
            fs::create_dir_all(out).map_err(err)?;
        } else {
            fs::copy(e.path(), out).map_err(err)?;
        }
    }
    Ok(())
}
#[cfg(test)]
fn import_folder_core(path: String, state: &State) -> Result<()> {
    import_folder_with_options(path, true, state)
}
fn import_folder_with_options(path: String, recursive: bool, state: &State) -> Result<()> {
    import_folder_to_library(path, recursive, state, None)
}
fn import_folder_to_library(
    path: String,
    recursive: bool,
    state: &State,
    destination: Option<(PathBuf, PathBuf)>,
) -> Result<()> {
    let src = PathBuf::from(&path).canonicalize().map_err(err)?;
    if !src.is_dir() {
        return Err("Choose a folder containing mod_manifest.json".into());
    }
    let (mods, _) = scan_with_options(&src, recursive);
    if mods.is_empty() {
        return Err(
            "No inZOI mod_manifest.json found. Legacy PAK mods are not yet supported.".into(),
        );
    }
    let mut unique = std::collections::HashSet::new();
    if mods
        .iter()
        .any(|m| !unique.insert(m.project.to_lowercase()))
    {
        return Err("The import contains duplicate ProjectName values".into());
    }
    let mut s = state.store.lock().map_err(err)?;
    if destination.is_none()
        && mods.iter().any(|m| {
            s.mods
                .iter()
                .any(|x| x.project.eq_ignore_ascii_case(&m.project))
        })
    {
        return Err("One of these projects already exists in your library".into());
    }
    let library = destination
        .as_ref()
        .map(|(folder, _)| folder.clone())
        .unwrap_or_else(|| PathBuf::from(&s.settings.library_dir));
    let managed_root = destination
        .as_ref()
        .map(|(_, root)| root.canonicalize().map_err(err))
        .transpose()?;
    fs::create_dir_all(&library).map_err(err)?;
    let library = library.canonicalize().map_err(err)?;
    if managed_root
        .as_ref()
        .is_some_and(|root| !library.starts_with(root))
    {
        return Err("Download folder containment check failed".into());
    }
    if library.starts_with(&src) {
        return Err("Import source must not contain the library".into());
    }
    let mut planned = vec![];
    for m in mods {
        let id = uuid::Uuid::new_v4().to_string();
        let target = library.join(if destination.is_some() {
            &m.project
        } else {
            &id
        });
        if destination.is_some() {
            if let Some(known) = s
                .mods
                .iter()
                .find(|known| library::same_path(&known.path, &target.to_string_lossy()))
            {
                let current = read_mod(&target, known.id.clone(), true)?;
                if current.project.eq_ignore_ascii_case(&known.project) {
                    continue;
                }
                return Err(
                    "Existing managed folder has a different ProjectName; it was kept".into(),
                );
            }
        }
        if target.exists() {
            return Err(format!(
                "A folder already exists at {}; it was not overwritten",
                target.display()
            ));
        }
        planned.push((m, id, target));
    }
    if planned.is_empty() {
        return Ok(());
    }
    let mut added = vec![];
    for (m, id, target) in planned {
        if let Err(error) = fs::create_dir(&target) {
            for m in &added {
                let m: &Mod = m;
                let _ = fs::remove_dir_all(&m.path);
            }
            return Err(err(error));
        }
        let prepared =
            copy_tree(Path::new(&m.path), &target).and_then(|_| read_mod(&target, id, true));
        let mut m = match prepared {
            Ok(m) => m,
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                for m in &added {
                    let m: &Mod = m;
                    let _ = fs::remove_dir_all(&m.path);
                }
                return Err(error);
            }
        };
        if let Some(root) = &managed_root {
            m.library_root = Some(root.to_string_lossy().into_owned());
        }
        added.push(m);
    }
    let old = s.mods.clone();
    s.mods.extend(added.clone());
    if let Err(e) = write_store(&state, &s) {
        s.mods = old;
        for m in added {
            let _ = fs::remove_dir_all(m.path);
        }
        return Err(e);
    }
    Ok(())
}
#[tauri::command(async)]
fn edit_mod(
    id: String,
    name: String,
    description: String,
    state: tauri::State<State>,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err("Name must not be empty".into());
    }
    let mut s = state.store.lock().map_err(err)?;
    let old = s.mods.clone();
    let m = s
        .mods
        .iter_mut()
        .find(|m| m.id == id)
        .ok_or("Only imported mods can be edited")?;
    m.name = name;
    m.description = description;
    if let Err(e) = write_store(&state, &s) {
        s.mods = old;
        return Err(e);
    }
    Ok(())
}
fn toggle_mod_core(id: String, state: &State) -> Result<()> {
    let enabled = {
        let s = state.store.lock().map_err(err)?;
        s.mods
            .iter()
            .find(|m| m.id == id)
            .ok_or("Mod no longer exists")?
            .enabled
    };
    set_managed_enabled(id, !enabled, state)
}

// Resolve catalogue entries at the time of the action, never trust a UI path.
// Game mods get a library backup first; attached source folders stay in place.
fn manage_catalogue_mod(id: String, state: &State) -> Result<String> {
    let mut s = state.store.lock().map_err(err)?;
    if s.mods.iter().any(|m| m.id == id) {
        return Ok(id);
    }
    let mut m = library::catalogue(&s)
        .0
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("Mod no longer exists")?;
    if m.archive {
        return Err("Extract and import this archive before enabling it".into());
    }
    let source = Path::new(&m.path).canonicalize().map_err(err)?;
    if Path::new(&s.settings.mods_dir)
        .canonicalize()
        .is_ok_and(|game| source.starts_with(game))
    {
        drop(s);
        adopt_mod_core(m.path.clone(), state)?;
        let s = state.store.lock().map_err(err)?;
        return s
            .mods
            .iter()
            .find(|known| {
                known
                    .deployed_path
                    .as_ref()
                    .is_some_and(|path| library::same_path(path, &m.path))
            })
            .map(|known| known.id.clone())
            .ok_or("Could not register game mod".into());
    }
    let root = s
        .settings
        .source_dirs
        .iter()
        .map(|source| PathBuf::from(&source.path))
        .chain(std::iter::once(nexus::downloads_root(&s.settings)))
        .filter_map(|root| root.canonicalize().ok())
        .filter(|root| source.starts_with(root))
        .max_by_key(|root| root.components().count())
        .ok_or("Mod is outside library sources")?;
    let root = if root == source {
        root.parent().ok_or("Missing source parent")?.to_path_buf()
    } else {
        root
    };
    m.managed = true;
    m.id = uuid::Uuid::new_v4().to_string();
    let managed_id = m.id.clone();
    m.enabled = false;
    m.library_root = Some(root.to_string_lossy().into_owned());
    s.mods.push(m);
    if let Err(error) = write_store(state, &s) {
        s.mods.pop();
        return Err(error);
    }
    Ok(managed_id)
}

fn set_mod_enabled_core(id: String, enabled: bool, state: &State) -> Result<String> {
    let id = manage_catalogue_mod(id, state)?;
    set_managed_enabled(id.clone(), enabled, state)?;
    Ok(id)
}

fn set_managed_enabled(id: String, enabled: bool, state: &State) -> Result<()> {
    let mut s = state.store.lock().map_err(err)?;
    let pos = s
        .mods
        .iter()
        .position(|m| m.id == id)
        .ok_or("Import this external mod before managing it")?;
    let m = s.mods[pos].clone();
    if enabled
        && library::catalogue(&s).0.iter().any(|other| {
            other.id != m.id && other.enabled && other.project.eq_ignore_ascii_case(&m.project)
        })
    {
        return Err(
            "Another version of this project is enabled. Disable it before enabling this one."
                .into(),
        );
    }
    let target = deployment_target(&s.settings, &m)?;
    let marker = target.join(".inzoimm-owner");
    if !enabled {
        if !target.exists() {
            if !m.enabled {
                return Ok(());
            }
            s.mods[pos].enabled = false;
            if let Err(error) = write_store(state, &s) {
                s.mods[pos].enabled = m.enabled;
                return Err(error);
            }
            return Ok(());
        }
        if fs::read_to_string(&marker).map_err(err)? != m.id {
            return Err("Ownership marker mismatch; refusing to touch game files".into());
        }
        let root = PathBuf::from(&s.settings.mods_dir)
            .canonicalize()
            .map_err(err)?;
        let trash = root
            .parent()
            .ok_or("Mod folder has no parent")?
            .join(".inzoimm-disabled");
        fs::create_dir_all(&trash).map_err(err)?;
        let backup = trash.join(uuid::Uuid::new_v4().to_string());
        fs::rename(&target, &backup).map_err(err)?;
        s.mods[pos].enabled = false;
        if let Err(e) = write_store(&state, &s) {
            let _ = fs::rename(backup, target);
            s.mods[pos].enabled = m.enabled;
            return Err(e);
        }
    } else {
        if target.exists()
            && fs::read_to_string(target.join(".inzoimm-owner"))
                .ok()
                .as_deref()
                == Some(m.id.as_str())
        {
            let file = target.join("mod_manifest.json");
            let old = fs::read(&file).map_err(err)?;
            enable_manifest(&target)?;
            s.mods[pos].enabled = true;
            if let Err(e) = write_store(state, &s) {
                s.mods[pos].enabled = m.enabled;
                let _ = fs::write(file, old);
                return Err(e);
            }
            return Ok(());
        }
        if target.exists() {
            return Err(
                "A mod with this ProjectName already exists in the game. It was not overwritten."
                    .into(),
            );
        }
        fs::create_dir_all(target.parent().ok_or("Missing target parent")?).map_err(err)?;
        let root = PathBuf::from(&s.settings.mods_dir)
            .canonicalize()
            .map_err(err)?;
        let staging = root
            .parent()
            .ok_or("Mod folder has no parent")?
            .join(".inzoimm-staging");
        fs::create_dir_all(&staging).map_err(err)?;
        let temp = staging.join(uuid::Uuid::new_v4().to_string());
        let prepared = (|| {
            copy_tree(Path::new(&m.path), &temp)?;
            enable_manifest(&temp)?;
            fs::write(temp.join(".inzoimm-owner"), &m.id).map_err(err)?;
            fs::rename(&temp, &target).map_err(err)
        })();
        if let Err(e) = prepared {
            let _ = fs::remove_dir_all(&temp);
            return Err(e);
        }
        s.mods[pos].enabled = true;
        if let Err(e) = write_store(&state, &s) {
            s.mods[pos].enabled = m.enabled;
            let _ = fs::rename(&target, &temp);
            return Err(e);
        }
    }
    Ok(())
}
fn remove_mod_core(id: String, state: &State) -> Result<()> {
    {
        let s = state.store.lock().map_err(err)?;
        if let Some(m) = library::catalogue(&s)
            .0
            .into_iter()
            .find(|m| m.id == id && m.archive)
        {
            let source = Path::new(&m.path).canonicalize().map_err(err)?;
            let root = Path::new(
                m.library_root
                    .as_ref()
                    .ok_or("Missing archive library root")?,
            )
            .canonicalize()
            .map_err(err)?;
            if !source.starts_with(&root) || source == root || !source.is_file() {
                return Err("Archive containment check failed".into());
            }
            let sidecar =
                metadata::download_sidecar(source.parent().ok_or("Missing archive parent")?);
            let trash = root.join(".trash").join(uuid::Uuid::new_v4().to_string());
            fs::create_dir_all(&trash).map_err(err)?;
            let dest = trash.join(source.file_name().ok_or("Missing archive filename")?);
            fs::rename(&source, &dest).map_err(err)?;
            if let Err(error) = fs::rename(
                &sidecar,
                trash.join(sidecar.file_name().ok_or("Missing download record name")?),
            ) {
                let _ = fs::rename(&dest, &source);
                return Err(err(error));
            }
            return Ok(());
        }
    }
    let id = manage_catalogue_mod(id, state)?;
    let mut s = state.store.lock().map_err(err)?;
    let pos = s
        .mods
        .iter()
        .position(|m| m.id == id)
        .ok_or("External mods are not removed by this command")?;
    let m = s.mods[pos].clone();
    let source = PathBuf::from(&m.path).canonicalize().map_err(err)?;
    let root = PathBuf::from(m.library_root.as_ref().unwrap_or(&s.settings.library_dir))
        .canonicalize()
        .map_err(err)?;
    let indexed = m.library_root.is_some();
    if !source.starts_with(&root)
        || source == root
        || (!indexed
            && (source.parent() != Some(root.as_path())
                || source.file_name().and_then(|s| s.to_str()) != Some(m.id.as_str())))
    {
        return Err("Library containment check failed".into());
    }
    if s.mods.iter().any(|other| {
        other.id != m.id
            && Path::new(&other.path)
                .canonicalize()
                .is_ok_and(|path| path.starts_with(&source))
    }) {
        return Err("This folder contains another managed mod; remove the nested mod first".into());
    }
    let trash = root.join(".trash");
    fs::create_dir_all(&trash).map_err(err)?;
    let deployed = deployment_target(&s.settings, &m)?;
    let mut deployed_backup = None;
    if deployed.exists()
        && (m.enabled
            || m.deployed_path.is_some()
            || fs::read_to_string(deployed.join(".inzoimm-owner"))
                .ok()
                .as_deref()
                == Some(m.id.as_str()))
    {
        if fs::read_to_string(deployed.join(".inzoimm-owner"))
            .ok()
            .as_deref()
            != Some(m.id.as_str())
        {
            return Err(
                "A deployment without matching ownership exists; removal was cancelled".into(),
            );
        }
        let game_root = PathBuf::from(&s.settings.mods_dir)
            .canonicalize()
            .map_err(err)?;
        let recovery = game_root
            .parent()
            .ok_or("Missing Mods parent")?
            .join(".inzoimm-disabled");
        fs::create_dir_all(&recovery).map_err(err)?;
        let backup = recovery.join(uuid::Uuid::new_v4().to_string());
        fs::rename(&deployed, &backup).map_err(err)?;
        deployed_backup = Some((deployed, backup));
    }
    let dest = trash.join(uuid::Uuid::new_v4().to_string());
    if let Err(e) = fs::rename(&source, &dest) {
        if let Some((original, backup)) = &deployed_backup {
            let _ = fs::rename(backup, original);
        }
        return Err(err(e));
    }
    s.mods.remove(pos);
    if let Err(e) = write_store(&state, &s) {
        s.mods.insert(pos, m);
        let _ = fs::rename(dest, source);
        if let Some((original, backup)) = deployed_backup {
            let _ = fs::rename(backup, original);
        }
        return Err(e);
    }
    Ok(())
}

fn deployment_target(settings: &Settings, m: &Mod) -> Result<PathBuf> {
    if !safe_project(&m.project) {
        return Err("Invalid mod project".into());
    }
    let root = PathBuf::from(&settings.mods_dir);
    fs::create_dir_all(&root).map_err(err)?;
    let root = root.canonicalize().map_err(err)?;
    let target = m
        .deployed_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(&m.project));
    let parent = target.parent().ok_or("Invalid deployment target")?;
    let resolved = parent.canonicalize().map_err(err)?;
    if !resolved.starts_with(&root) {
        return Err("Deployment target is outside the configured Mods folder".into());
    }
    if target.exists() && !target.canonicalize().map_err(err)?.starts_with(&root) {
        return Err("Deployment target resolves outside Mods".into());
    }
    Ok(target)
}
#[tauri::command(async)]
fn import_folder(
    path: String,
    scan_subfolders: Option<bool>,
    state: tauri::State<State>,
) -> Result<()> {
    import_folder_with_options(path, scan_subfolders.unwrap_or(true), &state)
}
#[tauri::command(async)]
fn toggle_mod(id: String, state: tauri::State<State>) -> Result<()> {
    toggle_mod_core(id, &state)
}
#[tauri::command(async)]
fn set_mod_enabled(id: String, enabled: bool, state: tauri::State<State>) -> Result<String> {
    set_mod_enabled_core(id, enabled, &state)
}
#[tauri::command(async)]
fn remove_mod(id: String, state: tauri::State<State>) -> Result<()> {
    remove_mod_core(id, &state)
}
fn adopt_mod_core(path: String, state: &State) -> Result<()> {
    let mut s = state.store.lock().map_err(err)?;
    let source = PathBuf::from(path).canonicalize().map_err(err)?;
    let root = PathBuf::from(&s.settings.mods_dir)
        .canonicalize()
        .map_err(err)?;
    if !source.starts_with(&root) || source == root {
        return Err("Choose a mod inside the configured Mods folder".into());
    }
    if source.join(".inzoimm-owner").exists() {
        return Err("This mod already has an ownership marker".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut m = read_mod(&source, id.clone(), false)?;
    m.managed = true;
    let library = PathBuf::from(&s.settings.library_dir);
    fs::create_dir_all(&library).map_err(err)?;
    let library = library.canonicalize().map_err(err)?;
    if library.starts_with(&root) || root.starts_with(&library) {
        return Err("Library and game Mods folders must be separate".into());
    }
    let dest = library.join(&id);
    if let Err(e) = copy_tree(&source, &dest) {
        let _ = fs::remove_dir_all(&dest);
        return Err(e);
    }
    m.path = dest.to_string_lossy().into_owned();
    m.thumbnail = m.thumbnail.map(|x| {
        dest.join(Path::new(&x).file_name().unwrap())
            .to_string_lossy()
            .into_owned()
    });
    m.deployed_path = Some(source.to_string_lossy().into_owned());
    if let Err(e) = fs::write(source.join(".inzoimm-owner"), &id) {
        let _ = fs::remove_dir_all(&dest);
        return Err(err(e));
    }
    s.mods.push(m);
    if let Err(e) = write_store(state, &s) {
        s.mods.pop();
        let _ = fs::remove_file(source.join(".inzoimm-owner"));
        let _ = fs::remove_dir_all(dest);
        return Err(e);
    }
    Ok(())
}
#[tauri::command(async)]
fn adopt_mod(path: String, state: tauri::State<State>) -> Result<()> {
    adopt_mod_core(path, &state)
}
fn extract_zip(source: &Path, dest: &Path) -> Result<()> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(fs::File::open(source).map_err(err)?).map_err(err)?;
    if archive.len() > 20000 {
        return Err("Archive contains too many files (limit 20,000)".into());
    }
    let mut total = 0u64;
    const LIMIT: u64 = 8 * 1024 * 1024 * 1024;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(err)?;
        let rel = entry
            .enclosed_name()
            .ok_or("Archive contains an unsafe path")?
            .to_path_buf();
        if rel.components().any(|x| match x {
            std::path::Component::Normal(v) => v.to_str().map(|x| !safe_project(x)).unwrap_or(true),
            _ => true,
        }) {
            return Err("Archive contains an invalid file name".into());
        }
        if entry
            .unix_mode()
            .map(|m| m & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            return Err("Archive symbolic links are not supported".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Archive is too large")?;
        if total > LIMIT {
            return Err("Archive exceeds the 8 GiB extraction limit".into());
        }
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(out).map_err(err)?;
            continue;
        }
        fs::create_dir_all(out.parent().ok_or("Invalid entry path")?).map_err(err)?;
        let declared = entry.size();
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(out)
            .map_err(err)?;
        let actual =
            std::io::copy(&mut entry.by_ref().take(declared + 1), &mut file).map_err(err)?;
        if actual != declared {
            return Err("Archive entry size mismatch".into());
        }
    }
    Ok(())
}
#[tauri::command(async)]
fn import_zip(path: String, state: tauri::State<State>) -> Result<()> {
    import_zip_core(&path, &state)
}
fn import_zip_core(path: &str, state: &State) -> Result<()> {
    import_archive(path, state, None)
}
#[tauri::command(async)]
fn import_download(path: String, state: tauri::State<State>) -> Result<()> {
    import_download_core(&path, &state)
}
fn import_download_core(path: &str, state: &State) -> Result<()> {
    let archive = Path::new(path).canonicalize().map_err(err)?;
    let destination = metadata::download_destination(&archive)?;
    import_archive(&archive.to_string_lossy(), state, Some(destination))
}
fn import_archive(
    path: &str,
    state: &State,
    destination: Option<(PathBuf, PathBuf)>,
) -> Result<()> {
    let temp = state
        .file
        .parent()
        .ok_or("Missing app data folder")?
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&temp).map_err(err)?;
    let result = extract_zip(Path::new(path), &temp)
        .and_then(|_| metadata::attach_download(Path::new(path), &temp))
        .and_then(|_| {
            import_folder_to_library(
                temp.to_string_lossy().into_owned(),
                true,
                state,
                destination,
            )
        });
    let cleanup = fs::remove_dir_all(&temp).map_err(err);
    result.and(cleanup)
}
#[derive(Clone, Serialize, Deserialize)]
struct NexusLink {
    #[serde(default = "nexus_provider")]
    provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slug: Option<String>,
    request_id: String,
    game: String,
    mod_id: u64,
    file_id: u64,
    url: String,
}
fn nexus_provider() -> String {
    "nexus".into()
}
fn parse_link(raw: &str) -> Result<NexusLink> {
    let u = url::Url::parse(raw).map_err(err)?;
    if u.scheme() != "nxm" || u.host_str() != Some("inzoi") {
        return Err("Only nxm://inzoi links are supported".into());
    }
    let parts: Vec<_> = u.path_segments().ok_or("Missing link path")?.collect();
    if parts.len() != 4 || parts[0] != "mods" || parts[2] != "files" {
        return Err("Invalid Nexus mod link".into());
    }
    Ok(NexusLink {
        provider: "nexus".into(),
        slug: None,
        request_id: uuid::Uuid::new_v4().to_string(),
        game: "inzoi".into(),
        mod_id: parts[1].parse().map_err(err)?,
        file_id: parts[3].parse().map_err(err)?,
        url: format!(
            "https://www.nexusmods.com/inzoi/mods/{}?tab=files&file_id={}",
            parts[1], parts[3]
        ),
    })
}
fn queue_link(app: &tauri::AppHandle, raw: &str) {
    if let Ok(request) = nexus::parse_request(raw) {
        let info = request.info.clone();
        if let Some(state) = app.try_state::<State>() {
            if let Ok(mut requests) = state.requests.lock() {
                requests.insert(info.request_id.clone(), request);
            }
            if let Ok(mut queue) = state.pending.lock() {
                queue.push(info.clone());
            }
        }
        let _ = app.emit("nexus-link", info);
    }
}
#[tauri::command]
fn pending_links(state: tauri::State<State>) -> Result<Vec<NexusLink>> {
    Ok(std::mem::take(&mut *state.pending.lock().map_err(err)?))
}
#[tauri::command]
fn parse_nxm(link: String, state: tauri::State<State>) -> Result<NexusLink> {
    let request = nexus::parse_request(&link)?;
    let info = request.info.clone();
    state
        .requests
        .lock()
        .map_err(err)?
        .insert(info.request_id.clone(), request);
    Ok(info)
}
#[tauri::command]
fn register_nxm(app: tauri::AppHandle) -> Result<()> {
    app.deep_link().register("nxm").map_err(err)
}
pub fn run() {
    nexus::configure_tls();
    let mut context = tauri::generate_context!();
    if let Some(root) = portable::root() {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hasher);
        context.config_mut().identifier = format!(
            "com.coolbirdzik.inzoi-better-mod-manager.portable-{:x}",
            hasher.finish()
        );
        context.config_mut().app.app_directories_override = Some(
            tauri::utils::config::AppDirectoriesOverride::Root(root.join("data")),
        );
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            for raw in args {
                queue_link(app, &raw);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let file = dir.join("library.json");
            let mut store: Store = if file.exists() {
                serde_json::from_slice(&fs::read(&file)?)?
            } else {
                let docs = app.path().document_dir()?;
                Store {
                    settings: Settings {
                        mods_dir: docs.join("inZOI").join("Mods").to_string_lossy().into_owned(),
                        library_dir: dir.join("library").to_string_lossy().into_owned(),
                        scan_subfolders: true,
                        source_dirs: vec![],
                        downloads_dir: String::new(),
                    },
                    mods: vec![],
                    thumbnails: Default::default(),
                    tags: Default::default(),
                }
            };
            if portable::root().is_some() {
                let location = dir.join("portable-location.json");
                if let Ok(bytes) = fs::read(&location) {
                    let previous: PathBuf = serde_json::from_slice(&bytes)?;
                    if portable::relocate_library(&mut store, &previous, &dir) {
                        let temp = file.with_extension("tmp");
                        fs::write(&temp, serde_json::to_vec_pretty(&store)?)?;
                        fs::rename(temp, &file)?;
                    }
                }
                fs::write(location, serde_json::to_vec(&dir)?)?;
            }
            let initial: Vec<_> = std::env::args()
                .filter_map(|s| nexus::parse_request(&s).ok())
                .collect();
            let pending = initial.iter().map(|r| r.info.clone()).collect();
            let requests = initial
                .into_iter()
                .map(|r| (r.info.request_id.clone(), r))
                .collect();
            app.manage(State {
                file,
                store: Mutex::new(store),
                pending: Mutex::new(pending),
                requests: Mutex::new(requests),
                mod_updates: Mutex::new(Default::default()),
            });
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    queue_link(&handle, url.as_str());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            folder_opener::open_folder,
            set_mod_thumbnail,
            library::set_mod_tags,
            mod_updates::check_mod_update,
            update_check_log::read_mod_check_log,
            mod_updates::prepare_mod_update,
            mod_updates::install_mod_update,
            mod_updates::restore_previous_mod,
            save_settings,
            import_folder,
            import_zip,
            import_download,
            adopt_mod,
            edit_mod,
            toggle_mod,
            set_mod_enabled,
            remove_mod,
            parse_nxm,
            pending_links,
            register_nxm,
            nexus::download_nxm,
            nexus::cancel_nxm,
            credentials::load_nexus_api_key,
            credentials::save_nexus_api_key,
            credentials::load_curseforge_api_key,
            credentials::save_curseforge_api_key,
            portable::is_portable,
            portable::update_portable
        ])
        .run(context)
        .expect("Unable to start inZOI Better Mod Manager");
}
#[cfg(test)]
mod managed_folder_tests {
    use super::*;

    fn mod_folder(path: &Path, project: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("mod_manifest.json"),
            serde_json::to_vec(&serde_json::json!({"ProjectName":project})).unwrap(),
        )
        .unwrap();
        fs::write(path.join("payload.pak"), b"original-payload").unwrap();
    }

    #[test]
    fn selected_managed_folder_scans_in_place_with_persistent_ids_and_recoverable_removal() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let library = root.join("selected");
        let source = library.join("category/deep/HairLibrary");
        mod_folder(&source, "HairLibrary");
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.library_dir = library.to_string_lossy().into_owned();
        settings.scan_subfolders = false;
        save_settings_core(settings.clone(), &state).unwrap();
        assert!(state.store.lock().unwrap().mods.is_empty());
        settings.scan_subfolders = true;
        save_settings_core(settings.clone(), &state).unwrap();
        let first = state.store.lock().unwrap().mods[0].clone();
        assert!(first.managed);
        assert!(!first.enabled);
        assert_eq!(
            Path::new(&first.path).canonicalize().unwrap(),
            source.canonicalize().unwrap()
        );
        assert_eq!(fs::read_dir(&library).unwrap().count(), 1);
        save_settings_core(settings, &state).unwrap();
        assert_eq!(state.store.lock().unwrap().mods.len(), 1);
        assert_eq!(state.store.lock().unwrap().mods[0].id, first.id);
        toggle_mod_core(first.id.clone(), &state).unwrap();
        toggle_mod_core(first.id.clone(), &state).unwrap();
        assert_eq!(
            fs::read(source.join("payload.pak")).unwrap(),
            b"original-payload"
        );
        remove_mod_core(first.id, &state).unwrap();
        assert!(!source.exists());
        let trash = fs::read_dir(library.join(".trash"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(trash.join("payload.pak")).unwrap(),
            b"original-payload"
        );
        let mut store = state.store.lock().unwrap();
        assert!(index_managed_folder(&mut store).is_empty());
        assert!(store.mods.is_empty());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn choosing_another_managed_folder_preserves_existing_library_and_operations() {
        let (root, state, original) = crate::lifecycle_tests::fixture();
        import_folder_core(original.to_string_lossy().into_owned(), &state).unwrap();
        let old = state.store.lock().unwrap().mods[0].clone();
        let library = root.join("selected");
        mod_folder(&library.join("NewMod"), "NewMod");
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.library_dir = library.to_string_lossy().into_owned();
        save_settings_core(settings, &state).unwrap();
        assert_eq!(state.store.lock().unwrap().mods.len(), 2);
        assert!(Path::new(&old.path).join("payload.pak").exists());
        remove_mod_core(old.id, &state).unwrap();
        assert!(original.join("payload.pak").exists());
        assert!(library.join("NewMod/payload.pak").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_settings_save_rolls_back_index_without_touching_selected_mods() {
        let (root, state, original) = crate::lifecycle_tests::fixture();
        let library = root.join("selected");
        mod_folder(&library.join("NewMod"), "NewMod");
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.library_dir = library.to_string_lossy().into_owned();
        fs::create_dir(&state.file).unwrap();
        assert!(save_settings_core(settings, &state).is_err());
        let store = state.store.lock().unwrap();
        assert!(store.mods.is_empty());
        assert_eq!(Path::new(&store.settings.library_dir), root.join("library"));
        assert!(library.join("NewMod/payload.pak").exists());
        assert!(original.join("payload.pak").exists());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod thumbnail_tests {
    use super::*;

    #[test]
    fn custom_thumbnail_survives_adoption_and_replacement_without_editing_game() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let image = root.join("preview.PNG");
        fs::write(&image, b"test-image").unwrap();
        let manifest = fs::read(source.join("mod_manifest.json")).unwrap();
        let id = scan(Path::new(&state.store.lock().unwrap().settings.mods_dir)).0[0]
            .id
            .clone();
        set_mod_thumbnail_core(id, image.to_string_lossy().into_owned(), &state).unwrap();
        let previous = state.store.lock().unwrap().thumbnails["hair_abc"].clone();
        assert_eq!(fs::read(root.join(&previous)).unwrap(), b"test-image");
        assert_eq!(
            fs::read(source.join("mod_manifest.json")).unwrap(),
            manifest
        );
        assert!(!source.join("preview.PNG").exists());
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        set_mod_thumbnail_core(id, image.to_string_lossy().into_owned(), &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert!(root.join(&saved.thumbnails["hair_abc"]).is_file());
        assert!(!root.join(previous).exists());
        assert!(Path::new(&saved.thumbnails["hair_abc"]).is_relative());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_thumbnail_and_failed_save_preserve_previous_state() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let id = scan(Path::new(&state.store.lock().unwrap().settings.mods_dir)).0[0]
            .id
            .clone();
        let image = root.join("preview.png");
        fs::write(&image, b"test-image").unwrap();
        assert!(set_mod_thumbnail_core(
            "missing".into(),
            image.to_string_lossy().into_owned(),
            &state
        )
        .is_err());
        assert!(set_mod_thumbnail_core(
            id.clone(),
            source
                .join("mod_manifest.json")
                .to_string_lossy()
                .into_owned(),
            &state
        )
        .is_err());
        fs::create_dir(&state.file).unwrap();
        assert!(set_mod_thumbnail_core(id, image.to_string_lossy().into_owned(), &state).is_err());
        assert!(state.store.lock().unwrap().thumbnails.is_empty());
        assert_eq!(fs::read_dir(root.join("thumbnails")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_are_game_scoped() {
        assert!(parse_link("nxm://skyrim/mods/1/files/2").is_err());
        assert!(parse_link("https://inzoi/mods/1/files/2").is_err());
        assert_eq!(
            parse_link("nxm://inzoi/mods/123/files/456?key=private")
                .unwrap()
                .file_id,
            456
        );
        assert!(parse_link("nxm://inzoi/mods/../files/2").is_err());
    }
    #[test]
    fn projects_cannot_escape_root() {
        for s in ["", "..", "../evil", "C:\\evil", "foo/bar", "foo."] {
            assert!(!safe_project(s));
        }
        assert!(safe_project("Hair_ABC"));
    }
}

#[cfg(test)]
mod filesystem_tests {
    use super::*;
    #[test]
    fn scan_option_excludes_nested_mods_and_supports_deep_folders() {
        let root = std::env::temp_dir().join(format!("inzoimm-scan-{}", uuid::Uuid::new_v4()));
        let nested = root.join("one/two/three/four/five/six/seven/eight/DeepMod");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("mod_manifest.json"),
            br#"{"ProjectName":"RootMod"}"#,
        )
        .unwrap();
        fs::write(
            nested.join("mod_manifest.json"),
            br#"{"ProjectName":"DeepMod"}"#,
        )
        .unwrap();
        let (shallow, warnings) = scan_with_options(&root, false);
        assert!(warnings.is_empty());
        assert_eq!(shallow.len(), 1);
        assert_eq!(shallow[0].project, "RootMod");
        let (recursive, warnings) = scan_with_options(&root, true);
        assert!(warnings.is_empty());
        assert_eq!(recursive.len(), 2);
        assert!(recursive.iter().any(|m| m.project == "DeepMod"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_preference_defaults_on_for_old_settings_and_round_trips_off() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"mods_dir":"game","library_dir":"library"}"#).unwrap();
        assert!(settings.scan_subfolders);
        settings.scan_subfolders = false;
        let saved = serde_json::to_vec(&settings).unwrap();
        assert!(
            !serde_json::from_slice::<Settings>(&saved)
                .unwrap()
                .scan_subfolders
        );
    }

    #[test]
    fn folder_import_honors_scan_option_without_copying_unselected_children() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let selected = root.join("game");
        assert!(
            import_folder_with_options(selected.to_string_lossy().into_owned(), false, &state)
                .is_err()
        );
        assert!(state.store.lock().unwrap().mods.is_empty());
        import_folder_with_options(selected.to_string_lossy().into_owned(), true, &state).unwrap();
        let s = state.store.lock().unwrap();
        assert_eq!(s.mods.len(), 1);
        assert!(Path::new(&s.mods[0].path).join("payload.pak").exists());
        assert!(source.join("payload.pak").exists());
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovers_nested_mods_without_writing_game_files() {
        let root = std::env::temp_dir().join(format!("inzoimm-test-{}", uuid::Uuid::new_v4()));
        let folder = root.join("88849/123_456/Hair_ABC");
        fs::create_dir_all(&folder).unwrap();
        let content = r#"{"ProjectName":"Hair_ABC","FriendlyName":"Hair collection","Author":"Test","Version":"1.0"}"#;
        fs::write(folder.join("mod_manifest.json"), content).unwrap();
        fs::write(folder.join("thumbnail.png"), b"test-thumbnail").unwrap();
        let (mods, warnings) = scan(&root);
        assert!(warnings.is_empty());
        assert_eq!(mods.len(), 1);
        assert!(!mods[0].managed);
        assert!(mods[0].enabled);
        assert!(mods[0].thumbnail.is_some());
        assert_eq!(
            fs::read_to_string(folder.join("mod_manifest.json")).unwrap(),
            content
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_manifest_is_visible_as_warning() {
        let root = std::env::temp_dir().join(format!("inzoimm-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("mod_manifest.json"), b"broken").unwrap();
        let (mods, warnings) = scan(&root);
        assert!(mods.is_empty());
        assert_eq!(warnings.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn copy_preserves_binary_mod_payload() {
        let root = std::env::temp_dir().join(format!("inzoimm-test-{}", uuid::Uuid::new_v4()));
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(src.join("Content/Paks/Windows")).unwrap();
        let bytes: Vec<u8> = (0..=255).collect();
        fs::write(src.join("Content/Paks/Windows/example.pak"), &bytes).unwrap();
        copy_tree(&src, &dst).unwrap();
        assert_eq!(
            fs::read(dst.join("Content/Paks/Windows/example.pak")).unwrap(),
            bytes
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::io::Write;
    pub(super) fn fixture() -> (PathBuf, State, PathBuf) {
        let root = std::env::temp_dir().join(format!("inzoimm-lifecycle-{}", uuid::Uuid::new_v4()));
        let game = root.join("game/Mods");
        let source = game.join("88849/123/Hair_ABC");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("mod_manifest.json"),
            r#"{"ProjectName":"Hair_ABC","FriendlyName":"Hair"}"#,
        )
        .unwrap();
        fs::write(source.join("payload.pak"), [1, 2, 3, 4]).unwrap();
        let state = State {
            file: root.join("store.json"),
            pending: Mutex::new(vec![]),
            requests: Mutex::new(std::collections::HashMap::new()),
            mod_updates: Mutex::new(Default::default()),
            store: Mutex::new(Store {
                settings: Settings {
                    mods_dir: game.to_string_lossy().into_owned(),
                    library_dir: root.join("library").to_string_lossy().into_owned(),
                    scan_subfolders: true,
                    source_dirs: vec![],
                    downloads_dir: String::new(),
                },
                mods: vec![],
                thumbnails: Default::default(),
                tags: Default::default(),
            }),
        };
        (root, state, source)
    }
    #[test]
    fn adopt_disable_enable_remove_keeps_recovery_copy() {
        let (root, state, source) = fixture();
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let m = state.store.lock().unwrap().mods[0].clone();
        assert!(source.join("payload.pak").exists());
        assert!(Path::new(&m.path).join("payload.pak").exists());
        toggle_mod_core(m.id.clone(), &state).unwrap();
        assert!(!source.exists());
        toggle_mod_core(m.id.clone(), &state).unwrap();
        assert_eq!(fs::read(source.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        remove_mod_core(m.id, &state).unwrap();
        assert!(!source.exists());
        assert!(state.store.lock().unwrap().mods.is_empty());
        assert_eq!(
            fs::read_dir(root.join("library/.trash")).unwrap().count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ownership_mismatch_does_not_remove_external_files() {
        let (root, state, source) = fixture();
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        fs::write(source.join(".inzoimm-owner"), "different-app").unwrap();
        assert!(toggle_mod_core(id, &state).is_err());
        assert!(source.join("payload.pak").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_duplicate_preserves_original_library() {
        let (root, state, source) = fixture();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        assert!(import_folder_core(source.to_string_lossy().into_owned(), &state).is_err());
        assert_eq!(state.store.lock().unwrap().mods.len(), 1);
        assert!(source.join("payload.pak").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsafe_zip_cannot_escape_destination() {
        let (root, _state, _) = fixture();
        let archive = root.join("bad.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"escape").unwrap();
        zip.finish().unwrap();
        let dest = root.join("extract");
        fs::create_dir(&dest).unwrap();
        assert!(extract_zip(&archive, &dest).is_err());
        assert!(!root.join("escaped.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn valid_zip_preserves_payload() {
        let (root, _state, _) = fixture();
        let archive = root.join("good.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file(
            "Hair_ABC/payload.pak",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&[1, 2, 3, 4]).unwrap();
        zip.finish().unwrap();
        let dest = root.join("extract");
        fs::create_dir(&dest).unwrap();
        extract_zip(&archive, &dest).unwrap();
        assert_eq!(
            fs::read(dest.join("Hair_ABC/payload.pak")).unwrap(),
            [1, 2, 3, 4]
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod manifest_compat_tests {
    use super::*;
    #[test]
    fn utf8_bom_and_camel_case_are_accepted() {
        let bytes=b"\xef\xbb\xbf{\"projectName\":\"Hair_ABC\",\"friendlyName\":\"Hair\",\"bEnable\":false}";
        let v = manifest_value(bytes).unwrap();
        assert_eq!(field(&v, "ProjectName"), "Hair_ABC");
        assert_eq!(value(&v, "bEnable").unwrap().as_bool(), Some(false));
    }
    #[test]
    fn activation_sets_existing_camel_case_flag() {
        let root = std::env::temp_dir().join(format!("inzoimm-enable-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("mod_manifest.json"),
            b"{\"projectName\":\"Hair_ABC\",\"benable\":false}",
        )
        .unwrap();
        enable_manifest(&root).unwrap();
        let v = manifest_value(&fs::read(root.join("mod_manifest.json")).unwrap()).unwrap();
        assert_eq!(value(&v, "bEnable").unwrap().as_bool(), Some(true));
        assert_eq!(v.as_object().unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn initial_queue_strips_sensitive_nxm_query() {
        let parsed =
            parse_link("nxm://inzoi/mods/1/files/2?key=SECRET&expires=123&user_id=4").unwrap();
        let json = serde_json::to_string(&parsed).unwrap();
        assert!(!json.contains("SECRET"));
        assert!(!json.contains("user_id"));
    }
}
#[cfg(test)]
mod installed_scan_diagnostic {
    #[test]
    #[ignore = "Requires INZOIMM_SCAN_ROOT; read-only diagnostic against a chosen installation"]
    fn scan_selected_installation() {
        let root = std::env::var("INZOIMM_SCAN_ROOT").expect("Set INZOIMM_SCAN_ROOT");
        let (mods, warnings) = super::scan(std::path::Path::new(&root));
        println!(
            "{}",
            serde_json::json!({"count":mods.len(),"warnings":warnings})
        );
        assert!(!mods.is_empty());
    }
}
#[cfg(test)]
mod utf16_tests {
    use super::*;
    #[test]
    fn both_utf16_byte_orders_are_supported() {
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in "{\"ProjectName\":\"Hair_ABC\"}".encode_utf16() {
                bytes.extend(if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                })
            }
            assert_eq!(
                field(&manifest_value(&bytes).unwrap(), "ProjectName"),
                "Hair_ABC"
            );
        }
    }
    #[test]
    fn truncated_utf16_reports_error() {
        assert!(manifest_value(&[0xff, 0xfe, 0x7b]).is_err());
    }
}

#[cfg(test)]
mod copied_external_tests {
    use super::*;
    #[test]
    fn removing_library_copy_keeps_unmanaged_original() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        let direct = root.join("game/Mods/Hair_ABC");
        fs::rename(&source, &direct).unwrap();
        import_folder_core(direct.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        remove_mod_core(id, &state).unwrap();
        assert_eq!(fs::read(direct.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        assert!(!direct.join(".inzoimm-owner").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removing_adopted_disabled_mod_retains_backup() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        fs::write(
            source.join("mod_manifest.json"),
            br#"{"ProjectName":"Hair_ABC","bEnable":false}"#,
        )
        .unwrap();
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let m = state.store.lock().unwrap().mods[0].clone();
        assert!(!m.enabled);
        remove_mod_core(m.id, &state).unwrap();
        assert!(!source.exists());
        assert_eq!(
            fs::read_dir(root.join("game/.inzoimm-disabled"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            fs::read_dir(root.join("library/.trash")).unwrap().count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(test)]
mod failure_rollback_tests {
    use super::*;
    #[test]
    fn failed_adoption_restores_original_and_index() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        fs::create_dir(&state.file).unwrap();
        assert!(adopt_mod_core(source.to_string_lossy().into_owned(), &state).is_err());
        assert!(source.join("payload.pak").exists());
        assert!(!source.join(".inzoimm-owner").exists());
        assert!(state.store.lock().unwrap().mods.is_empty());
        assert_eq!(fs::read_dir(root.join("library")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_disable_restores_original_deployment() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        fs::remove_file(&state.file).unwrap();
        fs::create_dir(&state.file).unwrap();
        assert!(toggle_mod_core(id, &state).is_err());
        assert_eq!(fs::read(source.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        assert!(state.store.lock().unwrap().mods[0].enabled);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn deployment_is_published_only_when_complete() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        fs::remove_dir_all(&source).unwrap(); // The original is no longer installed.
        let id = state.store.lock().unwrap().mods[0].id.clone();
        toggle_mod_core(id, &state).unwrap();
        let target = root.join("game/Mods/Hair_ABC");
        assert_eq!(fs::read(target.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        assert_eq!(
            fs::read_dir(root.join("game/.inzoimm-staging"))
                .unwrap()
                .count(),
            0
        );
        assert!(fs::read_dir(root.join("game/Mods")).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".inzoimm-")));
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod unified_library_tests {
    use super::*;

    #[test]
    fn installed_mod_can_be_removed_without_a_separate_adoption_step() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        let external = library::catalogue(&state.store.lock().unwrap()).0.remove(0);
        remove_mod_core(external.id, &state).unwrap();
        assert!(!source.exists());
        assert!(library::catalogue(&state.store.lock().unwrap())
            .0
            .is_empty());
        let recovered = fs::read_dir(root.join("library/.trash"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(recovered.join("payload.pak")).unwrap(),
            [1, 2, 3, 4]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn single_mod_source_folder_supports_activation_and_recoverable_removal() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        let local = root.join("local-mod");
        fs::rename(source, &local).unwrap();
        state
            .store
            .lock()
            .unwrap()
            .settings
            .source_dirs
            .push(local.to_string_lossy().into_owned().into());
        let external = library::catalogue(&state.store.lock().unwrap()).0.remove(0);
        let id = set_mod_enabled_core(external.id, true, &state).unwrap();
        assert!(root.join("game/Mods/Hair_ABC").exists());
        remove_mod_core(id, &state).unwrap();
        assert!(!local.exists());
        assert!(!root.join("game/Mods/Hair_ABC").exists());
        assert!(library::catalogue(&state.store.lock().unwrap())
            .0
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copied_version_cannot_enable_while_the_original_is_installed() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        assert!(set_mod_enabled_core(id, true, &state).is_err());
        assert!(!root.join("game/Mods/Hair_ABC").exists());
        assert!(source.join("payload.pak").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_mod_disables_directly_survives_reload_and_removes_while_enabled() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        let external = library::catalogue(&state.store.lock().unwrap()).0.remove(0);
        let id = set_mod_enabled_core(external.id, false, &state).unwrap();
        assert!(!source.exists());
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        let (mods, _) = library::catalogue(&saved);
        assert_eq!(mods.len(), 1);
        assert!(mods[0].managed && !mods[0].enabled);
        let library_path = PathBuf::from(&mods[0].path);
        assert!(library_path.starts_with(root.join("library").canonicalize().unwrap()));
        assert_eq!(
            fs::read(library_path.join("payload.pak")).unwrap(),
            [1, 2, 3, 4]
        );
        set_mod_enabled_core(id.clone(), true, &state).unwrap();
        set_mod_enabled_core(id.clone(), true, &state).unwrap();
        assert!(source.exists());
        assert_eq!(library::catalogue(&state.store.lock().unwrap()).0.len(), 1);
        remove_mod_core(id, &state).unwrap();
        assert!(!source.exists() && !library_path.exists());
        assert!(library::catalogue(&state.store.lock().unwrap())
            .0
            .is_empty());
        assert_eq!(
            fs::read_dir(root.join("library/.trash")).unwrap().count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disabling_an_already_disabled_game_manifest_still_keeps_it_only_in_library() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        fs::write(
            source.join("mod_manifest.json"),
            br#"{"ProjectName":"Hair_ABC","bEnable":false}"#,
        )
        .unwrap();
        let external = library::catalogue(&state.store.lock().unwrap()).0.remove(0);
        assert!(!external.enabled);
        set_mod_enabled_core(external.id, false, &state).unwrap();
        assert!(!source.exists());
        assert!(Path::new(&state.store.lock().unwrap().mods[0].path)
            .join("payload.pak")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_mod_has_direct_controls_without_copying_or_duplicate_rows() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        let sources = root.join("sources");
        fs::create_dir_all(&sources).unwrap();
        let local = sources.join("Hair");
        fs::rename(source, &local).unwrap();
        state
            .store
            .lock()
            .unwrap()
            .settings
            .source_dirs
            .push(sources.to_string_lossy().into_owned().into());
        let external = library::catalogue(&state.store.lock().unwrap()).0.remove(0);
        let id = set_mod_enabled_core(external.id, true, &state).unwrap();
        assert!(root.join("game/Mods/Hair_ABC/payload.pak").exists());
        assert_eq!(library::catalogue(&state.store.lock().unwrap()).0.len(), 1);
        set_mod_enabled_core(id.clone(), false, &state).unwrap();
        assert!(!root.join("game/Mods/Hair_ABC").exists());
        assert_eq!(fs::read(local.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        remove_mod_core(id, &state).unwrap();
        assert!(!local.exists());
        let remaining = library::catalogue(&state.store.lock().unwrap()).0;
        assert!(
            remaining.is_empty(),
            "Unexpected mods: {:?}",
            remaining.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_active_removal_restores_library_game_and_enabled_state() {
        let (root, state, source) = super::lifecycle_tests::fixture();
        adopt_mod_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let m = state.store.lock().unwrap().mods[0].clone();
        fs::remove_file(&state.file).unwrap();
        fs::create_dir(&state.file).unwrap();
        assert!(remove_mod_core(m.id, &state).is_err());
        assert!(state.store.lock().unwrap().mods[0].enabled);
        assert_eq!(fs::read(source.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        assert!(Path::new(&m.path).join("payload.pak").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
