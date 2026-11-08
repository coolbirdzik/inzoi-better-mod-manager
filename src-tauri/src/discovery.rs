//! Read-only discovery. Vortex's live database is never opened by LevelDB:
//! recovery and locks operate exclusively on a disposable copy.
use super::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

#[derive(Default)]
pub(crate) struct ScanState {
    active: Mutex<ScanRegistry>,
}
#[derive(Default)]
struct ScanRegistry {
    active: HashMap<String, Arc<AtomicBool>>,
    cancelled: HashSet<String>,
}
struct ScanGuard<'a> {
    state: &'a ScanState,
    id: String,
}
impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.state.active.lock() {
            active.active.remove(&self.id);
            active.cancelled.remove(&self.id);
        }
    }
}

#[derive(Clone, Serialize, Default)]
pub(crate) struct ScanProgress {
    current_path: String,
    entries: u64,
    mods: u64,
    archives: u64,
}
struct ScanControl {
    cancel: Arc<AtomicBool>,
    channel: Option<Channel<ScanProgress>>,
    progress: ScanProgress,
    last: Instant,
    /// Roots that are only searched a few levels deep in quick mode.
    shallow: Vec<PathBuf>,
}
/// Nexus mods land in `Downloads/<Mod>/…`; deeper trees are unrelated files.
const QUICK_SHALLOW_DEPTH: usize = 3;
impl Default for ScanControl {
    fn default() -> Self {
        Self {
            shallow: vec![],
            cancel: Arc::new(AtomicBool::new(false)),
            channel: None,
            progress: Default::default(),
            last: Instant::now(),
        }
    }
}
impl ScanControl {
    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    fn report(&mut self, path: &Path, force: bool) {
        if force || self.last.elapsed() >= Duration::from_millis(200) {
            self.progress.current_path = dunce::simplified(path).to_string_lossy().into_owned();
            if self
                .channel
                .as_ref()
                .is_some_and(|channel| channel.send(self.progress.clone()).is_err())
            {
                self.cancel.store(true, Ordering::Relaxed);
            }
            self.last = Instant::now();
        }
    }
}

#[derive(Serialize)]
pub(crate) struct DiscoveredArchive {
    path: String,
    name: String,
}

#[derive(Serialize)]
pub(crate) struct DiscoveredFolder {
    path: String,
    origin: String,
    mods: Vec<String>,
    already_added: bool,
}
#[derive(Serialize, Default)]
pub(crate) struct Discovery {
    folders: Vec<DiscoveredFolder>,
    warnings: Vec<String>,
    archives: Vec<DiscoveredArchive>,
    cancelled: bool,
    scanned_entries: u64,
    skipped_warnings: u64,
}

