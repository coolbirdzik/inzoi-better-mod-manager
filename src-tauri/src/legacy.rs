//! Legacy PAK mods: `<inZOI>/BlueClient/Content/Paks/~mods`.
//!
//! A mod is the set of same-named `.pak`, `.utoc`, `.ucas` and `.sig` files in
//! one folder. The game only mounts `*.pak`, so disabling renames each file to
//! `<name>_disabled`. Nothing is stored: the folder itself is the source of
//! truth, and files outside `~mods` (the game's own paks) are never listed.
use super::*;
use std::collections::BTreeMap;

const EXTENSIONS: [&str; 4] = ["pak", "utoc", "ucas", "sig"];
const DISABLED: &str = "_disabled";
pub(crate) const ID_PREFIX: &str = "legacy:";
const STEAM_APP_ID: &str = "2456740";

pub(crate) fn is_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

/// Returns `(stem, extension, disabled)` for a file belonging to a PAK set.
fn classify(name: &str) -> Option<(String, &'static str, bool)> {
    let (stem, ext) = name.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    let (ext, disabled) = match ext.strip_suffix(DISABLED) {
        Some(ext) => (ext.to_string(), true),
        None => (ext, false),
    };
    let ext = EXTENSIONS.iter().find(|known| **known == ext)?;
    (!stem.is_empty()).then(|| (stem.to_string(), *ext, disabled))
}

struct Member {
    path: PathBuf,
    extension: &'static str,
    disabled: bool,
}
struct Group {
    dir: PathBuf,
    stem: String,
    members: Vec<Member>,
}
impl Group {
    fn pak(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.extension == "pak")
    }
}

fn key(path: &Path) -> String {
    let text = dunce::simplified(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn groups(root: &Path) -> (BTreeMap<String, Group>, Vec<String>) {
    let mut result: BTreeMap<String, Group> = BTreeMap::new();
    let mut warnings = vec![];
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || (e.file_name() != ".trash"
                    && !e.file_name().to_string_lossy().starts_with(".inzoimm-"))
        })
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(error.to_string());
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Some((stem, extension, disabled)) = classify(&entry.file_name().to_string_lossy())
        else {
            continue;
        };
        let dir = entry.path().parent().unwrap_or(root).to_path_buf();
        let relative = dir.strip_prefix(root).unwrap_or(&dir);
        let id = format!(
            "{ID_PREFIX}{}/{}",
            relative.to_string_lossy().replace('\\', "/"),
            stem
        )
        .to_lowercase();
        result
            .entry(id)
            .or_insert_with(|| Group {
                dir,
                stem,
                members: vec![],
            })
            .members
            .push(Member {
                path: entry.path().to_path_buf(),
                extension,
                disabled,
            });
    }
    // Sidecars without a pak (for example a lone .sig) are not mods.
    result.retain(|_, group| group.pak().is_some());
    (result, warnings)
}

fn modified_ms(path: &Path) -> u64 {
    path_added_at(path)
}

pub(crate) fn scan(root: &Path) -> (Vec<Mod>, Vec<String>) {
    if !root.is_dir() {
        return (vec![], vec![]);
    }
    let (found, warnings) = groups(root);
    let mods = found
        .into_iter()
        .map(|(id, group)| {
            let pak = group.pak().expect("groups without a pak were removed");
            let size: u64 = group
                .members
                .iter()
                .filter_map(|m| fs::metadata(&m.path).ok())
                .map(|m| m.len())
                .sum();
            let thumbnail = ["png", "jpg", "jpeg", "webp"]
                .iter()
                .map(|ext| group.dir.join(format!("{}.{ext}", group.stem)))
                .find(|p| p.is_file())
                .map(|p| p.to_string_lossy().into_owned());
            let files: Vec<_> = group
                .members
                .iter()
                .filter_map(|m| m.path.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect();
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "Legacy PAK".to_string(),
                serde_json::json!({"Files": files.join(", "), "Size": size}),
            );
            Mod {
                id,
                name: group.stem.clone(),
                author: String::new(),
                version: String::new(),
                description: String::new(),
                path: pak.path.to_string_lossy().into_owned(),
                thumbnail,
                managed: false,
                enabled: !pak.disabled,
                source: String::new(),
                project: group.stem.clone(),
                deployed_path: None,
                library_root: None,
                metadata,
                tags: vec![],
                archive: false,
                previous_version: None,
                superseded_by: None,
                added_at: modified_ms(&pak.path),
                legacy: true,
            }
        })
        .collect();
    (mods, warnings)
}

