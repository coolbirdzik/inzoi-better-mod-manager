use super::*;
use std::io::{Cursor, Read};
#[derive(Serialize, Deserialize)]
struct Plan {
    root: PathBuf,
    parent_pid: u32,
    version: String,
    signature: String,
}
pub(crate) fn root() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    if parent.join("portable.json").is_file() {
        Some(parent.to_path_buf())
    } else {
        None
    }
}
// Only rebase the default portable library; an explicitly selected external library stays put.
pub(crate) fn relocate_library(store: &mut Store, previous: &Path, current: &Path) -> bool {
    let old = previous.join("library");
    let new = current.join("library");
    if dunce::simplified(previous) == dunce::simplified(current)
        || dunce::simplified(Path::new(&store.settings.library_dir)) != dunce::simplified(&old)
    {
        return false;
    }
    for m in &mut store.mods {
        if let Ok(rel) = dunce::simplified(Path::new(&m.path)).strip_prefix(dunce::simplified(&old))
        {
            m.path = new.join(rel).to_string_lossy().into_owned();
        }
        if let Some(thumb) = &m.thumbnail {
            if let Ok(rel) =
                dunce::simplified(Path::new(thumb)).strip_prefix(dunce::simplified(&old))
            {
                m.thumbnail = Some(new.join(rel).to_string_lossy().into_owned());
            }
        }
    }
    store.settings.library_dir = new.to_string_lossy().into_owned();
    true
}
#[tauri::command]
pub(crate) fn is_portable() -> bool {
    root().is_some()
}
fn verify_package(bytes: &[u8], signature: &str, version: &str) -> Result<()> {
    use base64::Engine;
    let cfg: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).map_err(err)?;
    let key = cfg["plugins"]["updater"]["pubkey"]
        .as_str()
        .ok_or("Missing updater public key")?;
    let decode = |s: &str| -> Result<String> {
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .map_err(err)?,
        )
        .map_err(err)
    };
    let key = minisign_verify::PublicKey::decode(&decode(key)?).map_err(err)?;
    let sig = minisign_verify::Signature::decode(&decode(signature)?).map_err(err)?;
    key.verify(bytes, &sig, true).map_err(err)?;
    if !sig.trusted_comment().split('\t').any(|field| {
        field
            .strip_prefix("version:")
            .map(|v| v.trim_start_matches('v') == version.trim_start_matches('v'))
            .unwrap_or(false)
    }) {
        return Err("Portable package version does not match its signed version".into());
    }
    Ok(())
}
fn extract_executable(bytes: &[u8], destination: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(err)?;
    let mut entry = archive
        .by_name("inzoi-better-mod-manager.exe")
        .map_err(err)?;
    if entry.size() > 256 * 1024 * 1024
        || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
    {
        return Err("Invalid portable executable".into());
    }
    let mut data = vec![];
    entry
        .by_ref()
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(err)?;
    if data.len() < 2 || data.len() > 256 * 1024 * 1024 || &data[..2] != b"MZ" {
        return Err("Portable package has no valid Windows executable".into());
    }
    fs::write(destination, data).map_err(err)
}
#[tauri::command]
pub(crate) async fn update_portable(app: tauri::AppHandle) -> Result<bool> {
    use tauri_plugin_updater::UpdaterExt;
    let root = root().ok_or("This app is not running in portable mode")?;
    let Some(update) = app
        .updater_builder()
        .target("windows-x86_64-portable")
        .build()
        .map_err(err)?
        .check()
        .await
        .map_err(err)?
    else {
        return Ok(false);
    };
    let bytes = update.download(|_, _| {}, || {}).await.map_err(err)?;
    let staging = root
        .join("data/updates")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&staging).map_err(err)?;
    // The official updater verifies the signature before any executable is staged.
    extract_executable(&bytes, &staging.join("new.exe"))?;
    fs::write(staging.join("package.zip"), bytes).map_err(err)?;
    let plan = Plan {
        root: root.canonicalize().map_err(err)?,
        parent_pid: std::process::id(),
        version: update.version,
        signature: update.signature,
    };
    fs::write(
        staging.join("plan.json"),
        serde_json::to_vec(&plan).map_err(err)?,
    )
    .map_err(err)?;
    let helper = staging.join("updater-helper.exe");
    fs::copy(std::env::current_exe().map_err(err)?, &helper).map_err(err)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(helper)
            .arg("--apply-portable-update")
            .arg(staging.join("plan.json"))
            .creation_flags(0x08000000)
            .spawn()
            .map_err(err)?;
        app.exit(0);
    }
    #[cfg(not(windows))]
    {
        return Err("Portable updates are supported on Windows only".into());
    }
    #[allow(unreachable_code)]
    Ok(true)
}
fn replace_file(target: &Path, staged: &Path, backup: &Path) -> Result<()> {
    if backup.exists() {
        return Err("An update backup already exists".into());
    }
    fs::rename(target, backup).map_err(err)?;
    if let Err(e) = fs::rename(staged, target) {
        let _ = fs::rename(backup, target);
        return Err(err(e));
    }
    Ok(())
}
#[cfg(windows)]
fn apply(plan_path: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };
    let plan: Plan = serde_json::from_slice(&fs::read(plan_path).map_err(err)?).map_err(err)?;
    let root = plan.root.canonicalize().map_err(err)?;
    let stage = plan_path
        .parent()
        .ok_or("Missing updater staging directory")?
        .canonicalize()
        .map_err(err)?;
    if !root.join("portable.json").is_file()
        || !stage.starts_with(root.join("data/updates").canonicalize().map_err(err)?)
    {
        return Err("Update plan is outside the portable folder".into());
    }
    let bytes = fs::read(stage.join("package.zip")).map_err(err)?;
    verify_package(&bytes, &plan.signature, &plan.version)?;
    if semver::Version::parse(&plan.version).map_err(err)?
        <= semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(err)?
    {
        return Err("Portable update must be newer than the running version".into());
    }
    // Re-extract verified bytes so an altered staging executable is never applied.
    let staged = stage.join("verified-new.exe");
    extract_executable(&bytes, &staged)?;
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, plan.parent_pid);
        if !process.is_null() {
            let result = WaitForSingleObject(process, 60_000);
            CloseHandle(process);
            if result != 0 {
                return Err("Timed out waiting for the app to exit".into());
            }
        }
    }
    let target = root.join("inzoi-better-mod-manager.exe");
    let backup = stage.join("previous.exe");
    replace_file(&target, &staged, &backup)?;
    if let Err(e) = std::process::Command::new(&target)
        .current_dir(&root)
        .creation_flags(0x08000000)
        .spawn()
    {
        let failed = stage.join("failed-new.exe");
        let _ = fs::rename(&target, failed);
        let _ = fs::rename(&backup, &target);
        let _ = std::process::Command::new(&target)
            .current_dir(&root)
            .creation_flags(0x08000000)
            .spawn();
        return Err(err(e));
    }
    Ok(())
}
pub fn apply_from_cli() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|s| s.to_str()) != Some("--apply-portable-update") {
        return false;
    }
    #[cfg(windows)]
    {
        if let Some(path) = args.get(2) {
            let path = Path::new(path);
            if let Err(error) = apply(path) {
                if let Some(dir) = path.parent() {
                    let _ = fs::write(dir.join("update-error.txt"), error);
                }
            }
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn moving_portable_rebases_only_the_default_library() {
        let previous = std::env::temp_dir().join("old-portable/data");
        let current = std::env::temp_dir().join("new-portable/data");
        let mut store = Store {
            settings: Settings {
                mods_dir: "game-mods".into(),
                library_dir: previous.join("library").to_string_lossy().into_owned(),
            },
            mods: vec![],
        };
        assert!(relocate_library(&mut store, &previous, &current));
        assert_eq!(
            Path::new(&store.settings.library_dir),
            current.join("library")
        );
        store.settings.library_dir = "external-library".into();
        assert!(!relocate_library(&mut store, &previous, &current));
        assert_eq!(store.settings.library_dir, "external-library");
        assert_eq!(store.settings.mods_dir, "game-mods");
    }
    #[test]
    fn extracts_only_expected_executable() {
        let mut bytes = Cursor::new(vec![]);
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            zip.start_file("../unrelated.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"untrusted extra file").unwrap();
            zip.start_file(
                "inzoi-better-mod-manager.exe",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"MZtest-exe").unwrap();
            zip.finish().unwrap();
        }
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        extract_executable(bytes.get_ref(), &root.join("new.exe")).unwrap();
        assert_eq!(fs::read(root.join("new.exe")).unwrap(), b"MZtest-exe");
        assert!(!root.join("unrelated.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replacement_keeps_previous_version() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let old = root.join("app.exe");
        let new = root.join("new.exe");
        let backup = root.join("previous.exe");
        fs::write(&old, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        replace_file(&old, &new, &backup).unwrap();
        assert_eq!(fs::read(old).unwrap(), b"new");
        assert_eq!(fs::read(backup).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_replacement_restores_previous_version() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let old = root.join("app.exe");
        fs::write(&old, b"old").unwrap();
        assert!(replace_file(&old, &root.join("missing.exe"), &root.join("backup.exe")).is_err());
        assert_eq!(fs::read(old).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsigned_package_cannot_be_verified() {
        assert!(verify_package(b"invalid", "invalid signature", "9.9.9").is_err());
    }
}
