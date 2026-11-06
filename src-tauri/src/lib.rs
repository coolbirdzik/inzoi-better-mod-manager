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
    vortex_dir: String,
    github_repo: String,
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
}
#[derive(Clone, Serialize, Deserialize)]
struct Store {
    settings: Settings,
    mods: Vec<Mod>,
}
struct State {
    file: PathBuf,
    store: Mutex<Store>,
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
fn field(v: &serde_json::Value, k: &str) -> String {
    v.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}
fn read_mod(folder: &Path, id: String, managed: bool) -> Result<Mod> {
    let manifest = folder.join("mod_manifest.json");
    let v: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).map_err(err)?).map_err(err)?;
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
        enabled: !managed,
        source: field(&v, "Url"),
        project,
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
    (mods, warnings)
}
#[tauri::command]
fn snapshot(app: tauri::AppHandle, state: tauri::State<State>) -> Result<Snapshot> {
    let s = state.store.lock().map_err(err)?;
    let (external, warnings) = scan(Path::new(&s.settings.mods_dir));
    let mut mods = s.mods.clone();
    for m in external {
        if !mods.iter().any(|x| x.project == m.project && x.enabled) {
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
#[tauri::command]
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
    s.settings = settings;
    if let Err(e) = write_store(&state, &s) {
        s.settings = old;
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
#[tauri::command]
fn import_folder(path: String, state: tauri::State<State>) -> Result<()> {
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
    let mut s = state.store.lock().map_err(err)?;
    if mods
        .iter()
        .any(|m| s.mods.iter().any(|x| x.project == m.project))
    {
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
#[tauri::command]
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
#[tauri::command]
fn toggle_mod(id: String, state: tauri::State<State>) -> Result<()> {
    let mut s = state.store.lock().map_err(err)?;
    let pos = s
        .mods
        .iter()
        .position(|m| m.id == id)
        .ok_or("Import this external mod before managing it")?;
    let m = s.mods[pos].clone();
    let target = PathBuf::from(&s.settings.mods_dir).join(&m.project);
    let marker = target.join(".inzoimm-owner");
    if m.enabled {
        if fs::read_to_string(&marker).map_err(err)? != m.id {
            return Err("Ownership marker mismatch; refusing to touch game files".into());
        }
        let trash = PathBuf::from(&s.settings.library_dir).join(".disabled");
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
        if target.exists() {
            return Err(
                "A mod with this ProjectName already exists in the game. It was not overwritten."
                    .into(),
            );
        }
        fs::create_dir_all(Path::new(&s.settings.mods_dir)).map_err(err)?;
        let temp = target.with_file_name(format!(".inzoimm-{}", m.id));
        if temp.exists() {
            return Err("An incomplete deployment exists; inspect the game mod folder".into());
        }
        copy_tree(Path::new(&m.path), &temp)?;
        fs::write(temp.join(".inzoimm-owner"), &m.id).map_err(err)?;
        fs::rename(&temp, &target).map_err(err)?;
        s.mods[pos].enabled = true;
        if let Err(e) = write_store(&state, &s) {
            s.mods[pos].enabled = false;
            let _ = fs::rename(&target, &temp);
            return Err(e);
        }
    }
    Ok(())
}
#[tauri::command]
fn remove_mod(id: String, state: tauri::State<State>) -> Result<()> {
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
    let dest = trash.join(format!("{}-{}", m.id, uuid::Uuid::new_v4()));
    fs::rename(&source, &dest).map_err(err)?;
    s.mods.remove(pos);
    if let Err(e) = write_store(&state, &s) {
        s.mods.insert(pos, m);
        let _ = fs::rename(dest, source);
        return Err(e);
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
struct NexusLink {
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
        game: "inzoi".into(),
        mod_id: parts[1].parse().map_err(err)?,
        file_id: parts[3].parse().map_err(err)?,
        url: format!(
            "https://www.nexusmods.com/inzoi/mods/{}?tab=files&file_id={}",
            parts[1], parts[3]
        ),
    })
}
#[tauri::command]
fn parse_nxm(link: String) -> Result<NexusLink> {
    parse_link(&link)
}
#[tauri::command]
fn register_nxm(app: tauri::AppHandle) -> Result<()> {
    app.deep_link().register("nxm").map_err(err)
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
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
                        vortex_dir: app
                            .path()
                            .home_dir()?
                            .join("AppData/Roaming/Vortex")
                            .to_string_lossy()
                            .into_owned(),
                        github_repo: "coolbirdzik/inzoi-better-mod-manager".into(),
                    },
                    mods: vec![],
                }
            };
            app.manage(State {
                file,
                store: Mutex::new(store),
            });
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    if let Ok(link) = parse_link(url.as_str()) {
                        let _ = handle.emit("nexus-link", link);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_settings,
            import_folder,
            edit_mod,
            toggle_mod,
            remove_mod,
            parse_nxm,
            register_nxm
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
