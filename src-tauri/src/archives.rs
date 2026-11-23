//! Installing downloaded or chosen archives (ZIP, 7z, RAR) and loose folders.
//!
//! An archive can hold three kinds of content, handled separately:
//! - folders with `mod_manifest.json` are imported into the library as before;
//! - loose legacy PAK sets (`.pak` with `.utoc`/`.ucas`/`.sig`) are copied into
//!   the configured `~mods` folder, disabled, below one folder per archive;
//! - the inZOI Mod Enabler (`dsound.dll` and its helpers) is copied into the
//!   game's `BlueClient/Binaries/Win64`, only after explicit confirmation.
use super::*;
use std::collections::BTreeMap;
use std::io::Read;

const MAX_ENTRIES: usize = 20_000;
const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Prefix of the error returned when an archive contains the Mod Enabler and
/// the caller did not confirm writing it into the game folder.
pub(crate) const ENABLER_CONFIRM: &str = "modEnablerConfirm:";
/// Beside a downloaded archive: what was installed from it outside the library.
/// Prefix of the error returned when nothing installable was found; the
/// download queue shows such archives as needing manual extraction.
pub(crate) const NOTHING_INSTALLABLE: &str = "noInstallableMod:";
pub(crate) const INSTALL_RECORD: &str = ".inzoimm-installed.json";
const IMAGES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

#[derive(Serialize, Default, Debug, Clone, PartialEq)]
pub(crate) struct InstallReport {
    /// Library mods (with `mod_manifest.json`) that were imported.
    imported: usize,
    /// Legacy PAK sets copied to `~mods` (disabled).
    legacy: usize,
    /// The inZOI Mod Enabler was written into the game folder.
    enabler: bool,
    /// Legacy mods were installed but the Mod Enabler is not in the game.
    needs_enabler: bool,
}

impl InstallReport {
    pub(crate) fn imported(count: usize) -> Self {
        Self {
            imported: count,
            ..Self::default()
        }
    }
}

pub(crate) fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub(crate) fn supported(path: &Path) -> bool {
    matches!(extension(path).as_str(), "zip" | "7z" | "rar")
}

struct Budget {
    entries: usize,
    bytes: u64,
}
impl Budget {
    fn new() -> Self {
        Self {
            entries: 0,
            bytes: 0,
        }
    }
    fn add(&mut self, size: u64) -> Result<()> {
        self.entries += 1;
        if self.entries > MAX_ENTRIES {
            return Err("Archive contains too many files (limit 20,000)".into());
        }
        self.bytes = self.bytes.checked_add(size).ok_or("Archive is too large")?;
        if self.bytes > MAX_BYTES {
            return Err("Archive exceeds the 8 GiB extraction limit".into());
        }
        Ok(())
    }
}

/// Validates an archive entry name and returns it as a relative path.
fn relative(name: &str) -> Result<PathBuf> {
    if name.starts_with(['/', '\\']) {
        return Err("Archive contains an unsafe path".into());
    }
    let mut path = PathBuf::new();
    for part in name.split(['/', '\\']).filter(|p| !p.is_empty()) {
        if !safe_project(part) {
            return Err("Archive contains an invalid file name".into());
        }
        path.push(part);
    }
    if path.as_os_str().is_empty() {
        return Err("Archive contains an empty file name".into());
    }
    Ok(path)
}

fn write_entry(reader: &mut dyn Read, out: &Path, declared: u64) -> Result<()> {
    fs::create_dir_all(out.parent().ok_or("Invalid entry path")?).map_err(err)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out)
        .map_err(err)?;
    let actual = std::io::copy(&mut reader.take(declared + 1), &mut file).map_err(err)?;
    if actual != declared {
        return Err("Archive entry size mismatch".into());
    }
    Ok(())
}

fn extract_7z(source: &Path, dest: &Path) -> Result<()> {
    let mut reader =
        sevenz_rust2::ArchiveReader::open(source, sevenz_rust2::Password::empty()).map_err(err)?;
    let mut budget = Budget::new();
    for entry in &reader.archive().files {
        relative(entry.name())?;
        budget.add(entry.size())?;
    }
    let mut failure = None;
    reader
        .for_each_entries(|entry, data| {
            let result = relative(entry.name()).and_then(|rel| {
                let out = dest.join(rel);
                if entry.is_directory() {
                    fs::create_dir_all(out).map_err(err)
                } else {
                    write_entry(data, &out, entry.size())
                }
            });
            match result {
                Ok(()) => Ok(true),
                Err(error) => {
                    failure = Some(error);
                    Ok(false)
                }
            }
        })
        .map_err(err)?;
    failure.map_or(Ok(()), Err)
}

