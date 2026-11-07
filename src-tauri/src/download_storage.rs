use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const MOD_RECORD: &str = ".inzoimm-mod.json";
const FILE_RECORD: &str = ".inzoimm-nexus.json";

fn json(path: &Path) -> Result<serde_json::Value> {
    manifest_value(&metadata::read_limited(path, 2 * 1024 * 1024)?)
}
fn provider(record: &serde_json::Value) -> &str {
    record["provider"].as_str().unwrap_or("nexus")
}
fn mod_matches(record: &serde_json::Value, id: u64, expected_provider: &str) -> bool {
    record["game"] == "inzoi"
        && record["mod_id"].as_u64() == Some(id)
        && provider(record) == expected_provider
}
fn file_record(provider: &str) -> &str {
    if provider == "curseforge" {
        ".inzoimm-curseforge.json"
    } else {
        FILE_RECORD
    }
}
fn safe_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let shortened: String = cleaned.chars().take(72).collect();
    let name = shortened.trim_matches(['.', ' ']);
    if name.is_empty() {
        "Mod".into()
    } else {
        name.into()
    }
}

fn find_mod_for(root: &Path, id: u64, provider: &str) -> Result<Option<PathBuf>> {
    if !root.exists() {
        return Ok(None);
    }
    let root = root.canonicalize().map_err(err)?;
    let mut found = None;
    for entry in fs::read_dir(&root).map_err(err)? {
        let entry = entry.map_err(err)?;
        if !entry.file_type().map_err(err)?.is_dir() {
            continue;
        }
        let record = entry.path().join(MOD_RECORD);
        let expected_name = entry
            .file_name()
            .to_string_lossy()
            .ends_with(&format!(" [{provider}-{id}]"));
        if !record.is_file() {
            if expected_name {
                return Err(
                    "This mod folder already exists without app metadata; files were kept".into(),
                );
            }
            continue;
        }
        if let Ok(record) = json(&record) {
            if mod_matches(&record, id, provider) {
                let path = entry.path().canonicalize().map_err(err)?;
                if !path.starts_with(&root) {
                    return Err("Mod download folder is outside the selected root".into());
                }
                if found.replace(path).is_some() {
                    return Err(format!("Multiple saved folders exist for Nexus mod #{id}; keep one folder for this mod"));
                }
            } else if expected_name {
                return Err("Mod folder name and saved ID do not match; files were kept".into());
            }
        } else if expected_name {
            return Err("Saved mod folder metadata could not be read; files were kept".into());
        }
    }
    Ok(found)
}

pub(crate) fn mod_folder(root: &Path, info: &NexusLink, title: &str) -> Result<PathBuf> {
    if let Some(path) = find_mod_for(root, info.mod_id, &info.provider)? {
        return Ok(path);
    }
    fs::create_dir_all(root).map_err(err)?;
    let root = root.canonicalize().map_err(err)?;
    let path = root.join(format!(
        "{} [{}-{}]",
        safe_name(title),
        info.provider,
        info.mod_id
    ));
    // A pre-existing unrecognised folder belongs to the user; never take it over.
    fs::create_dir(&path)
        .map_err(|e| format!("Could not create mod folder {}: {e}", path.display()))?;
    let record = serde_json::json!({"layout":1,"game":"inzoi","mod_id":info.mod_id,"name":title,"provider":info.provider});
    let result = fs::write(
        path.join(MOD_RECORD),
        serde_json::to_vec_pretty(&record).map_err(err)?,
    )
    .map_err(err);
    if result.is_err() {
        let _ = fs::remove_file(path.join(MOD_RECORD));
        let _ = fs::remove_dir(&path);
    }
    result?;
    Ok(path)
}

pub(crate) fn file_folder(mod_folder: &Path, file_id: u64) -> Result<PathBuf> {
    let mod_folder = mod_folder.canonicalize().map_err(err)?;
    let files = mod_folder.join("files");
    if files.exists() && !files.canonicalize().map_err(err)?.starts_with(&mod_folder) {
        return Err("File collection is outside its mod folder".into());
    }
    let folder = mod_folder.join("files").join(format!("file-{file_id}"));
    fs::create_dir_all(&folder).map_err(err)?;
    let folder = folder.canonicalize().map_err(err)?;
    if !folder.starts_with(mod_folder.canonicalize().map_err(err)?) {
        return Err("File download folder is outside its mod folder".into());
    }
    Ok(folder)
}

