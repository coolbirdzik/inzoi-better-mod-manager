use super::*;
use std::collections::BTreeMap;

// Completed Nexus archives remain library items even when this app cannot
// extract/install their format. Their existing sidecars survive app restarts.
pub(crate) fn scan(root: &Path, recursive: bool) -> Vec<Mod> {
    if !root.is_dir() {
        return vec![];
    }
    WalkDir::new(root)
        .follow_links(false)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || entry.file_name() != ".trash")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && (entry.file_name() == ".inzoimm-nexus.json"
                    || entry.file_name() == ".inzoimm-curseforge.json")
        })
        .filter_map(|entry| item(entry.path(), root).ok())
        .collect()
}

fn item(sidecar: &Path, root: &Path) -> Result<Mod> {
    let record = manifest_value(&metadata::read_limited(sidecar, 2 * 1024 * 1024)?)?;
    let curseforge = record["provider"] == "curseforge";
    let mod_id = record["mod_id"].as_u64().ok_or("Missing mod ID")?;
    let file_id = record["file_id"].as_u64().ok_or("Missing file ID")?;
    let name = record["archive"]
        .as_str()
        .or_else(|| record["file"]["file_name"].as_str())
        .or_else(|| record["file"]["fileName"].as_str())
        .filter(|name| safe_project(name))
        .ok_or("Missing archive name")?;
    let folder = sidecar
        .parent()
        .ok_or("Missing archive folder")?
        .canonicalize()
        .map_err(err)?;
    let path = folder.join(name).canonicalize().map_err(err)?;
    if !path.is_file() || !path.starts_with(&folder) {
        return Err("Archive is outside its download folder".into());
    }
    metadata::download_destination(&path)?;
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["zip", "rar", "7z"].contains(&extension.as_str()) {
        return Err("Unsupported archive type".into());
    }
    let title = record["file"]["name"]
        .as_str()
        .or_else(|| record["file"]["displayName"].as_str())
        .or_else(|| record["mod"]["name"].as_str())
        .unwrap_or(name)
        .to_string();
    let author = record["mod"]["author"]
        .as_str()
        .or_else(|| record["mod"]["authors"][0]["name"].as_str())
        .unwrap_or("")
        .to_string();
    let version = record["file"]["version"].as_str().unwrap_or("").to_string();
    let description = record["mod"]["summary"].as_str().unwrap_or("").to_string();
    let metadata = BTreeMap::from([(
        (if curseforge {
            "curseforge.json"
        } else {
            "nexus.json"
        })
        .into(),
        serde_json::json!({"Download":record}),
    )]);
    Ok(Mod {
        id: format!("archive:{}", path.to_string_lossy()),
        name: title,
        author,
        version,
        description,
        thumbnail: thumbnails::from_nexus(&metadata)
            .or_else(|| thumbnails::from_curseforge(&metadata)),
        path: path.to_string_lossy().into_owned(),
        project: format!(
            "{}Download_{mod_id}",
            if curseforge { "CurseForge" } else { "Nexus" }
        ),
        source: record["url"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!("https://www.nexusmods.com/inzoi/mods/{mod_id}?tab=files&file_id={file_id}")
            }),
        managed: false,
        enabled: false,
        deployed_path: None,
        library_root: Some(root.to_string_lossy().into_owned()),
        metadata,
        tags: vec![],
        archive: true,
        previous_version: None,
        superseded_by: None,
        added_at: path_added_at(&path),
        legacy: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(state: &State, root: &Path, filename: &str) -> PathBuf {
        let info = nexus::parse_request("nxm://inzoi/mods/651/files/2669")
            .unwrap()
            .info;
        let downloads = root.join("downloads");
        let parent = download_storage::mod_folder(&downloads, &info, "Sample Mod").unwrap();
        let folder = download_storage::file_folder(&parent, 2669).unwrap();
        fs::write(folder.join(filename), b"original archive bytes").unwrap();
        let mut record = metadata::download_record(
            &info,
            &serde_json::json!({"name":"Sample Mod AIO","file_name":filename,"version":"3.7"}),
        );
        record["archive"] = filename.into();
        record["mod"] = serde_json::json!({"name":"Sample Mod","author":"Creator","picture_url":"https://staticdelivery.nexusmods.com/mods/7480/images/651/image.jpg"});
        fs::write(folder.join(".inzoimm-nexus.json"), record.to_string()).unwrap();
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.downloads_dir = downloads.to_string_lossy().into_owned();
        settings.source_dirs = vec![downloads.to_string_lossy().into_owned().into()];
        save_settings_core(settings, state).unwrap();
        folder
    }

    #[test]
    fn deleting_archive_keeps_recoverable_bytes_and_does_not_reappear_on_scan() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let folder = fixture(&state, &root, "sample.rar");
        let archive = library::catalogue(&state.store.lock().unwrap())
            .0
            .into_iter()
            .find(|m| m.archive)
            .unwrap();
        assert!(set_mod_enabled_core(archive.id.clone(), true, &state).is_err());
        remove_mod_core(archive.id, &state).unwrap();
        assert!(!folder.join("sample.rar").exists());
        assert!(!folder.join(".inzoimm-nexus.json").exists());
        assert!(!library::catalogue(&state.store.lock().unwrap())
            .0
            .iter()
            .any(|m| m.archive));
        let recovery = fs::read_dir(root.join("downloads/.trash"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(recovery.join("sample.rar")).unwrap(),
            b"original archive bytes"
        );
        assert!(recovery.join(".inzoimm-nexus.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rar_downloads_survive_reload_with_metadata_thumbnail_and_tags_without_being_enabled() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let folder = fixture(&state, &root, "sample.rar");
        let first = library::catalogue(&state.store.lock().unwrap()).0;
        let archives: Vec<_> = first.iter().filter(|m| m.archive).collect();
        assert_eq!(archives.len(), 1); // Overlapping sources must not duplicate it.
        let m = archives[0];
        assert!(m.added_at > 0);
        assert!(!m.enabled && !m.managed);
        assert_eq!(m.name, "Sample Mod AIO");
        assert_eq!(m.version, "3.7");
        assert!(m
            .thumbnail
            .as_deref()
            .is_some_and(|v| v.starts_with("https://staticdelivery.nexusmods.com/")));
        library::set_mod_tags_core(m.id.clone(), vec!["To install".into()], &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        let reloaded = library::catalogue(&saved).0;
        let restored = reloaded.iter().find(|m| m.id == archives[0].id).unwrap();
        assert_eq!(restored.added_at, m.added_at);
        assert_eq!(restored.tags, vec!["To install"]);
        assert_eq!(restored.metadata["nexus.json"]["Download"]["file_id"], 2669);
        assert_eq!(
            fs::read(folder.join("sample.rar")).unwrap(),
            b"original archive bytes"
        );
        assert!(saved.mods.iter().all(|m| !m.archive)); // No fabricated installed mod.
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn imported_folders_replace_archive_cards_and_removing_them_reveals_the_saved_archive() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let folder = fixture(&state, &root, "sample.zip");
        let extracted = folder.join("RecognizedMod");
        copy_tree(&source, &extracted).unwrap();
        assert!(!library::catalogue(&state.store.lock().unwrap())
            .0
            .iter()
            .any(|m| m.archive));
        fs::remove_dir_all(&extracted).unwrap();
        assert_eq!(
            library::catalogue(&state.store.lock().unwrap())
                .0
                .iter()
                .filter(|m| m.archive)
                .count(),
            1
        );
        assert!(folder.join("sample.zip").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_missing_or_non_archive_targets_are_not_catalogued() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let folder = fixture(&state, &root, "sample.7z");
        let marker = folder.join(".inzoimm-nexus.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
        for filename in ["../outside.rar", "missing.rar", "program.exe"] {
            record["archive"] = filename.into();
            fs::write(&marker, record.to_string()).unwrap();
            assert!(scan(&root, true).is_empty());
        }
        assert!(folder.join("sample.7z").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Read-only diagnostic for a saved Nexus download selected with INZOIMM_ARCHIVE_ROOT"]
    fn saved_download_is_visible_as_an_archive() {
        let root =
            PathBuf::from(std::env::var("INZOIMM_ARCHIVE_ROOT").expect("Set INZOIMM_ARCHIVE_ROOT"));
        let items = scan(&root, true);
        assert!(items.iter().any(|m| m.archive
            && m.metadata["nexus.json"]["Download"]["mod_id"] == 651
            && m.metadata["nexus.json"]["Download"]["file_id"] == 2669));
        println!("Nexus mod #651, file #2669 is visible as a saved archive");
    }
}
