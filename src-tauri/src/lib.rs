mod nexus;
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
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    mods_dir: String,
    library_dir: String,
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
}
#[derive(Clone, Serialize, Deserialize)]
struct Store {
    settings: Settings,
    mods: Vec<Mod>,
}
struct State {
    file: PathBuf,
    store: Mutex<Store>,
    pending: Mutex<Vec<NexusLink>>,
    requests: Mutex<std::collections::HashMap<String, nexus::Request>>,
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
    Ok(Mod {
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
    })
}
fn scan(root: &Path) -> (Vec<Mod>, Vec<String>) {
    let mut mods = vec![];
    let mut warnings = vec![];
    if !root.exists() {
        return (
            mods,
            vec![format!("Mod folder does not exist: {}", root.display())],
        );
    }
    for e in WalkDir::new(root).follow_links(false).max_depth(7) {
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
#[tauri::command(async)]
fn snapshot(app: tauri::AppHandle, state: tauri::State<State>) -> Result<Snapshot> {
    let s = state.store.lock().map_err(err)?;
    let (external, warnings) = scan(Path::new(&s.settings.mods_dir));
    let mut mods = s.mods.clone();
    for m in external {
        if !mods
            .iter()
            .any(|x| x.project.eq_ignore_ascii_case(&m.project) && x.enabled)
        {
            mods.push(m);
        }
    }
    for root in [&s.settings.mods_dir, &s.settings.library_dir] {
        let _ = app.asset_protocol_scope().allow_directory(root, true);
    }
    Ok(Snapshot {
        settings: s.settings.clone(),
        mods,
        warnings,
    })
}
#[tauri::command(async)]
fn save_settings(settings: Settings, state: tauri::State<State>) -> Result<()> {
    if !Path::new(&settings.mods_dir).is_absolute()
        || !Path::new(&settings.library_dir).is_absolute()
    {
        return Err("Use absolute folder paths".into());
    }
    let mut s = state.store.lock().map_err(err)?;
    if !s.mods.is_empty() && settings.library_dir != s.settings.library_dir {
        return Err("Library relocation requires exporting installed mods first".into());
    }
    if s.mods.iter().any(|m| m.enabled) && settings.mods_dir != s.settings.mods_dir {
        return Err("Disable managed mods before changing the game mod folder".into());
    }
    let old = s.settings.clone();
    let old_mods = s.mods.clone();
    if s.settings.mods_dir != settings.mods_dir {
        for m in &mut s.mods {
            m.deployed_path = None;
        }
    }
    s.settings = settings;
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
fn import_folder_core(path: String, state: &State) -> Result<()> {
    let src = PathBuf::from(&path).canonicalize().map_err(err)?;
    if !src.is_dir() {
        return Err("Choose a folder containing mod_manifest.json".into());
    }
    let (mods, _) = scan(&src);
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
    if mods.iter().any(|m| {
        s.mods
            .iter()
            .any(|x| x.project.eq_ignore_ascii_case(&m.project))
    }) {
        return Err("One of these projects already exists in your library".into());
    }
    let library = PathBuf::from(&s.settings.library_dir);
    fs::create_dir_all(&library).map_err(err)?;
    let library = library.canonicalize().map_err(err)?;
    if library.starts_with(&src) {
        return Err("Import source must not contain the library".into());
    }
    let mut added = vec![];
    for m in mods {
        let id = uuid::Uuid::new_v4().to_string();
        let target = library.join(&id);
        if let Err(e) = copy_tree(Path::new(&m.path), &target) {
            let _ = fs::remove_dir_all(&target);
            for m in &added {
                let m: &Mod = m;
                let _ = fs::remove_dir_all(&m.path);
            }
            return Err(e);
        };
        added.push(read_mod(&target, id, true)?);
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
    let mut s = state.store.lock().map_err(err)?;
    let pos = s
        .mods
        .iter()
        .position(|m| m.id == id)
        .ok_or("Import this external mod before managing it")?;
    let m = s.mods[pos].clone();
    let target = deployment_target(&s.settings, &m)?;
    let marker = target.join(".inzoimm-owner");
    if m.enabled {
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
            s.mods[pos].enabled = true;
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
                s.mods[pos].enabled = false;
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
            s.mods[pos].enabled = false;
            let _ = fs::rename(&target, &temp);
            return Err(e);
        }
    }
    Ok(())
}
fn remove_mod_core(id: String, state: &State) -> Result<()> {
    let mut s = state.store.lock().map_err(err)?;
    let pos = s
        .mods
        .iter()
        .position(|m| m.id == id)
        .ok_or("External mods are not removed by this command")?;
    let m = s.mods[pos].clone();
    if m.enabled {
        return Err("Disable this mod before removing it".into());
    }
    let source = PathBuf::from(&m.path).canonicalize().map_err(err)?;
    let root = PathBuf::from(&s.settings.library_dir)
        .canonicalize()
        .map_err(err)?;
    if source.parent() != Some(root.as_path())
        || source.file_name().and_then(|s| s.to_str()) != Some(m.id.as_str())
    {
        return Err("Library containment check failed".into());
    }
    let trash = root.join(".trash");
    fs::create_dir_all(&trash).map_err(err)?;
    let deployed = deployment_target(&s.settings, &m)?;
    let mut deployed_backup = None;
    if deployed.exists()
        && (m.deployed_path.is_some()
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
    let dest = trash.join(format!("{}-{}", m.id, uuid::Uuid::new_v4()));
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
fn import_folder(path: String, state: tauri::State<State>) -> Result<()> {
    import_folder_core(path, &state)
}
#[tauri::command(async)]
fn toggle_mod(id: String, state: tauri::State<State>) -> Result<()> {
    toggle_mod_core(id, &state)
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
    if s.mods
        .iter()
        .any(|x| x.project.eq_ignore_ascii_case(&m.project))
    {
        return Err("This project is already in your library".into());
    }
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
    let temp = state
        .file
        .parent()
        .ok_or("Missing app data folder")?
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&temp).map_err(err)?;
    let result = extract_zip(Path::new(&path), &temp)
        .and_then(|_| import_folder_core(temp.to_string_lossy().into_owned(), &state));
    let cleanup = fs::remove_dir_all(&temp).map_err(err);
    result.and(cleanup)
}
#[derive(Clone, Serialize, Deserialize)]
struct NexusLink {
    request_id: String,
    game: String,
    mod_id: u64,
    file_id: u64,
    url: String,
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
            let store = if file.exists() {
                serde_json::from_slice(&fs::read(&file)?)?
            } else {
                let docs = app.path().document_dir()?;
                Store {
                    settings: Settings {
                        mods_dir: docs.join("inZOI/Mods").to_string_lossy().into_owned(),
                        library_dir: dir.join("library").to_string_lossy().into_owned(),
                    },
                    mods: vec![],
                }
            };
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
            save_settings,
            import_folder,
            import_zip,
            adopt_mod,
            edit_mod,
            toggle_mod,
            remove_mod,
            parse_nxm,
            pending_links,
            register_nxm,
            nexus::download_nxm
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start inZOI Better Mod Manager");
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
            store: Mutex::new(Store {
                settings: Settings {
                    mods_dir: game.to_string_lossy().into_owned(),
                    library_dir: root.join("library").to_string_lossy().into_owned(),
                },
                mods: vec![],
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
        assert!(remove_mod_core(m.id.clone(), &state).is_err());
        toggle_mod_core(m.id.clone(), &state).unwrap();
        remove_mod_core(m.id, &state).unwrap();
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