fn vortex_paths(data: &Path) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
    let source = data.join("state.v2");
    if !source.is_dir() {
        return Ok((None, None));
    }
    let temporary = tempfile::tempdir().map_err(err)?;
    let mut originals = vec![];
    let mut total = 0;
    for entry in fs::read_dir(&source).map_err(err)? {
        let entry = entry.map_err(err)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Skip locks/logging and all unrelated files. Never follow links.
        if !(name == "CURRENT"
            || name.starts_with("MANIFEST-")
            || name.ends_with(".ldb")
            || name.ends_with(".sst")
            || name.ends_with(".log") && name.chars().next().is_some_and(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let metadata = entry.path().symlink_metadata().map_err(err)?;
        if !metadata.file_type().is_file() {
            return Err("Vortex database contains a linked or invalid file".into());
        }
        total += metadata.len();
        if total > 256 * 1024 * 1024 {
            return Err("Vortex database exceeds the discovery size limit; choose its staging folder manually".into());
        }
        fs::copy(entry.path(), temporary.path().join(entry.file_name())).map_err(err)?;
        originals.push((
            entry.path(),
            metadata.len(),
            metadata.modified().map_err(err)?,
        ));
    }
    for (path, len, modified) in originals {
        let now = fs::metadata(path).map_err(err)?;
        if now.len() != len || now.modified().map_err(err)? != modified {
            return Err("Vortex data changed during scanning. Close Vortex and scan again".into());
        }
    }
    let mut db = rusty_leveldb::DB::open(
        temporary.path(),
        rusty_leveldb::Options {
            create_if_missing: false,
            ..Default::default()
        },
    )
    .map_err(err)?;
    // Vortex persists Redux paths with ### separators. Accommodate both
    // individual leaf keys and older object-valued parent keys.
    let mut pattern = None;
    let snapshot = db.get_snapshot();
    for (key, segments) in [
        ("settings###mods###installPath###inzoi", vec![]),
        ("settings###mods###installPath", vec!["inzoi"]),
        ("settings###mods", vec!["installPath", "inzoi"]),
        ("settings", vec!["mods", "installPath", "inzoi"]),
    ] {
        if let Some(bytes) = db.get_at(&snapshot, key.as_bytes()).map_err(err)? {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(err)?;
            for segment in segments {
                value = value[segment].clone();
            }
            if let Some(path) = value.as_str().filter(|p| !p.is_empty()) {
                pattern = Some(path.to_string());
                break;
            }
        }
    }
    let staging = pattern
        .map(|pattern| expand_vortex_path(&pattern, data))
        .transpose()?;
    let mut download_pattern = None;
    for (key, segments) in [
        ("settings###downloads###path", vec![]),
        ("settings###downloads", vec!["path"]),
        ("settings", vec!["downloads", "path"]),
    ] {
        if let Some(bytes) = db.get_at(&snapshot, key.as_bytes()).map_err(err)? {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(err)?;
            for segment in segments {
                value = value[segment].clone();
            }
            if let Some(path) = value.as_str().filter(|p| !p.is_empty()) {
                download_pattern = Some(path.to_string());
                break;
            }
        }
    }
    let downloads = download_pattern
        .map(|pattern| expand_vortex_path(&pattern, data).map(|path| path.join("inzoi")))
        .transpose()?;
    Ok((staging, downloads))
}

#[cfg(test)]
fn vortex_staging(data: &Path) -> Result<Option<PathBuf>> {
    vortex_paths(data).map(|paths| paths.0)
}

fn expand_vortex_path(pattern: &str, data: &Path) -> Result<PathBuf> {
    let mut resolved = pattern.to_string();
    for (key, value) in [
        ("userdata", data.to_string_lossy().into_owned()),
        ("game", "inzoi".into()),
        ("username", std::env::var("USERNAME").unwrap_or_default()),
    ] {
        loop {
            let needle = format!("{{{key}}}");
            let Some(index) = resolved.to_ascii_lowercase().find(&needle) else {
                break;
            };
            resolved.replace_range(index..index + needle.len(), &value);
        }
    }
    let path = PathBuf::from(resolved);
    if !path.is_absolute() || path.to_string_lossy().contains(['{', '}']) {
        return Err("Unsupported Vortex path; choose its folder manually".into());
    }
    Ok(path)
}

fn add_root(roots: &mut Vec<(PathBuf, String)>, path: PathBuf, origin: &str) {
    if path.is_dir()
        && !roots
            .iter()
            .any(|(other, _)| library::same_path(&other.to_string_lossy(), &path.to_string_lossy()))
    {
        roots.push((path, origin.into()));
    }
}

fn automatic_roots(
    settings: &Settings,
    documents: &Path,
    downloads_dir: Option<&Path>,
    data_dirs: &[PathBuf],
) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let mut roots = vec![];
    let mut warnings = vec![];
    for data in data_dirs {
        match vortex_paths(data) {
            Ok((staging, downloads)) => {
                if let Some(path) = staging {
                    if !path.is_dir() {
                        warnings.push(format!(
                            "Vortex staging folder is unavailable: {}",
                            path.display()
                        ));
                    }
                    add_root(&mut roots, path, "vortex");
                }
                if let Some(path) = downloads {
                    add_root(&mut roots, path, "vortex-downloads");
                }
            }
            Err(error) => warnings.push(format!("Vortex ({}): {error}", data.display())),
        }
        add_root(&mut roots, data.join("downloads/inzoi"), "vortex-downloads");
        add_root(&mut roots, data.join("inzoi/mods"), "vortex");
    }
    add_root(&mut roots, PathBuf::from(&settings.mods_dir), "game");
    add_root(&mut roots, documents.join("inZOI/Mods"), "game");
    add_root(&mut roots, PathBuf::from(&settings.library_dir), "local");
    add_root(&mut roots, nexus::downloads_root(settings), "local");
    for source in &settings.source_dirs {
        add_root(&mut roots, PathBuf::from(&source.path), "local");
    }
    // The browser Downloads folder is shared with everything else the user
    // downloads; only a shallow look is worthwhile (see QUICK_SHALLOW_DEPTH).
    if let Some(path) = downloads_dir {
        add_root(&mut roots, path.to_path_buf(), "local");
    }
    // Vortex's default staging location on non-system drives. Query the OS for
    // real local drives instead of probing every letter (slow for dead mounts).
    #[cfg(windows)]
    for drive in discovery_drives::roots().unwrap_or_default() {
        add_root(&mut roots, drive.join("Vortex Mods").join("inzoi"), "vortex");
    }
    (roots, warnings)
}