fn extract_rar(source: &Path, dest: &Path) -> Result<()> {
    let mut budget = Budget::new();
    for header in unrar::Archive::new(source)
        .open_for_listing()
        .map_err(err)?
    {
        let header = header.map_err(err)?;
        if header.is_encrypted() {
            return Err("Password-protected archives are not supported".into());
        }
        if header.is_split() {
            return Err("Multi-part RAR archives are not supported; extract them manually".into());
        }
        relative(&header.filename.to_string_lossy())?;
        budget.add(header.unpacked_size)?;
    }
    let mut archive = unrar::Archive::new(source)
        .open_for_processing()
        .map_err(err)?;
    while let Some(header) = archive.read_header().map_err(err)? {
        let entry = header.entry();
        let out = dest.join(relative(&entry.filename.to_string_lossy())?);
        archive = if entry.is_directory() {
            fs::create_dir_all(&out).map_err(err)?;
            header.skip().map_err(err)?
        } else {
            if out.exists() {
                return Err("Archive contains a duplicate file name".into());
            }
            let declared = entry.unpacked_size;
            fs::create_dir_all(out.parent().ok_or("Invalid entry path")?).map_err(err)?;
            let next = header.extract_to(&out).map_err(err)?;
            let written = fs::symlink_metadata(&out).map_err(err)?;
            // Checked per entry, so no later entry can be written through a link.
            if written.file_type().is_symlink() {
                return Err("Archive symbolic links are not supported".into());
            }
            if written.len() != declared {
                return Err("Archive entry size mismatch".into());
            }
            next
        };
    }
    Ok(())
}

/// Extracts a ZIP, 7z or RAR archive into the empty folder `dest`.
pub(crate) fn extract(source: &Path, dest: &Path) -> Result<()> {
    match extension(source).as_str() {
        "zip" => extract_zip(source, dest)?,
        "7z" => extract_7z(source, dest)?,
        "rar" => extract_rar(source, dest)?,
        _ => return Err("Only ZIP, 7z and RAR archives can be installed".into()),
    }
    // Whatever the format, nothing extracted may point outside `dest`.
    for entry in WalkDir::new(dest).follow_links(false) {
        if entry.map_err(err)?.path_is_symlink() {
            return Err("Archive symbolic links are not supported".into());
        }
    }
    Ok(())
}

struct PakSet {
    dir: PathBuf,
    files: Vec<PathBuf>,
}

fn has_component(path: &Path, name: &str) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(name))
}

/// True for files below a `Paks` folder other than `Paks/~mods`: the game's
/// own packages, DLC and script mods, which are never copied.
fn game_package(rel: &Path) -> bool {
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    parts
        .iter()
        .position(|p| p == "paks")
        .is_some_and(|index| parts.get(index + 1).is_none_or(|next| next != "~mods"))
}

/// Legacy PAK sets in `root` that do not belong to a manifest mod. Game-side
/// folders the app does not manage (`LogicMods`, UE4SS, the game's own paks)
/// are ignored.
fn pak_sets(root: &Path, manifests: &[PathBuf]) -> Vec<PakSet> {
    let mut sets: BTreeMap<(PathBuf, String), (bool, Vec<PathBuf>)> = BTreeMap::new();
    let mut images: Vec<(PathBuf, String, PathBuf)> = vec![];
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(path);
        if manifests.iter().any(|m| path.starts_with(m))
            || has_component(rel, "LogicMods")
            || has_component(rel, "ue4ss")
            || game_package(rel)
        {
            continue;
        }
        let dir = path.parent().unwrap_or(root).to_path_buf();
        let name = entry.file_name().to_string_lossy();
        if let Some((stem, ext, _)) = legacy::classify(&name) {
            let set = sets
                .entry((dir, stem.to_lowercase()))
                .or_insert((false, vec![]));
            set.0 |= ext == "pak";
            set.1.push(path.to_path_buf());
        } else if let Some((stem, ext)) = name.rsplit_once('.') {
            if IMAGES.contains(&ext.to_ascii_lowercase().as_str()) {
                images.push((dir, stem.to_lowercase(), path.to_path_buf()));
            }
        }
    }
    for (dir, stem, image) in images {
        if let Some(set) = sets.get_mut(&(dir, stem)) {
            set.1.push(image);
        }
    }
    sets.into_iter()
        .filter(|(_, (has_pak, _))| *has_pak)
        .map(|((dir, _), (_, files))| PakSet { dir, files })
        .collect()
}

/// A folder name derived from the archive or folder name.
fn folder_name(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']).to_string();
    if safe_project(&clean) {
        clean
    } else {
        "Legacy PAK mod".into()
    }
}

/// Where each set goes below `~mods/<base>`: the part after a `~mods` folder
/// when the archive mirrors the game layout, without a shared wrapper folder.
fn targets(root: &Path, sets: &[PakSet]) -> Vec<PathBuf> {
    let parts: Vec<Vec<String>> = sets
        .iter()
        .map(|set| {
            let rel: Vec<String> = set
                .dir
                .strip_prefix(root)
                .unwrap_or(&set.dir)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            match rel.iter().rposition(|p| p.eq_ignore_ascii_case("~mods")) {
                Some(index) => rel[index + 1..].to_vec(),
                None => rel,
            }
        })
        .collect();
    let mut shared = parts.first().map_or(0, Vec::len);
    for p in &parts {
        shared = shared.min(
            p.iter()
                .zip(&parts[0])
                .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
                .count(),
        );
    }
    parts
        .into_iter()
        .map(|p| p[shared..].iter().collect())
        .collect()
}