pub(crate) fn cached_file(root: &Path, info: &NexusLink) -> Result<Option<(PathBuf, u64)>> {
    let Some(parent) = find_mod_for(root, info.mod_id, &info.provider)? else {
        return Ok(None);
    };
    let folder = parent.join("files").join(format!("file-{}", info.file_id));
    if !folder.exists() {
        return Ok(None);
    }
    if !folder.canonicalize().map_err(err)?.starts_with(&parent) {
        return Err("Saved file folder is outside its mod folder".into());
    }
    cached(&folder, info)
}

pub(crate) fn download_root(folder: &Path, record: &serde_json::Value) -> Result<Option<PathBuf>> {
    let mod_id = record["mod_id"].as_u64().ok_or("Missing download mod ID")?;
    let file_id = record["file_id"]
        .as_u64()
        .ok_or("Missing download file ID")?;
    if folder.file_name().and_then(|n| n.to_str()) != Some(format!("file-{file_id}").as_str()) {
        return Ok(None);
    }
    let files = folder.parent().ok_or("Missing file collection folder")?;
    if files.file_name().and_then(|n| n.to_str()) != Some("files") {
        return Err("Invalid saved mod folder layout".into());
    }
    let parent = files.parent().ok_or("Missing mod folder")?;
    if !mod_matches(&json(&parent.join(MOD_RECORD))?, mod_id, provider(record)) {
        return Err("Saved mod ID does not match this archive".into());
    }
    Ok(Some(
        parent
            .parent()
            .ok_or("Missing downloads root")?
            .to_path_buf(),
    ))
}