fn path_key(path: &Path) -> String {
    let raw = dunce::simplified(path).to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        raw.trim_end_matches('/').to_ascii_lowercase()
    } else {
        raw.trim_end_matches('/').to_string()
    }
}
fn inside(path: &Path, root: &Path) -> bool {
    let path = path_key(path);
    let root = path_key(root);
    path == root || path.starts_with(&format!("{root}/"))
}
fn warning(result: &mut Discovery, text: String) {
    if result.warnings.len() < 200 {
        result.warnings.push(text);
    } else {
        result.skipped_warnings += 1;
    }
}
fn archive_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "zip" | "7z" | "rar"))
}
fn add_vortex_paths(
    data: &Path,
    hints: &mut Vec<(PathBuf, String)>,
    pending: &mut VecDeque<(PathBuf, String)>,
    result: &mut Discovery,
) {
    match vortex_paths(data) {
        Ok((staging, downloads)) => {
            for (path, origin) in staging
                .into_iter()
                .map(|p| (p, "vortex"))
                .chain(downloads.into_iter().map(|p| (p, "vortex-downloads")))
            {
                if path.is_dir() {
                    add_root(hints, path.clone(), origin);
                    pending.push_back((path, origin.into()));
                } else {
                    warning(
                        result,
                        format!("Vortex folder is unavailable: {}", path.display()),
                    );
                }
            }
        }
        Err(error) => warning(result, format!("Vortex ({}): {error}", data.display())),
    }
    for (suffix, origin) in [
        ("inzoi/mods", "vortex"),
        ("downloads/inzoi", "vortex-downloads"),
    ] {
        let path = data.join(suffix);
        if path.is_dir() {
            add_root(hints, path.clone(), origin);
            pending.push_back((path, origin.into()));
        }
    }
}

#[cfg(test)]
fn discover(settings: &Settings, roots: Vec<(PathBuf, String)>) -> Discovery {
    discover_with_control(settings, roots, false, &mut ScanControl::default())
}