fn existing_stems(paks: &Path) -> BTreeMap<String, String> {
    legacy::scan(paks)
        .0
        .into_iter()
        .map(|m| (m.project.to_lowercase(), m.path))
        .collect()
}

/// Copies legacy PAK sets into `~mods/<base>/…` with a `_disabled` suffix.
/// Nothing is overwritten; on failure every copied file is removed again.
/// Returns the installed `.pak` paths without the suffix.
fn install_paks(root: &Path, sets: &[PakSet], paks: &Path, base: &str) -> Result<Vec<PathBuf>> {
    let installed = existing_stems(paks);
    let base_dir = paks.join(base);
    let mut plan: Vec<(PathBuf, PathBuf)> = vec![];
    let mut paks_installed = vec![];
    for (set, rel) in sets.iter().zip(targets(root, sets)) {
        let dir = base_dir.join(rel);
        for file in &set.files {
            let name = file
                .file_name()
                .ok_or("Invalid file name")?
                .to_string_lossy();
            let target = match legacy::classify(&name) {
                Some((stem, ext, disabled)) => {
                    if let Some(path) = installed.get(&stem.to_lowercase()) {
                        return Err(format!(
                            "A PAK mod named {stem} is already in ~mods ({path}). Remove it first; nothing was installed."
                        ));
                    }
                    let plain = dir.join(format!("{stem}.{ext}"));
                    if ext == "pak" {
                        paks_installed.push(plain.clone());
                    }
                    if disabled {
                        dir.join(name.as_ref())
                    } else {
                        dir.join(format!("{name}{}", legacy::DISABLED))
                    }
                }
                None => dir.join(name.as_ref()),
            };
            let plain = target.to_string_lossy();
            let plain = plain.strip_suffix(legacy::DISABLED).unwrap_or(&plain);
            if target.exists() || Path::new(plain).exists() {
                return Err(format!(
                    "{} already exists. It was not overwritten; nothing was installed.",
                    target.display()
                ));
            }
            plan.push((file.clone(), target));
        }
    }
    let base_existed = base_dir.exists();
    let mut copied: Vec<PathBuf> = vec![];
    let rollback = |copied: &[PathBuf]| {
        for file in copied.iter().rev() {
            let _ = fs::remove_file(file);
        }
        if !base_existed {
            let _ = fs::remove_dir_all(&base_dir);
        }
    };
    for (from, to) in &plan {
        let result = to
            .parent()
            .ok_or_else(|| "Invalid target".to_string())
            .and_then(|dir| fs::create_dir_all(dir).map_err(|e| legacy::io_message(e, dir)))
            .and_then(|_| fs::copy(from, to).map_err(|e| legacy::io_message(e, to)));
        if let Err(error) = result {
            rollback(&copied);
            return Err(error);
        }
        copied.push(to.clone());
    }
    Ok(paks_installed)
}

/// The Mod Enabler's files and the game-relative folder they map to.
fn enabler_layout(root: &Path) -> Result<Option<(PathBuf, PathBuf)>> {
    let dlls: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("dsound.dll")
        })
        .map(|e| e.into_path())
        .collect();
    let dll = match dlls.as_slice() {
        [] => return Ok(None),
        [dll] => dll,
        _ => {
            return Err("The archive contains more than one dsound.dll; install it manually".into())
        }
    };
    let named = |path: Option<&Path>, name: &str| {
        path.and_then(Path::file_name)
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
    };
    let win64 = dll.parent().ok_or("Invalid archive layout")?;
    let binaries = win64.parent();
    let blue = binaries.and_then(Path::parent);
    // Only the game-side subtree is copied: a LICENSE or readme next to
    // `BlueClient` in the archive does not belong in the game folder.
    Ok(Some(
        if named(Some(win64), "Win64") && named(binaries, "Binaries") {
            match (blue, binaries) {
                (Some(blue), _) if named(Some(blue), "BlueClient") => {
                    (blue.to_path_buf(), PathBuf::from("BlueClient"))
                }
                (_, Some(binaries)) => {
                    (binaries.to_path_buf(), PathBuf::from("BlueClient/Binaries"))
                }
                _ => (
                    win64.to_path_buf(),
                    PathBuf::from("BlueClient/Binaries/Win64"),
                ),
            }
        } else {
            (
                win64.to_path_buf(),
                PathBuf::from("BlueClient/Binaries/Win64"),
            )
        },
    ))
}