fn io_message(error: std::io::Error, path: &Path) -> String {
    // 5 = access denied, 32 = sharing violation (the game has the pak open).
    if matches!(error.raw_os_error(), Some(5 | 32))
        || error.kind() == std::io::ErrorKind::PermissionDenied
    {
        format!(
            "Could not change {}. Close inZOI, and make sure the game folder is writable (run the manager as administrator if it is in Program Files).",
            path.display()
        )
    } else {
        format!("{}: {error}", path.display())
    }
}

fn find(root: &Path, id: &str) -> Result<Group> {
    if !root.is_dir() {
        return Err("Set the inZOI ~mods folder in Settings first".into());
    }
    groups(root)
        .0
        .remove(&id.to_lowercase())
        .ok_or_else(|| "Mod no longer exists".into())
}

fn rename_all(moves: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (done, (from, to)) in moves.iter().enumerate() {
        if let Err(error) = fs::rename(from, to) {
            for (from, to) in moves[..done].iter().rev() {
                let _ = fs::rename(to, from);
            }
            return Err(io_message(error, from));
        }
    }
    Ok(())
}

pub(crate) fn set_enabled(root: &Path, id: &str, enabled: bool) -> Result<()> {
    let group = find(root, id)?;
    let mut moves = vec![];
    for member in &group.members {
        if member.disabled == enabled {
            let name = member.path.file_name().unwrap().to_string_lossy();
            let target = if enabled {
                member.path.with_file_name(&name[..name.len() - DISABLED.len()])
            } else {
                member.path.with_file_name(format!("{name}{DISABLED}"))
            };
            if target.exists() {
                return Err(format!(
                    "{} already exists. It was not overwritten.",
                    target.display()
                ));
            }
            moves.push((member.path.clone(), target));
        }
    }
    rename_all(&moves)
}

/// `Content/.inzoimm-trash`: beside `Paks`, never inside it, so a removed set
/// can be recovered by hand but can never be mounted by the game.
fn trash_dir(root: &Path) -> Result<PathBuf> {
    root.ancestors()
        .find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("Paks")))
        .and_then(Path::parent)
        .map(|content| content.join(".inzoimm-trash"))
        .ok_or_else(|| "The legacy PAK folder must be inside Content/Paks".into())
}

/// Moves the whole set to the trash beside `Paks`.
pub(crate) fn remove(root: &Path, id: &str) -> Result<()> {
    let group = find(root, id)?;
    let trash = trash_dir(root)?.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&trash).map_err(err)?;
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    for member in &group.members {
        let to = trash.join(member.path.file_name().unwrap());
        // Same volume as the game, so a rename is atomic and can be undone.
        if let Err(error) = fs::rename(&member.path, &to) {
            for (from, to) in moved.iter().rev() {
                let _ = fs::rename(to, from);
            }
            let _ = fs::remove_dir(&trash);
            return Err(io_message(error, &member.path));
        }
        moved.push((member.path.clone(), to));
    }
    Ok(())
}

/// Accepts the game folder, `Content/Paks`, `~mods` or a folder below it and
/// returns the folder legacy mods belong in.
pub(crate) fn normalize(raw: &Path) -> Result<PathBuf> {
    if !raw.is_absolute() {
        return Err("Use an absolute folder path".into());
    }
    let parts: Vec<_> = raw
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if let Some(index) = parts.iter().position(|p| p == "paks") {
        // Only ~mods (or a folder in it) may be managed: the rest of Paks
        // holds the game's own files, DLC and script mods.
        return match parts.get(index + 1) {
            None => Ok(raw.join("~mods")),
            Some(next) if next == "~mods" => Ok(raw.to_path_buf()),
            Some(_) => Err("Only the Content/Paks/~mods folder can be used for legacy PAK mods".into()),
        };
    }
    for relative in ["BlueClient/Content/Paks", "Content/Paks"] {
        let paks = raw.join(relative);
        if paks.is_dir() {
            return Ok(paks.join("~mods"));
        }
    }
    Err("Choose the inZOI game folder, its Content/Paks folder or the ~mods folder".into())
}

pub(crate) fn validate(settings: &Settings) -> Result<()> {
    if settings.paks().is_empty() {
        return Ok(());
    }
    let paks = normalize(Path::new(settings.paks()))?;
    let library = PathBuf::from(&settings.library_dir);
    let (library, paks) = (
        library.canonicalize().unwrap_or(library),
        paks.canonicalize().unwrap_or(paks),
    );
    if library.starts_with(&paks) || paks.starts_with(&library) {
        return Err("The managed mods folder and the ~mods folder must be separate".into());
    }
    Ok(())
}