fn discover_with_control(
    settings: &Settings,
    roots: Vec<(PathBuf, String)>,
    full: bool,
    control: &mut ScanControl,
) -> Discovery {
    let mut result = Discovery::default();
    let mut hints: Vec<_> = roots
        .iter()
        .filter(|(_, origin)| origin != "disk")
        .cloned()
        .collect();
    let mut pending = VecDeque::from(roots);
    let mut visited = HashSet::new();
    let mut completed = HashSet::new();
    let mut seen_mods = HashSet::new();
    let mut found = vec![];
    let mut archives = HashSet::new();
    let mut nexus_archives = HashSet::new();
    let mut databases = HashSet::new();
    while let Some((root, origin)) = pending.pop_front() {
        if control.stopped() {
            break;
        }
        control.report(&root, true);
        let root = match root.canonicalize() {
            Ok(path) => dunce::simplified(&path).to_path_buf(),
            Err(error) => {
                warning(&mut result, format!("{}: {error}", root.display()));
                continue;
            }
        };
        let key = path_key(&root);
        if !visited.insert(key.clone()) {
            continue;
        }
        let cancel = control.cancel.clone();
        let max_depth = if full {
            usize::MAX
        } else if control.shallow.iter().any(|s| path_key(s) == key) {
            QUICK_SHALLOW_DEPTH
        } else {
            32
        };
        for (index, entry) in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|e| {
                !cancel.load(Ordering::Relaxed)
                    && !discovery_drives::skip_entry(e)
                    && (e.depth() == 0
                        || !e.file_type().is_dir()
                        || !completed.contains(&path_key(e.path())))
            })
            .enumerate()
        {
            if control.stopped() {
                break;
            }
            if !full && index >= 150_000 {
                warning(&mut result, format!("Scan limit reached: {}. Use Scan all drives to continue without a depth or entry limit", root.display()));
                break;
            }
            control.progress.entries += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(error) => {
                    warning(&mut result, error.to_string());
                    continue;
                }
            };
            if entry.file_type().is_dir() {
                control.report(entry.path(), false);
                if !full && entry.depth() == 32 {
                    warning(
                        &mut result,
                        format!("Scan depth limit reached: {}", entry.path().display()),
                    );
                }
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy();
            if name == "mod_manifest.json" {
                let folder = path.parent().unwrap();
                match folder.canonicalize() {
                    Ok(canonical) if seen_mods.insert(path_key(&canonical)) => {
                        match read_mod(folder, String::new(), false) {
                            Ok(m) => {
                                found.push((
                                    folder.to_path_buf(),
                                    if m.name.is_empty() { m.project } else { m.name },
                                ));
                                control.progress.mods += 1;
                            }
                            Err(error) => {
                                warning(&mut result, format!("{}: {error}", path.display()))
                            }
                        }
                    }
                    Err(error) => warning(&mut result, format!("{}: {error}", path.display())),
                    _ => {}
                }
            } else if name == "__vortex_staging_folder" {
                match metadata::read_limited(path, 64 * 1024)
                    .and_then(|bytes| manifest_value(&bytes))
                {
                    Ok(record)
                        if record["game"]
                            .as_str()
                            .is_some_and(|game| game.eq_ignore_ascii_case("inzoi")) =>
                    {
                        add_root(&mut hints, path.parent().unwrap().to_path_buf(), "vortex")
                    }
                    Err(error) => warning(&mut result, format!("{}: {error}", path.display())),
                    _ => {}
                }
            } else if name == "CURRENT"
                && path
                    .parent()
                    .is_some_and(|p| p.file_name().is_some_and(|n| n == "state.v2"))
            {
                let data = path.parent().unwrap().parent().unwrap();
                if data
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("Vortex"))
                    && databases.insert(path_key(data))
                {
                    add_vortex_paths(data, &mut hints, &mut pending, &mut result);
                }
            } else if name.starts_with("vortex.deployment.")
                && path.extension().is_some_and(|ext| ext == "json")
            {
                match metadata::read_limited(path, 16 * 1024 * 1024)
                    .and_then(|bytes| manifest_value(&bytes))
                {
                    Ok(record)
                        if record["gameId"]
                            .as_str()
                            .is_none_or(|game| game.eq_ignore_ascii_case("inzoi")) =>
                    {
                        if let Some(raw) = record["stagingPath"].as_str() {
                            let staging = PathBuf::from(raw);
                            if staging.is_absolute() && staging.is_dir() {
                                add_root(&mut hints, staging.clone(), "vortex");
                                pending.push_back((staging, "vortex".into()));
                            } else {
                                warning(
                                    &mut result,
                                    format!(
                                        "Unavailable Vortex staging folder: {}",
                                        staging.display()
                                    ),
                                );
                            }
                        }
                    }
                    Err(error) => warning(&mut result, format!("{}: {error}", path.display())),
                    _ => {}
                }
            } else if archive_file(path) {
                archives.insert(path.to_path_buf());
                // Other Windows users' default Vortex folders can be found
                // during the disk walk even if they have no readable state DB.
                if let Some(folder) = path.parent() {
                    if folder
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("inzoi"))
                        && folder.parent().is_some_and(|p| {
                            p.file_name()
                                .is_some_and(|n| n.eq_ignore_ascii_case("downloads"))
                                && p.parent().is_some_and(|p| {
                                    p.file_name()
                                        .is_some_and(|n| n.eq_ignore_ascii_case("Vortex"))
                                })
                        })
                    {
                        add_root(&mut hints, folder.to_path_buf(), "vortex-downloads");
                    }
                }
            } else if name.to_ascii_lowercase().ends_with(".meta") {
                // MO2/Nexus sidecars identify the game. Never present arbitrary
                // archives found elsewhere on the disk as inZOI downloads.
                let archive = path.with_file_name(name[..name.len() - 5].to_string());
                if archive_file(&archive) {
                    if let Ok(bytes) = metadata::read_limited(path, 64 * 1024) {
                        let text = String::from_utf8_lossy(&bytes);
                        if text.lines().filter_map(|line| line.split_once('=')).any(
                            |(key, value)| {
                                matches!(
                                    key.trim().to_ascii_lowercase().as_str(),
                                    "gamename" | "game"
                                ) && value.trim().eq_ignore_ascii_case("inzoi")
                            },
                        ) {
                            nexus_archives.insert(archive);
                        }
                    }
                }
            }
            // File-heavy directories must also publish progress and notice
            // cancellation instead of waiting for the next directory.
            control.report(path, false);
        }
        if control.stopped() {
            break;
        }
        completed.insert(key);
        let _ = origin;
    }
    let downloads = nexus::downloads_root(settings);
    let mut groups: BTreeMap<String, DiscoveredFolder> = BTreeMap::new();
    for (path, name) in found {
        let hint = hints
            .iter()
            .filter(|(root, _)| inside(&path, root))
            .max_by_key(|(root, origin)| (origin.starts_with("vortex"), root.as_os_str().len()));
        let (group, origin) = hint
            .map(|(path, origin)| (path.clone(), origin.clone()))
            .unwrap_or((path, "local".into()));
        let raw = dunce::simplified(&group).to_string_lossy().into_owned();
        let already_added = inside(&group, Path::new(&settings.mods_dir))
            || settings.scan_subfolders && inside(&group, Path::new(&settings.library_dir))
            || inside(&group, &downloads)
            || settings
                .source_dirs
                .iter()
                .any(|source| source.scan_subfolders && inside(&group, Path::new(&source.path)));
        groups
            .entry(path_key(&group))
            .or_insert_with(|| DiscoveredFolder {
                path: raw,
                origin,
                mods: vec![],
                already_added,
            })
            .mods
            .push(name);
    }
    for (_, mut group) in groups {
        group.mods.sort_by_key(|m| m.to_lowercase());
        result.folders.push(group);
    }
    for path in archives {
        if nexus_archives.contains(&path)
            || hints
                .iter()
                .any(|(root, origin)| origin == "vortex-downloads" && inside(&path, root))
        {
            result.archives.push(DiscoveredArchive {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                path: dunce::simplified(&path).to_string_lossy().into_owned(),
            });
        }
    }
    result.archives.sort_by_key(|a| a.path.to_ascii_lowercase());
    control.progress.archives = result.archives.len() as u64;
    control.report(Path::new(""), true);
    result.cancelled = control.stopped();
    result.scanned_entries = control.progress.entries;
    result
}