/// Copies the Mod Enabler into the game. Existing files that would be
/// replaced are first moved to `Content/.inzoimm-trash`, and restored if a
/// later copy fails. Returns the written files.
fn install_enabler(source: &Path, target_rel: &Path, paks: &Path) -> Result<Vec<PathBuf>> {
    let game = legacy::game_root(paks)
        .filter(|game| game.join("BlueClient").is_dir())
        .ok_or("Set the inZOI ~mods folder in Settings first, so the game folder is known")?;
    let target_root = game.join(target_rel);
    let trash = legacy::trash_dir(paks)?.join(uuid::Uuid::new_v4().to_string());
    let mut backups: Vec<(PathBuf, PathBuf)> = vec![];
    let mut written: Vec<PathBuf> = vec![];
    let restore = |backups: &[(PathBuf, PathBuf)], written: &[PathBuf]| {
        for file in written.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for (original, backup) in backups.iter().rev() {
            let _ = fs::rename(backup, original);
        }
    };
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(err)?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(source).map_err(err)?;
        let to = target_root.join(rel);
        let step = (|| -> Result<()> {
            if to.is_dir() {
                return Err(format!("{} is a folder; it was not replaced", to.display()));
            }
            if to.exists() {
                let backup = trash.join(to.strip_prefix(&game).unwrap_or(rel));
                fs::create_dir_all(backup.parent().ok_or("Invalid backup path")?).map_err(err)?;
                fs::rename(&to, &backup).map_err(|e| legacy::io_message(e, &to))?;
                backups.push((to.clone(), backup));
            }
            let dir = to.parent().ok_or("Invalid target")?;
            fs::create_dir_all(dir).map_err(|e| legacy::io_message(e, dir))?;
            fs::copy(entry.path(), &to).map_err(|e| legacy::io_message(e, &to))?;
            written.push(to.clone());
            Ok(())
        })();
        if let Err(error) = step {
            restore(&backups, &written);
            return Err(error);
        }
    }
    Ok(written)
}

fn manifest_dirs(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("mod_manifest.json")
        })
        .filter_map(|e| e.path().parent().map(Path::to_path_buf))
        .collect()
}

pub(crate) struct Source<'a> {
    /// Extracted archive or chosen folder, never modified.
    pub root: &'a Path,
    /// Folder name for legacy PAK sets below `~mods`.
    pub name: &'a str,
    /// Library destination for downloads (see `import_folder_to_library`).
    pub destination: Option<(PathBuf, PathBuf)>,
    pub recursive: bool,
    pub allow_enabler: bool,
    /// Copy every PAK set into `~mods`, including those of `mod_manifest.json`
    /// mods, instead of importing them into the library.
    pub legacy_only: bool,
}

/// Installs everything recognised in `source.root`. Returns what was done and
/// the paths written outside the library (for the archive's install record).
pub(crate) fn install(source: Source, state: &State) -> Result<(InstallReport, Vec<PathBuf>)> {
    let settings = state.store.lock().map_err(err)?.settings.clone();
    let paks = PathBuf::from(settings.paks());
    let mut report = InstallReport::default();
    if let Some((enabler_root, target)) = enabler_layout(source.root)? {
        if settings.paks().is_empty() {
            return Err("This is the inZOI Mod Enabler. Set the inZOI ~mods folder in Settings first, so the game folder is known.".into());
        }
        if !source.allow_enabler {
            return Err(format!(
                "{ENABLER_CONFIRM}This archive contains the inZOI Mod Enabler, which is installed into the game folder."
            ));
        }
        let written = install_enabler(&enabler_root, &target, &paks)?;
        report.enabler = true;
        return Ok((report, written));
    }
    let manifests = if source.legacy_only {
        vec![]
    } else {
        manifest_dirs(source.root)
    };
    let sets = pak_sets(source.root, &manifests);
    if source.legacy_only && sets.is_empty() {
        return Err(format!("{NOTHING_INSTALLABLE}There are no .pak files in this package to install as legacy PAK mods."));
    }
    if manifests.is_empty() && sets.is_empty() {
        return Err(format!("{NOTHING_INSTALLABLE}No installable inZOI mod found: there is no mod_manifest.json and no .pak file (LogicMods/UE4SS packages need a manual installation)."));
    }
    let mut written = vec![];
    if !sets.is_empty() {
        if settings.paks().is_empty() {
            return Err("This package contains legacy PAK mods. Set the inZOI ~mods folder in Settings first.".into());
        }
        let paks = legacy::normalize(&paks)?;
        fs::create_dir_all(&paks).map_err(|e| legacy::io_message(e, &paks))?;
        written = install_paks(source.root, &sets, &paks, &folder_name(source.name))?;
        report.legacy = sets.len();
    }
    if !manifests.is_empty() {
        let before = state.store.lock().map_err(err)?.mods.len();
        let result = import_folder_to_library(
            source.root.to_string_lossy().into_owned(),
            source.recursive,
            state,
            source.destination,
        );
        if let Err(error) = result {
            if report.legacy > 0 {
                remove_installed(&paks, &written, source.name);
            }
            return Err(error);
        }
        report.imported = state
            .store
            .lock()
            .map_err(err)?
            .mods
            .len()
            .saturating_sub(before)
            .max(1);
    }
    report.needs_enabler = report.legacy > 0 && legacy::enabler_installed(&settings) == Some(false);
    Ok((report, written))
}