fn acf_value(text: &str, wanted: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut quoted = line.split('"').skip(1).step_by(2);
        (quoted.next()?.eq_ignore_ascii_case(wanted))
            .then(|| quoted.next().map(|v| v.replace("\\\\", "\\")))
            .flatten()
    })
}

#[cfg(windows)]
fn steam_from_registry() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text.lines().find(|l| l.contains("SteamPath"))?;
    let path = value.split("REG_SZ").nth(1)?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path.replace('/', "\\")))
}
#[cfg(not(windows))]
fn steam_from_registry() -> Option<PathBuf> {
    None
}

fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = vec![];
    let mut roots: Vec<PathBuf> = steam_from_registry().into_iter().collect();
    for drive in discovery_drives::roots().unwrap_or_default() {
        for relative in [
            "SteamLibrary",
            "Steam",
            "Program Files (x86)/Steam",
            "Program Files/Steam",
            "Games/Steam",
            "Games/SteamLibrary",
        ] {
            roots.push(drive.join(relative));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join("Library/Application Support/Steam"));
    }
    let add = |path: PathBuf, libraries: &mut Vec<PathBuf>| {
        if path.join("steamapps").is_dir() && !libraries.iter().any(|p| key(p) == key(&path)) {
            libraries.push(path);
        }
    };
    for root in roots {
        add(root.clone(), &mut libraries);
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in text.lines() {
                if let Some(path) = acf_value(line, "path") {
                    add(PathBuf::from(path), &mut libraries);
                }
            }
        }
    }
    libraries
}