fn register_scan(scans: &ScanState, id: &str) -> Result<Arc<AtomicBool>> {
    uuid::Uuid::parse_str(id).map_err(err)?;
    let mut registry = scans.active.lock().map_err(err)?;
    if !registry.active.is_empty() {
        return Err("A mod scan is already running".into());
    }
    let cancel = Arc::new(AtomicBool::new(registry.cancelled.remove(id)));
    registry.active.insert(id.to_string(), cancel.clone());
    Ok(cancel)
}
fn request_cancel(scans: &ScanState, id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id).map_err(err)?;
    let mut registry = scans.active.lock().map_err(err)?;
    if let Some(cancel) = registry.active.get(id) {
        cancel.store(true, Ordering::Relaxed);
    } else {
        if registry.cancelled.len() >= 128 {
            registry.cancelled.clear();
        }
        registry.cancelled.insert(id.to_string());
    }
    Ok(())
}
#[tauri::command]
pub(crate) fn cancel_discovery(request_id: String, scans: tauri::State<ScanState>) -> Result<()> {
    request_cancel(&scans, &request_id)
}

/// Local drives the user may pick for a deep scan.
#[tauri::command]
pub(crate) fn list_scan_drives() -> Result<Vec<String>> {
    Ok(discovery_drives::roots()?
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Restrict the deep scan to drives the user picked. Only roots the OS
/// reports as local drives are accepted, never arbitrary renderer paths.
fn selected_drives(available: Vec<PathBuf>, chosen: Option<Vec<String>>) -> Result<Vec<PathBuf>> {
    let Some(chosen) = chosen else {
        return Ok(available);
    };
    let picked: Vec<_> = available
        .into_iter()
        .filter(|root| chosen.iter().any(|c| path_key(Path::new(c)) == path_key(root)))
        .collect();
    if picked.is_empty() {
        return Err("Choose at least one drive to scan".into());
    }
    Ok(picked)
}

#[tauri::command(async)]
pub(crate) fn discover_mods(
    app: tauri::AppHandle,
    state: tauri::State<State>,
    scans: tauri::State<ScanState>,
    paths: Option<Vec<String>>,
    all_drives: Option<bool>,
    drives: Option<Vec<String>>,
    request_id: Option<String>,
    on_progress: Channel<ScanProgress>,
) -> Result<Discovery> {
    let settings = state.store.lock().map_err(err)?.settings.clone();
    let id = request_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut control = ScanControl {
        channel: Some(on_progress),
        cancel: register_scan(&scans, &id)?,
        ..Default::default()
    };
    let _guard = ScanGuard { state: &scans, id };
    let full = all_drives.unwrap_or(false) && paths.is_none();
    let (mut roots, warnings) = if let Some(paths) = paths {
        let mut roots = vec![];
        for raw in paths {
            let path = PathBuf::from(raw);
            if !path.is_absolute() || !path.is_dir() {
                return Err(format!(
                    "Choose an existing absolute folder: {}",
                    path.display()
                ));
            }
            add_root(&mut roots, path, "local");
        }
        (roots, vec![])
    } else {
        let dirs: Vec<_> = ["APPDATA", "LOCALAPPDATA", "PROGRAMDATA"]
            .iter()
            .filter_map(|key| std::env::var_os(key).map(|base| PathBuf::from(base).join("Vortex")))
            .collect();
        control.report(Path::new("Vortex"), true);
        let downloads = app.path().download_dir().ok().or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|home| PathBuf::from(home).join("Downloads"))
        });
        if let Some(path) = downloads.as_ref().and_then(|p| p.canonicalize().ok()) {
            control.shallow.push(dunce::simplified(&path).to_path_buf());
        }
        automatic_roots(
            &settings,
            &app.path().document_dir().map_err(err)?,
            downloads.as_deref(),
            &dirs,
        )
    };
    if full {
        roots.extend(
            selected_drives(discovery_drives::roots()?, drives)?
                .into_iter()
                .map(|root| (root, "disk".into())),
        );
    }
    let mut result = discover_with_control(&settings, roots, full, &mut control);
    for text in warnings {
        warning(&mut result, text);
    }
    let mut seen = HashSet::new();
    result
        .warnings
        .retain(|warning| seen.insert(warning.clone()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_disk_walk_finds_deep_mods_and_returns_mod_folders_instead_of_drive_roots() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let mut deep = root.join(".hidden/Unusually named backup");
        for _ in 0..36 {
            deep = deep.join("d");
        }
        copy_tree(&source, &deep).unwrap();
        let settings = state.store.lock().unwrap().settings.clone();
        let quick = discover(&settings, vec![(root.clone(), "disk".into())]);
        assert_eq!(quick.folders.iter().map(|f| f.mods.len()).sum::<usize>(), 1);
        assert!(quick.warnings.iter().any(|w| w.contains("depth limit")));
        let result = discover_with_control(
            &settings,
            vec![(root.clone(), "disk".into())],
            true,
            &mut ScanControl::default(),
        );
        assert_eq!(
            result.folders.iter().map(|f| f.mods.len()).sum::<usize>(),
            2
        );
        assert!(result
            .folders
            .iter()
            .any(|f| library::same_path(&f.path, &deep.to_string_lossy())));
        assert!(result
            .folders
            .iter()
            .all(|f| !library::same_path(&f.path, &root.to_string_lossy())));
        assert!(!result.cancelled);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn moved_vortex_staging_and_nexus_archives_are_detected_without_extracting_other_games() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let staging = root.join("Moved collection");
        copy_tree(&source, &staging.join("Hair")).unwrap();
        fs::write(
            staging.join("__vortex_staging_folder"),
            br#"{"game":"inzoi"}"#,
        )
        .unwrap();
        let downloads = root.join("Vortex/downloads/inzoi");
        fs::create_dir_all(&downloads).unwrap();
        let archive = downloads.join("Hair.7z");
        fs::write(&archive, b"original archive").unwrap();
        fs::write(root.join("Unrelated.zip"), b"other data").unwrap();
        fs::write(root.join("MO2-Hair.zip"), b"download").unwrap();
        fs::write(
            root.join("MO2-Hair.zip.meta"),
            b"[General]\ngameName=inzoi\nmodID=42",
        )
        .unwrap();
        let other_game = root.join("Vortex/downloads/skyrimse");
        fs::create_dir_all(&other_game).unwrap();
        fs::write(other_game.join("Other.zip"), b"other game").unwrap();
        let settings = state.store.lock().unwrap().settings.clone();
        let result = discover_with_control(
            &settings,
            vec![(root.clone(), "disk".into())],
            true,
            &mut ScanControl::default(),
        );
        let found = result
            .folders
            .iter()
            .find(|f| library::same_path(&f.path, &staging.to_string_lossy()))
            .unwrap();
        assert_eq!(found.origin, "vortex");
        assert_eq!(found.mods.len(), 1);
        assert_eq!(result.archives.len(), 2);
        assert!(result.archives.iter().any(|a| a.name == "Hair.7z"));
        assert!(result.archives.iter().any(|a| a.name == "MO2-Hair.zip"));
        assert_eq!(fs::read(archive).unwrap(), b"original archive");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn drive_selection_keeps_only_chosen_known_drives() {
        let available = vec![PathBuf::from("C:\\"), PathBuf::from("D:\\")];
        assert_eq!(
            selected_drives(available.clone(), Some(vec!["d:/".into(), "Z:\\".into()])).unwrap(),
            vec![PathBuf::from("D:\\")]
        );
        assert!(selected_drives(available.clone(), Some(vec!["Z:\\".into()])).is_err());
        assert_eq!(selected_drives(available.clone(), None).unwrap(), available);
    }

    #[test]
    fn quick_scan_only_looks_a_few_levels_into_the_downloads_folder() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let downloads = root.join("DownloadsLike");
        copy_tree(&source, &downloads.join("Shallow")).unwrap();
        copy_tree(&source, &downloads.join("a/b/c/d/e/Deep")).unwrap();
        let settings = state.store.lock().unwrap().settings.clone();
        let mut control = ScanControl::default();
        control.shallow.push(downloads.canonicalize().unwrap());
        let result = discover_with_control(
            &settings,
            vec![(downloads.clone(), "local".into())],
            false,
            &mut control,
        );
        assert_eq!(result.folders.iter().map(|f| f.mods.len()).sum::<usize>(), 1);
        assert!(result.warnings.iter().all(|w| !w.contains("depth limit")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_before_dispatch_is_honoured_and_scan_guard_releases_the_next_run() {
        let scans = ScanState::default();
        let id = uuid::Uuid::new_v4().to_string();
        request_cancel(&scans, &id).unwrap();
        let cancel = register_scan(&scans, &id).unwrap();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(register_scan(&scans, &uuid::Uuid::new_v4().to_string()).is_err());
        {
            let _guard = ScanGuard { state: &scans, id };
            let (root, state, _) = crate::lifecycle_tests::fixture();
            let settings = state.store.lock().unwrap().settings.clone();
            let result = discover_with_control(
                &settings,
                vec![(root.clone(), "disk".into())],
                true,
                &mut ScanControl {
                    cancel,
                    ..Default::default()
                },
            );
            assert!(result.cancelled);
            assert_eq!(result.scanned_entries, 0);
            assert!(result.folders.is_empty());
            fs::remove_dir_all(root).unwrap();
        }
        let next = uuid::Uuid::new_v4().to_string();
        assert!(!register_scan(&scans, &next)
            .unwrap()
            .load(Ordering::Relaxed));
        request_cancel(&scans, &next).unwrap();
        assert!(scans.active.lock().unwrap().active[&next].load(Ordering::Relaxed));
    }

    #[cfg(windows)]
    #[test]
    fn full_scan_skips_a_junction_cycle_without_touching_its_target() {
        use std::os::windows::process::CommandExt;
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let junction = root.join("cycle");
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&root)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let settings = state.store.lock().unwrap().settings.clone();
        let result = discover_with_control(
            &settings,
            vec![(root.clone(), "disk".into())],
            true,
            &mut ScanControl::default(),
        );
        assert_eq!(
            result.folders.iter().map(|f| f.mods.len()).sum::<usize>(),
            1
        );
        assert!(result.scanned_entries < 50);
        assert_eq!(fs::read(source.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        fs::remove_dir(junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn custom_vortex_staging_is_read_from_copy_and_original_is_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("Vortex");
        let db_path = data.join("state.v2");
        fs::create_dir_all(&data).unwrap();
        {
            let mut db = rusty_leveldb::DB::open(&db_path, Default::default()).unwrap();
            db.put(
                b"settings###mods###installPath###inzoi",
                br#""{USERDATA}/custom/{GAME}""#,
            )
            .unwrap();
            db.put(
                b"settings###downloads###path",
                br#""{USERDATA}/custom-downloads""#,
            )
            .unwrap();
            db.flush().unwrap();
        }
        let before: Vec<_> = fs::read_dir(&db_path)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name(), fs::read(e.path()).unwrap())
            })
            .collect();
        assert_eq!(
            vortex_staging(&data).unwrap(),
            Some(data.join("custom/inzoi"))
        );
        assert_eq!(
            vortex_paths(&data).unwrap().1,
            Some(data.join("custom-downloads/inzoi"))
        );
        for (name, bytes) in before {
            assert_eq!(fs::read(db_path.join(name)).unwrap(), bytes);
        }
    }
    #[test]
    fn discovery_is_read_only_deduplicates_and_preserves_disabled_mods() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let ignored = root.join(".trash/Old");
        fs::create_dir_all(&ignored).unwrap();
        fs::copy(
            source.join("mod_manifest.json"),
            ignored.join("mod_manifest.json"),
        )
        .unwrap();
        let settings = state.store.lock().unwrap().settings.clone();
        let mut manifest =
            manifest_value(&fs::read(source.join("mod_manifest.json")).unwrap()).unwrap();
        manifest["bEnable"] = false.into();
        fs::write(source.join("mod_manifest.json"), manifest.to_string()).unwrap();
        let before = fs::read(source.join("mod_manifest.json")).unwrap();
        let result = discover(
            &settings,
            vec![
                (root.clone(), "vortex".into()),
                (source.clone(), "local".into()),
            ],
        );
        assert_eq!(result.folders.len(), 1);
        assert_eq!(result.folders[0].mods.len(), 1);
        assert_eq!(result.folders[0].origin, "vortex");
        assert_eq!(fs::read(source.join("mod_manifest.json")).unwrap(), before);
        assert!(state.store.lock().unwrap().settings.source_dirs.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deployment_manifest_finds_custom_staging_without_a_database() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let game = root.join("game-scan");
        fs::create_dir_all(&game).unwrap();
        fs::write(
            game.join("vortex.deployment.json"),
            serde_json::json!({"gameId":"inzoi", "stagingPath":source.parent().unwrap()})
                .to_string(),
        )
        .unwrap();
        let settings = state.store.lock().unwrap().settings.clone();
        let result = discover(&settings, vec![(game, "game".into())]);
        assert_eq!(result.folders.len(), 1);
        assert_eq!(result.folders[0].origin, "vortex");
        assert_eq!(result.folders[0].mods.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_nonrecursive_source_can_be_selected_to_include_nested_mods() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let mut settings = state.store.lock().unwrap().settings.clone();
        let external = root.join("external/Category/Hair");
        copy_tree(&source, &external).unwrap();
        let path = external.parent().unwrap().to_path_buf();
        settings.source_dirs.push(SourceFolder {
            path: path.to_string_lossy().into_owned(),
            scan_subfolders: false,
        });
        let result = discover(&settings, vec![(path.clone(), "local".into())]);
        assert!(!result.folders[0].already_added);
        settings.source_dirs[0].scan_subfolders = true;
        assert!(discover(&settings, vec![(path, "local".into())]).folders[0].already_added);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn broken_vortex_database_warns_and_keeps_default_staging_available() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("Vortex");
        fs::create_dir_all(data.join("state.v2")).unwrap();
        fs::write(data.join("state.v2/CURRENT"), b"not-a-manifest\n").unwrap();
        fs::create_dir_all(data.join("inzoi/mods")).unwrap();
        let (_, state, _) = crate::lifecycle_tests::fixture();
        let settings = state.store.lock().unwrap().settings.clone();
        let (roots, warnings) = automatic_roots(&settings, root.path(), None, &[data.clone()]);
        assert!(warnings
            .iter()
            .any(|warning| warning.starts_with("Vortex (")));
        assert!(roots
            .iter()
            .any(|(path, origin)| *path == data.join("inzoi/mods") && origin == "vortex"));
        assert_eq!(
            fs::read(data.join("state.v2/CURRENT")).unwrap(),
            b"not-a-manifest\n"
        );
        let fixture_root = state.file.parent().unwrap();
        fs::remove_dir_all(fixture_root).unwrap();
    }

    #[test]
    #[ignore = "Read-only scan of the local machine; set INZOIMM_DISCOVERY_DOCUMENTS"]
    fn scan_local_machine() {
        let (_, state, _) = crate::lifecycle_tests::fixture();
        let settings = state.store.lock().unwrap().settings.clone();
        let documents = PathBuf::from(std::env::var("INZOIMM_DISCOVERY_DOCUMENTS").unwrap());
        let dirs: Vec<_> = ["APPDATA", "LOCALAPPDATA", "PROGRAMDATA"]
            .iter()
            .filter_map(|key| std::env::var_os(key).map(|base| PathBuf::from(base).join("Vortex")))
            .collect();
        let (roots, issues) = automatic_roots(&settings, &documents, None, &dirs);
        let result = discover(&settings, roots);
        for folder in result.folders {
            println!(
                "{}: {} mods ({})",
                folder.path,
                folder.mods.len(),
                folder.origin
            );
        }
        for warning in issues.into_iter().chain(result.warnings) {
            println!("Warning: {warning}");
        }
        let fixture_root = state.file.parent().unwrap();
        fs::remove_dir_all(fixture_root).unwrap();
    }

    #[test]
    #[ignore = "Read-only scan of all local drives, cancelled after 10 seconds; set INZOIMM_SCAN_ALL=1"]
    fn full_drive_smoke_stops_and_keeps_partial_results() {
        assert_eq!(std::env::var("INZOIMM_SCAN_ALL").as_deref(), Ok("1"));
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let settings = state.store.lock().unwrap().settings.clone();
        let drives = discovery_drives::roots().unwrap();
        assert!(!drives.is_empty());
        println!(
            "Local scan roots: {}",
            drives
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut control = ScanControl::default();
        let cancel = control.cancel.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(10));
            cancel.store(true, Ordering::Relaxed)
        });
        let result = discover_with_control(
            &settings,
            drives.into_iter().map(|p| (p, "disk".into())).collect(),
            true,
            &mut control,
        );
        stopper.join().unwrap();
        println!(
            "Read-only full-drive scan: {} entries, {} mod folders, {} archives, cancelled={}",
            result.scanned_entries,
            result.folders.len(),
            result.archives.len(),
            result.cancelled
        );
        assert!(result.scanned_entries > 0);
        assert!(result.cancelled);
        fs::remove_dir_all(root).unwrap();
    }
}