fn hash(path: &Path) -> Result<(u64, String)> {
    let mut file = fs::File::open(path).map_err(err)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(err)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

pub(crate) fn cached(folder: &Path, info: &NexusLink) -> Result<Option<(PathBuf, u64)>> {
    let marker = folder.join(file_record(&info.provider));
    if !marker.exists() {
        return Ok(None);
    }
    let record = json(&marker)?;
    if provider(&record) != info.provider
        || record["mod_id"].as_u64() != Some(info.mod_id)
        || record["file_id"].as_u64() != Some(info.file_id)
    {
        return Err("Existing download metadata belongs to another mod or file".into());
    }
    let name = record["archive"]
        .as_str()
        .filter(|name| safe_project(name))
        .ok_or("Existing download has no complete archive record; files were kept")?;
    let path = folder.join(name);
    let expected = record["bytes"]
        .as_u64()
        .ok_or("Existing download has no size record")?;
    let (bytes, digest) = hash(&path)?;
    if bytes != expected || bytes == 0 || record["sha256"].as_str() != Some(digest.as_str()) {
        return Err("The saved archive changed or is incomplete. It was kept; move it out before downloading this file again.".into());
    }
    Ok(Some((path, bytes)))
}

// Cancellation/failure cleans only this attempt's own files. Other versions and
// already unpacked mods in this stable directory must never be removed.
pub(crate) struct PendingArchive {
    partial: PathBuf,
    target: PathBuf,
    published: bool,
    committed: bool,
}
impl PendingArchive {
    pub(crate) fn new(folder: &Path, filename: &str) -> Result<Self> {
        let target = folder.join(filename);
        if target.exists() {
            return Err(format!(
                "{} already exists and was not overwritten",
                target.display()
            ));
        }
        Ok(Self {
            partial: folder.join(format!(".inzoimm-{}.part", uuid::Uuid::new_v4())),
            target,
            published: false,
            committed: false,
        })
    }
    pub(crate) fn create(&self) -> Result<fs::File> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.partial)
            .map_err(err)
    }
    pub(crate) fn finish(
        mut self,
        mut record: serde_json::Value,
        bytes: u64,
        digest: String,
    ) -> Result<PathBuf> {
        // create_new also prevents replacement on platforms where rename overwrites.
        if fs::hard_link(&self.partial, &self.target).is_ok() {
            self.published = true;
        } else {
            let mut target = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.target)
                .map_err(err)?;
            self.published = true;
            std::io::copy(
                &mut fs::File::open(&self.partial).map_err(err)?,
                &mut target,
            )
            .map_err(err)?;
            target.sync_all().map_err(err)?;
        }
        record["archive"] = self
            .target
            .file_name()
            .unwrap()
            .to_string_lossy()
            .as_ref()
            .into();
        record["bytes"] = bytes.into();
        record["sha256"] = digest.into();
        record["layout"] = 2.into();
        let marker = self
            .target
            .parent()
            .unwrap()
            .join(file_record(provider(&record)));
        let serialized = serde_json::to_vec_pretty(&record).map_err(err)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
            .map_err(err)?;
        if let Err(error) = file.write_all(&serialized).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&marker);
            return Err(err(error));
        }
        self.committed = true;
        Ok(self.target.clone())
    }
}
impl Drop for PendingArchive {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.partial);
        if self.published && !self.committed {
            let _ = fs::remove_file(&self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(mod_id: u64, file_id: u64) -> NexusLink {
        nexus::parse_request(&format!("nxm://inzoi/mods/{mod_id}/files/{file_id}"))
            .unwrap()
            .info
    }
    fn save(root: &Path, info: &NexusLink, title: &str, bytes: &[u8]) -> PathBuf {
        let parent = mod_folder(root, info, title).unwrap();
        let folder = file_folder(&parent, info.file_id).unwrap();
        let pending = PendingArchive::new(&folder, "mod.zip").unwrap();
        let mut file = pending.create().unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);
        pending
            .finish(
                metadata::download_record(info, &serde_json::json!({"name": title})),
                bytes.len() as u64,
                format!("{:x}", Sha256::digest(bytes)),
            )
            .unwrap()
    }

    #[test]
    fn one_readable_folder_per_mod_reuses_identity_across_titles_and_file_versions() {
        let (root, _, _) = crate::lifecycle_tests::fixture();
        let downloads = root.join("downloads");
        let first = mod_folder(&downloads, &info(42, 99), "  Hair: Silk / Shine?  ").unwrap();
        assert_eq!(first.file_name().unwrap(), "Hair Silk Shine [nexus-42]");
        let old = file_folder(&first, 99).unwrap();
        assert_eq!(
            mod_folder(&downloads, &info(42, 100), "New title").unwrap(),
            first
        );
        assert_ne!(file_folder(&first, 100).unwrap(), old);
        let renamed = downloads.join("My favorite hair");
        fs::rename(&first, &renamed).unwrap();
        assert_eq!(
            mod_folder(&downloads, &info(42, 101), "Other title").unwrap(),
            renamed.canonicalize().unwrap()
        );
        let other = mod_folder(&downloads, &info(43, 99), "Hair Silk Shine").unwrap();
        assert_ne!(other, renamed);
        assert_eq!(fs::read_dir(&downloads).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repeated_file_reuses_verified_archive_and_preserves_changed_or_unrecognised_files() {
        let (root, _, _) = crate::lifecycle_tests::fixture();
        let downloads = root.join("downloads");
        let info = info(42, 99);
        let archive = save(&downloads, &info, "Hair", b"original");
        assert_eq!(
            cached_file(&downloads, &info).unwrap(),
            Some((archive.clone(), 8))
        );
        assert!(PendingArchive::new(archive.parent().unwrap(), "mod.zip").is_err());
        fs::write(&archive, b"modified").unwrap(); // Same byte count must still be rejected.
        assert!(cached_file(&downloads, &info).is_err());
        assert_eq!(fs::read(&archive).unwrap(), b"modified");
        let parent = find_mod_for(&downloads, 42, "nexus").unwrap().unwrap();
        fs::write(parent.join(MOD_RECORD), b"broken").unwrap();
        assert!(mod_folder(&downloads, &info, "Different title").is_err());
        assert_eq!(fs::read_dir(&downloads).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_or_failed_attempt_keeps_other_versions_and_unowned_targets() {
        let (root, _, _) = crate::lifecycle_tests::fixture();
        let downloads = root.join("downloads");
        let old = save(&downloads, &info(42, 99), "Hair", b"saved version");
        let parent = mod_folder(&downloads, &info(42, 100), "Hair").unwrap();
        let folder = file_folder(&parent, 100).unwrap();
        let pending = PendingArchive::new(&folder, "mod.zip").unwrap();
        let partial = pending.partial.clone();
        pending.create().unwrap().write_all(b"incomplete").unwrap();
        drop(pending);
        assert!(!partial.exists());
        assert_eq!(fs::read(&old).unwrap(), b"saved version");
        // If a file appears during transfer, publishing must not overwrite it.
        let pending = PendingArchive::new(&folder, "mod.zip").unwrap();
        pending.create().unwrap().write_all(b"new").unwrap();
        fs::write(folder.join("mod.zip"), b"user file").unwrap();
        assert!(pending
            .finish(serde_json::json!({}), 3, "test".into())
            .is_err());
        assert_eq!(fs::read(folder.join("mod.zip")).unwrap(), b"user file");
        // A failed metadata commit also cleans only this attempt's archive.
        let pending = PendingArchive::new(&folder, "other.zip").unwrap();
        pending.create().unwrap().write_all(b"new").unwrap();
        fs::write(folder.join(FILE_RECORD), b"keep metadata").unwrap();
        assert!(pending
            .finish(serde_json::json!({}), 3, "test".into())
            .is_err());
        assert!(!folder.join("other.zip").exists());
        assert_eq!(
            fs::read(folder.join(FILE_RECORD)).unwrap(),
            b"keep metadata"
        );
        assert_eq!(fs::read(&old).unwrap(), b"saved version");
        fs::remove_dir_all(root).unwrap();
    }

    fn zip_bytes(version: &str, payload: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("Hair/mod_manifest.json", options).unwrap();
        zip.write_all(
            serde_json::json!({"ProjectName":"Hair_ABC", "FriendlyName":"Hair", "Version":version})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        zip.start_file("Hair/payload.pak", options).unwrap();
        zip.write_all(payload).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn downloaded_versions_import_in_place_and_repeat_import_preserves_user_state() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        fs::remove_dir_all(&source).unwrap();
        let downloads = root.join("chosen root");
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.downloads_dir = downloads.to_string_lossy().into_owned();
        save_settings_core(settings, &state).unwrap();
        let archive1 = save(
            &downloads,
            &info(42, 99),
            "Hair",
            &zip_bytes("1", b"version one"),
        );
        import_download_core(&archive1.to_string_lossy(), &state).unwrap();
        let first = state.store.lock().unwrap().mods[0].clone();
        assert_eq!(
            Path::new(&first.path).parent().unwrap(),
            archive1.parent().unwrap()
        );
        assert_eq!(
            first.library_root.as_deref(),
            downloads.canonicalize().unwrap().to_str()
        );
        state.store.lock().unwrap().mods[0].name = "My custom title".into();
        toggle_mod_core(first.id.clone(), &state).unwrap();
        import_download_core(&archive1.to_string_lossy(), &state).unwrap();
        {
            let store = state.store.lock().unwrap();
            assert_eq!(store.mods.len(), 1);
            assert_eq!(store.mods[0].id, first.id);
            assert!(store.mods[0].enabled);
            assert_eq!(store.mods[0].name, "My custom title");
        }
        let archive2 = save(
            &downloads,
            &info(42, 100),
            "Renamed Hair",
            &zip_bytes("2", b"version two"),
        );
        import_download_core(&archive2.to_string_lossy(), &state).unwrap();
        let second = state.store.lock().unwrap().mods[1].clone();
        assert_eq!(second.version, "2");
        assert!(!second.enabled);
        assert_eq!(fs::read_dir(&downloads).unwrap().count(), 1);
        assert!(toggle_mod_core(second.id.clone(), &state).is_err());
        assert_eq!(
            fs::read(Path::new(&first.path).join("payload.pak")).unwrap(),
            b"version one"
        );
        toggle_mod_core(first.id.clone(), &state).unwrap();
        toggle_mod_core(second.id.clone(), &state).unwrap();
        let store: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert_eq!(store.mods.len(), 2);
        assert_eq!(library::catalogue(&store).0.len(), 2);
        assert!(archive1.is_file() && archive2.is_file());
        assert!(!root.join("library/Nexus").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