/// Undoes `install_paks` after a later step failed.
fn remove_installed(paks: &Path, written: &[PathBuf], name: &str) {
    for pak in written {
        let Some(dir) = pak.parent() else { continue };
        let Some(stem) = pak.file_stem().map(|s| s.to_string_lossy().to_lowercase()) else {
            continue;
        };
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file = entry.file_name().to_string_lossy().to_lowercase();
                if file.rsplit_once('.').is_some_and(|(s, _)| s == stem) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
    let base = paks.join(folder_name(name));
    if WalkDir::new(&base)
        .into_iter()
        .filter_map(|e| e.ok())
        .all(|e| !e.file_type().is_file())
    {
        let _ = fs::remove_dir_all(base);
    }
}

fn archive_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Extracts `path` to a temporary folder and installs its contents.
pub(crate) fn install_archive(
    path: &str,
    state: &State,
    destination: Option<(PathBuf, PathBuf)>,
    allow_enabler: bool,
) -> Result<InstallReport> {
    install_archive_as(path, state, destination, allow_enabler, false)
}

/// `legacy_only`: the user chose to install the package as legacy PAK mods.
pub(crate) fn install_archive_as(
    path: &str,
    state: &State,
    destination: Option<(PathBuf, PathBuf)>,
    allow_enabler: bool,
    legacy_only: bool,
) -> Result<InstallReport> {
    let archive = Path::new(path);
    if !supported(archive) {
        return Err("Only ZIP, 7z and RAR archives can be installed".into());
    }
    let temp = state
        .file
        .parent()
        .ok_or("Missing app data folder")?
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&temp).map_err(err)?;
    let is_download = destination.is_some();
    let result = extract(archive, &temp)
        .and_then(|_| metadata::attach_download(archive, &temp))
        .and_then(|_| {
            install(
                Source {
                    root: &temp,
                    name: &archive_name(archive),
                    destination,
                    recursive: true,
                    allow_enabler,
                    legacy_only,
                },
                state,
            )
        });
    let cleanup = fs::remove_dir_all(&temp).map_err(err);
    let (report, written) = result?;
    cleanup?;
    if is_download && !written.is_empty() {
        write_record(archive, &written)?;
    }
    Ok(report)
}

fn write_record(archive: &Path, written: &[PathBuf]) -> Result<()> {
    let folder = archive.parent().ok_or("Missing archive folder")?;
    let record = serde_json::json!({
        "archive": archive.file_name().map(|n| n.to_string_lossy().into_owned()),
        "files": written.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
    });
    fs::write(
        folder.join(INSTALL_RECORD),
        serde_json::to_vec_pretty(&record).map_err(err)?,
    )
    .map_err(err)
}

/// True when files installed from the archive in `folder` are still present
/// (enabled or disabled), so its "not installed" card can be hidden.
pub(crate) fn installed_from(folder: &Path) -> bool {
    let Ok(bytes) = metadata::read_limited(&folder.join(INSTALL_RECORD), 2 * 1024 * 1024) else {
        return false;
    };
    let Ok(record) = manifest_value(&bytes) else {
        return false;
    };
    record["files"].as_array().is_some_and(|files| {
        files.iter().filter_map(|f| f.as_str()).any(|f| {
            Path::new(f).is_file() || Path::new(&format!("{f}{}", legacy::DISABLED)).is_file()
        })
    })
}

/// "Add mod" on a folder without `mod_manifest.json`: install loose PAK sets.
pub(crate) fn install_folder(path: &str, recursive: bool, state: &State) -> Result<InstallReport> {
    let src = PathBuf::from(path).canonicalize().map_err(err)?;
    if !src.is_dir() {
        return Err("Choose a folder containing mod_manifest.json or .pak files".into());
    }
    let paks = state.store.lock().map_err(err)?.settings.paks().to_string();
    if !paks.is_empty() {
        let paks = PathBuf::from(&paks);
        let paks = paks.canonicalize().unwrap_or(paks);
        let game = legacy::game_root(&paks).unwrap_or_else(|| paks.clone());
        if paks.starts_with(&src) || src.starts_with(&paks) || src.starts_with(&game) {
            return Err("Choose a folder outside the inZOI game folder; PAK mods already in ~mods are listed automatically.".into());
        }
    }
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    install(
        Source {
            root: &src,
            name: &name,
            destination: None,
            recursive,
            allow_enabler: false,
            legacy_only: false,
        },
        state,
    )
    .map(|(report, _)| report)
}

/// Settings: install the Mod Enabler from a chosen archive.
#[tauri::command(async)]
pub(crate) fn install_mod_enabler(
    path: String,
    state: tauri::State<State>,
) -> Result<InstallReport> {
    install_mod_enabler_core(&path, &state)
}