/// Finds `Content/Paks/~mods` of installed copies of the game. The folder may
/// not exist yet, but its `Paks` parent must.
pub(crate) fn detect() -> Vec<PathBuf> {
    let mut games = vec![];
    for library in steam_libraries() {
        let steamapps = library.join("steamapps");
        let installdir =
            fs::read_to_string(steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf")))
                .ok()
                .and_then(|text| acf_value(&text, "installdir"))
                .unwrap_or_else(|| "inZOI".into());
        games.push(steamapps.join("common").join(installdir));
    }
    for drive in discovery_drives::roots().unwrap_or_default() {
        games.push(drive.join("Program Files/Epic Games/inZOI"));
        games.push(drive.join("Epic Games/inZOI"));
    }
    let mut found: Vec<PathBuf> = vec![];
    for game in games {
        let paks = game.join("BlueClient").join("Content").join("Paks");
        if paks.is_dir() && !found.iter().any(|p| key(p) == key(&paks.join("~mods"))) {
            found.push(paks.join("~mods"));
        }
    }
    found
}

#[tauri::command(async)]
pub(crate) fn detect_paks_dir(picked: Option<String>) -> Result<Option<String>> {
    let path = match picked {
        Some(picked) => Some(normalize(Path::new(&picked))?),
        None => detect().into_iter().next(),
    };
    Ok(path.map(|p| {
        let text = dunce::simplified(&p).to_string_lossy().into_owned();
        if cfg!(windows) {
            text.replace('/', "\\")
        } else {
            text
        }
    }))
}

/// Runs once for stores that never chose a folder (`None`). An empty string
/// means the user switched legacy PAK support off and is left alone.
#[tauri::command(async)]
pub(crate) fn init_paks_dir(state: tauri::State<State>) -> Result<bool> {
    if state.store.lock().map_err(err)?.settings.paks_dir.is_some() {
        return Ok(false);
    }
    let found = detect().into_iter().next().map(|p| {
        let text = dunce::simplified(&p).to_string_lossy().into_owned();
        if cfg!(windows) {
            text.replace('/', "\\")
        } else {
            text
        }
    });
    let mut s = state.store.lock().map_err(err)?;
    if s.settings.paks_dir.is_some() {
        return Ok(false);
    }
    s.settings.paks_dir = Some(found.unwrap_or_default());
    if let Err(error) = write_store(&state, &s) {
        s.settings.paks_dir = None;
        return Err(error);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let paks = root.path().join("BlueClient/Content/Paks");
        let mods = paks.join("~mods");
        fs::create_dir_all(&mods).unwrap();
        (root, paks, mods)
    }

    #[test]
    fn a_pak_set_is_one_mod_and_toggles_together_without_touching_game_paks() {
        let (root, paks, mods) = fixture();
        fs::write(paks.join("pakchunk0-Windows.pak"), b"game").unwrap();
        for ext in ["pak", "utoc", "ucas", "sig"] {
            fs::write(mods.join(format!("Hair.{ext}")), ext).unwrap();
        }
        fs::write(mods.join("Lonely.sig"), b"x").unwrap();
        fs::create_dir_all(mods.join("AnimConfig")).unwrap();
        fs::write(mods.join("AnimConfig/anim.json"), b"{}").unwrap();
        let (found, _) = scan(&mods);
        assert_eq!(found.len(), 1);
        assert!(found[0].legacy && found[0].enabled && found[0].project == "Hair");
        set_enabled(&mods, &found[0].id, false).unwrap();
        for ext in ["pak", "utoc", "ucas", "sig"] {
            assert!(!mods.join(format!("Hair.{ext}")).exists());
            assert!(mods.join(format!("Hair.{ext}{DISABLED}")).exists());
        }
        let (found, _) = scan(&mods);
        assert_eq!(found.len(), 1);
        assert!(!found[0].enabled);
        set_enabled(&mods, &found[0].id, true).unwrap();
        assert!(mods.join("Hair.utoc").exists());
        assert_eq!(fs::read(paks.join("pakchunk0-Windows.pak")).unwrap(), b"game");
        assert!(mods.join("AnimConfig/anim.json").exists());
        drop(root);
    }

    #[test]
    fn an_existing_file_is_never_overwritten_and_a_failed_toggle_is_rolled_back() {
        let (root, _, mods) = fixture();
        fs::write(mods.join("A.pak"), b"new").unwrap();
        fs::write(mods.join("A.utoc"), b"new").unwrap();
        fs::write(mods.join("A.utoc_disabled"), b"other").unwrap();
        let id = scan(&mods).0[0].id.clone();
        assert!(set_enabled(&mods, &id, false).is_err());
        assert!(mods.join("A.pak").exists() && mods.join("A.utoc").exists());
        assert_eq!(fs::read(mods.join("A.utoc_disabled")).unwrap(), b"other");
        drop(root);
    }

    #[test]
    fn subfolders_and_already_disabled_paks_are_listed_and_removal_leaves_the_game_folder() {
        let (_root, paks, mods) = fixture();
        fs::create_dir_all(mods.join("Pack")).unwrap();
        fs::write(mods.join("Pack/Dress.pak_disabled"), b"p").unwrap();
        fs::write(mods.join("Pack/Dress.ucas"), b"c").unwrap();
        let found = scan(&mods).0;
        assert_eq!(found.len(), 1);
        assert!(!found[0].enabled);
        remove(&mods, &found[0].id).unwrap();
        assert!(scan(&mods).0.is_empty());
        assert!(!mods.join("Pack/Dress.ucas").exists());
        assert!(paks.parent().unwrap().join(".inzoimm-trash").is_dir());
        assert!(!paks.join(".inzoimm-trash").exists());
    }

    #[test]
    fn folders_are_normalised_to_mods_and_old_stores_still_load() {
        let (root, paks, mods) = fixture();
        let game = root.path();
        assert_eq!(normalize(game).unwrap(), mods);
        assert_eq!(normalize(&paks).unwrap(), mods);
        assert_eq!(normalize(&mods).unwrap(), mods);
        assert!(normalize(&root.path().join("elsewhere")).is_err());
        for other in ["DLC", "LogicMods"] {
            assert!(normalize(&paks.join(other)).is_err());
        }
        let old = r#"{"mods_dir":"a","library_dir":"b"}"#;
        let settings: Settings = serde_json::from_str(old).unwrap();
        assert!(settings.paks_dir.is_none());
        let old_mod = r#"{"id":"1","name":"","author":"","version":"","description":"","path":"","thumbnail":null,"managed":false,"enabled":false,"source":"","project":"x"}"#;
        assert!(!serde_json::from_str::<Mod>(old_mod).unwrap().legacy);
    }

    #[test]
    fn steam_manifest_values_are_read() {
        let text = "\t\"installdir\"\t\t\"inZOI\"\n\t\"path\"\t\t\"D:\\\\SteamLibrary\"";
        assert_eq!(acf_value(text, "installdir").as_deref(), Some("inZOI"));
        assert_eq!(acf_value(text, "path").as_deref(), Some("D:\\SteamLibrary"));
    }
}
#[cfg(test)]
mod machine {
    #[test]
    #[ignore = "Reads the local Steam installation"]
    fn detects_the_local_game() {
        println!("{:?}", super::detect());
    }
}