pub(crate) fn install_mod_enabler_core(path: &str, state: &State) -> Result<InstallReport> {
    let archive = Path::new(path);
    if !supported(archive) {
        return Err("Choose the Mod Enabler ZIP, 7z or RAR archive".into());
    }
    let temp = state
        .file
        .parent()
        .ok_or("Missing app data folder")?
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&temp).map_err(err)?;
    let result = extract(archive, &temp).and_then(|_| {
        if enabler_layout(&temp)?.is_none() {
            return Err("This archive does not contain the inZOI Mod Enabler (dsound.dll)".into());
        }
        install(
            Source {
                root: &temp,
                name: &archive_name(archive),
                destination: None,
                recursive: true,
                allow_enabler: true,
                legacy_only: false,
            },
            state,
        )
    });
    let cleanup = fs::remove_dir_all(&temp).map_err(err);
    let (report, _) = result?;
    cleanup?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    /// A store whose `~mods` folder lives in a fake game installation.
    fn game_fixture() -> (PathBuf, State, PathBuf, PathBuf) {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let game = root.join("inZOI");
        let paks = game.join("BlueClient/Content/Paks/~mods");
        fs::create_dir_all(&paks).unwrap();
        state.store.lock().unwrap().settings.paks_dir = Some(paks.to_string_lossy().into_owned());
        (root, state, game, paks)
    }

    #[test]
    fn loose_pak_zip_installs_disabled_into_mods_without_overwriting() {
        let (root, state, _, paks) = game_fixture();
        let archive = root.join("Pretty Hair-12-1-0.zip");
        zip(
            &archive,
            &[
                ("Pretty Hair/~mods/Hair_P.pak", b"pak"),
                ("Pretty Hair/~mods/Hair_P.utoc", b"utoc"),
                ("Pretty Hair/~mods/Hair_P.ucas", b"ucas"),
                ("Pretty Hair/~mods/Hair_P.png", b"img"),
                ("Pretty Hair/readme.txt", b"hi"),
            ],
        );
        let report = install_archive(&archive.to_string_lossy(), &state, None, false).unwrap();
        assert_eq!(report.legacy, 1);
        assert!(report.needs_enabler);
        let dir = paks.join("Pretty Hair-12-1-0");
        for ext in ["pak", "utoc", "ucas"] {
            assert!(dir.join(format!("Hair_P.{ext}_disabled")).is_file());
        }
        assert!(dir.join("Hair_P.png").is_file());
        assert!(!dir.join("readme.txt").exists());
        let found = legacy::scan(&paks).0;
        assert_eq!(found.len(), 1);
        assert!(!found[0].enabled && found[0].thumbnail.is_some());
        // A second install of the same PAK name is refused and changes nothing.
        let again = install_archive(&archive.to_string_lossy(), &state, None, false);
        assert!(again.unwrap_err().contains("already in ~mods"));
        assert_eq!(fs::read(dir.join("Hair_P.pak_disabled")).unwrap(), b"pak");
    }

    #[test]
    fn variants_keep_their_folders_and_manifest_paks_stay_in_the_library() {
        let (root, state, _, paks) = game_fixture();
        let archive = root.join("Outfit.zip");
        zip(
            &archive,
            &[
                ("Outfit/Option A/Dress.pak", b"a"),
                ("Outfit/Option B/Dress.pak", b"b"),
                ("Shoes/mod_manifest.json", br#"{"ProjectName":"Shoes"}"#),
                ("Shoes/Shoes.pak", b"s"),
            ],
        );
        let report = install_archive(&archive.to_string_lossy(), &state, None, false).unwrap();
        assert_eq!((report.legacy, report.imported), (2, 1));
        assert!(paks.join("Outfit/Option A/Dress.pak_disabled").is_file());
        assert!(paks.join("Outfit/Option B/Dress.pak_disabled").is_file());
        assert!(!paks.join("Outfit/Shoes").exists());
        let s = state.store.lock().unwrap();
        assert!(s.mods.iter().any(|m| m.project == "Shoes" && !m.enabled));
    }

    #[test]
    fn manifest_mods_can_be_installed_as_legacy_paks_instead_of_the_library() {
        let (root, state, _, paks) = game_fixture();
        let archive = root.join("Shoes.zip");
        zip(
            &archive,
            &[
                ("Shoes/mod_manifest.json", br#"{"ProjectName":"Shoes"}"#),
                ("Shoes/Shoes.pak", b"s"),
                ("Shoes/Shoes.utoc", b"u"),
            ],
        );
        let path = archive.to_string_lossy();
        let report = install_archive_as(&path, &state, None, false, true).unwrap();
        assert_eq!((report.legacy, report.imported), (1, 0));
        assert!(paks.join("Shoes/Shoes.pak_disabled").is_file());
        assert!(!paks.join("Shoes/mod_manifest.json").exists());
        assert!(state.store.lock().unwrap().mods.is_empty());
        let empty = root.join("Notes.zip");
        zip(&empty, &[("readme.txt", b"r")]);
        let error =
            install_archive_as(&empty.to_string_lossy(), &state, None, false, true).unwrap_err();
        assert!(error.starts_with(NOTHING_INSTALLABLE));
    }

    #[test]
    fn failed_library_import_removes_copied_paks() {
        let (root, state, _, paks) = game_fixture();
        let archive = root.join("Mixed.zip");
        zip(
            &archive,
            &[
                ("Loose/Hat.pak", b"h"),
                ("A/mod_manifest.json", br#"{"ProjectName":"Same"}"#),
                ("B/mod_manifest.json", br#"{"ProjectName":"Same"}"#),
            ],
        );
        assert!(install_archive(&archive.to_string_lossy(), &state, None, false).is_err());
        assert!(legacy::scan(&paks).0.is_empty());
        assert!(!paks.join("Mixed").exists());
    }

    #[test]
    fn paks_require_a_mods_folder_and_unknown_content_is_refused() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let archive = root.join("Hair.zip");
        zip(&archive, &[("Hair.pak", b"p")]);
        let error = install_archive(&archive.to_string_lossy(), &state, None, false).unwrap_err();
        assert!(error.contains("~mods"));
        let other = root.join("Script.zip");
        zip(
            &other,
            &[("LogicMods/Script.pak", b"p"), ("notes.txt", b"n")],
        );
        let error = install_archive(&other.to_string_lossy(), &state, None, false).unwrap_err();
        assert!(error.starts_with(NOTHING_INSTALLABLE));
    }

    #[test]
    fn mod_enabler_needs_confirmation_and_backs_up_replaced_files() {
        let (root, state, game, paks) = game_fixture();
        let win64 = game.join("BlueClient/Binaries/Win64");
        fs::create_dir_all(&win64).unwrap();
        fs::write(win64.join("dsound.dll"), b"old").unwrap();
        let archive = root.join("inZOI Mod Enabler.zip");
        zip(
            &archive,
            &[
                // Layout of "InZOI Mod Enabler (Engine Next)" 1.01 (Nexus mod 1).
                ("BlueClient/Binaries/Win64/dsound.dll", b"new"),
                ("BlueClient/Binaries/Win64/bitfix/sig.lua", b"lua"),
                ("LICENSE.txt", b"license"),
            ],
        );
        let path = archive.to_string_lossy();
        let error = install_archive(&path, &state, None, false).unwrap_err();
        assert!(error.starts_with(ENABLER_CONFIRM));
        assert_eq!(fs::read(win64.join("dsound.dll")).unwrap(), b"old");
        let report = install_archive(&path, &state, None, true).unwrap();
        assert!(report.enabler);
        assert_eq!(fs::read(win64.join("dsound.dll")).unwrap(), b"new");
        assert!(win64.join("bitfix/sig.lua").is_file());
        assert!(!game.join("LICENSE.txt").exists());
        let trash = paks
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".inzoimm-trash");
        let backup = WalkDir::new(trash)
            .into_iter()
            .filter_map(|e| e.ok())
            .find(|e| e.file_name() == "dsound.dll")
            .unwrap();
        assert_eq!(fs::read(backup.path()).unwrap(), b"old");
        assert_eq!(
            legacy::enabler_installed(&state.store.lock().unwrap().settings),
            Some(true)
        );
        let settings = state.store.lock().unwrap().settings.clone();
        assert_eq!(
            legacy::game_dir(&settings).map(PathBuf::from),
            Some(dunce::simplified(&game).to_path_buf())
        );
    }

    #[test]
    fn bare_enabler_files_map_to_win64() {
        let (root, state, game, _) = game_fixture();
        let archive = root.join("Enabler.zip");
        zip(
            &archive,
            &[("Enabler/dsound.dll", b"d"), ("Enabler/bitfix/x.lua", b"x")],
        );
        install_mod_enabler_core(&archive.to_string_lossy(), &state).unwrap();
        assert!(game.join("BlueClient/Binaries/Win64/dsound.dll").is_file());
        assert!(game
            .join("BlueClient/Binaries/Win64/bitfix/x.lua")
            .is_file());
        let not_enabler = root.join("Hair.zip");
        zip(&not_enabler, &[("Hair.pak", b"p")]);
        assert!(install_mod_enabler_core(&not_enabler.to_string_lossy(), &state).is_err());
    }

    #[test]
    fn seven_zip_archives_are_extracted_with_the_same_guards() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("mod.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Mod/Hair.pak"),
                Some(&b"payload"[..]),
            )
            .unwrap();
        writer.finish().unwrap();
        let dest = root.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        extract(&archive, &dest).unwrap();
        assert_eq!(fs::read(dest.join("Mod/Hair.pak")).unwrap(), b"payload");
        for bad in ["../evil", "/abs", "C:/x", "a/../b"] {
            assert!(relative(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rar_archives_are_extracted() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("version.rar");
        // `version.rar` from the unrar crate's test data: one 11-byte VERSION file.
        fs::write(&archive, include_bytes!("../tests/fixtures/version.rar")).unwrap();
        let dest = root.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        extract(&archive, &dest).unwrap();
        assert_eq!(fs::metadata(dest.join("VERSION")).unwrap().len(), 11);
    }

    #[test]
    fn installed_download_hides_its_archive_card_until_the_paks_are_removed() {
        let (root, state, _, paks) = game_fixture();
        let downloads = root.join("downloads");
        state.store.lock().unwrap().settings.downloads_dir =
            downloads.to_string_lossy().into_owned();
        let info = nexus::parse_request("nxm://inzoi/mods/651/files/2669")
            .unwrap()
            .info;
        let parent = download_storage::mod_folder(&downloads, &info, "Hair").unwrap();
        let folder = download_storage::file_folder(&parent, 2669).unwrap();
        let archive = folder.join("Hair-651.zip");
        zip(&archive, &[("Hair.pak", b"p"), ("Hair.utoc", b"u")]);
        let mut record = metadata::download_record(
            &info,
            &serde_json::json!({"name":"Hair","file_name":"Hair-651.zip"}),
        );
        record["archive"] = "Hair-651.zip".into();
        fs::write(folder.join(".inzoimm-nexus.json"), record.to_string()).unwrap();
        let cards = || library::catalogue(&state.store.lock().unwrap()).0;
        assert!(cards().iter().any(|m| m.archive));
        let report = import_download_core(&archive.to_string_lossy(), &state).unwrap();
        assert_eq!(report.legacy, 1);
        assert!(paks.join("Hair-651/Hair.pak_disabled").is_file());
        let after = cards();
        assert!(!after.iter().any(|m| m.archive));
        let installed: Vec<_> = after.iter().filter(|m| m.legacy).collect();
        assert_eq!(installed.len(), 1);
        legacy::remove(&paks, &installed[0].id, |path| {
            crate::lifecycle_tests::mock_recycle(&root, path)
        })
        .unwrap();
        assert!(cards().iter().any(|m| m.archive));
        assert!(archive.is_file());
    }

    #[test]
    fn add_mod_installs_a_folder_of_loose_paks_but_refuses_the_game_folder() {
        let (root, state, game, paks) = game_fixture();
        let folder = root.join("Loose Hair");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Hair.pak"), b"p").unwrap();
        let report =
            import_any_folder(folder.to_string_lossy().into_owned(), true, &state).unwrap();
        assert_eq!(report.legacy, 1);
        assert!(paks.join("Loose Hair/Hair.pak_disabled").is_file());
        assert_eq!(fs::read(folder.join("Hair.pak")).unwrap(), b"p");
        fs::write(
            game.join("BlueClient/Content/Paks/pakchunk0-Windows.pak"),
            b"game",
        )
        .unwrap();
        assert!(import_any_folder(game.to_string_lossy().into_owned(), true, &state).is_err());
        assert!(game_package(Path::new(
            "BlueClient/Content/Paks/pakchunk0.pak"
        )));
        assert!(game_package(Path::new("Paks/DLC/x.pak")));
        assert!(!game_package(Path::new("Content/Paks/~mods/Hair.pak")));
        assert!(!game_package(Path::new("Hair/Hair.pak")));
    }

    #[test]
    fn download_install_record_hides_the_archive_card_while_files_exist() {
        let root = tempfile::tempdir().unwrap();
        let pak = root.path().join("Hair.pak");
        let archive = root.path().join("Hair.zip");
        fs::write(&archive, b"zip").unwrap();
        write_record(&archive, std::slice::from_ref(&pak)).unwrap();
        assert!(!installed_from(root.path()));
        fs::write(root.path().join("Hair.pak_disabled"), b"p").unwrap();
        assert!(installed_from(root.path()));
    }
}

#[cfg(test)]
mod machine {
    use super::*;

    /// `INZOIMM_ENABLER_ARCHIVE=<path to the Mod Enabler .7z/.zip/.rar>`
    #[test]
    #[ignore = "Reads a Mod Enabler archive from this computer"]
    fn real_mod_enabler_archive_maps_to_blueclient() {
        let archive = PathBuf::from(std::env::var("INZOIMM_ENABLER_ARCHIVE").unwrap());
        let temp = tempfile::tempdir().unwrap();
        extract(&archive, temp.path()).unwrap();
        let (source, target) = enabler_layout(temp.path()).unwrap().unwrap();
        assert_eq!(target, PathBuf::from("BlueClient"));
        assert!(source.join("Binaries/Win64/dsound.dll").is_file());
        for entry in WalkDir::new(temp.path()).into_iter().filter_map(|e| e.ok()) {
            println!(
                "{}",
                entry.path().strip_prefix(temp.path()).unwrap().display()
            );
        }
    }
}
