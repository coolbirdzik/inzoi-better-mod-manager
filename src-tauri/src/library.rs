use super::*;
use std::collections::HashSet;

fn path_key(raw: &str) -> String {
    let path = Path::new(raw)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(raw));
    let text = dunce::simplified(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let text = text.trim_end_matches('/').to_string();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

pub(crate) fn same_path(left: &str, right: &str) -> bool {
    path_key(left) == path_key(right)
}

pub(crate) fn validate_sources(settings: &Settings) -> Result<()> {
    if !settings.downloads_dir.is_empty() && !Path::new(&settings.downloads_dir).is_absolute() {
        return Err("Use an absolute download folder path".into());
    }
    for (index, source) in settings.source_dirs.iter().enumerate() {
        let raw = &source.path;
        if !Path::new(raw).is_absolute() {
            return Err("Use absolute source folder paths".into());
        }
        if same_path(raw, &settings.mods_dir)
            || same_path(raw, &settings.library_dir)
            || settings.source_dirs[..index]
                .iter()
                .any(|other| same_path(raw, &other.path))
        {
            return Err("This folder is already a library source".into());
        }
    }
    Ok(())
}

// Scanning is read-only. Explicit activation/removal registers the selected
// source mod for management. Overlapping sources are de-duplicated.
pub(crate) fn scan_sources(settings: &Settings) -> (Vec<Mod>, Vec<String>) {
    let mut mods = vec![];
    let mut warnings = vec![];
    let mut seen = HashSet::new();
    let game = Path::new(&settings.mods_dir).canonicalize().ok();
    let downloads = nexus::downloads_root(settings);
    let roots: Vec<_> = std::iter::once((Path::new(&settings.mods_dir), true))
        .chain(
            settings
                .source_dirs
                .iter()
                .map(|source| (Path::new(&source.path), source.scan_subfolders)),
        )
        .chain(downloads.exists().then_some((downloads.as_path(), true)))
        .collect();
    for (root, recursive) in &roots {
        let (found, issues) = scan_with_options(root, *recursive);
        warnings.extend(issues);
        for mut m in found {
            let path = Path::new(&m.path)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&m.path));
            let key = dunce::simplified(&path).to_path_buf();
            if seen.insert(key) {
                m.enabled &= game.as_ref().is_some_and(|game| path.starts_with(game));
                mods.push(m);
            }
        }
    }
    for (root, recursive) in roots.into_iter().chain(std::iter::once((
        Path::new(&settings.library_dir),
        settings.scan_subfolders,
    ))) {
        for m in download_catalogue::scan(root, recursive) {
            let path = Path::new(&m.path)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&m.path));
            if seen.insert(dunce::simplified(&path).to_path_buf()) {
                mods.push(m);
            }
        }
    }
    if !settings.paks().is_empty() {
        let (found, issues) = legacy::scan(Path::new(settings.paks()));
        warnings.extend(issues);
        for m in found {
            if seen.insert(PathBuf::from(&m.path)) {
                mods.push(m);
            }
        }
    }
    (mods, warnings)
}

pub(crate) fn catalogue(store: &Store) -> (Vec<Mod>, Vec<String>) {
    let (external, warnings) = scan_sources(&store.settings);
    let mut mods = store.mods.clone();
    let mut seen = HashSet::new();
    for m in &mut mods {
        seen.insert(path_key(&m.path));
        if m.enabled {
            let deployed = m.deployed_path.clone().unwrap_or_else(|| {
                Path::new(&store.settings.mods_dir)
                    .join(&m.project)
                    .to_string_lossy()
                    .into_owned()
            });
            seen.insert(path_key(&deployed));
        }
        // Managed names/descriptions can be edited independently; refresh the
        // original metadata without overwriting those app-level edits.
        if let Ok(current) = read_mod(Path::new(&m.path), m.id.clone(), m.managed) {
            if m.added_at == 0 {
                m.added_at = current.added_at;
            }
            if m.thumbnail
                .as_ref()
                .is_none_or(|path| !Path::new(path).is_file())
            {
                m.thumbnail = current.thumbnail;
            }
            m.metadata = current.metadata;
            if m.source.is_empty() {
                m.source = current.source;
            }
        }
    }
    for m in external {
        if seen.insert(path_key(&m.path)) {
            mods.push(m);
        }
    }
    // Imported projects already represent this archive in the library. Keep
    // the standalone archive card only while it has no recognised mod folder.
    let unpacked: Vec<_> = mods
        .iter()
        .filter(|m| !m.archive)
        .map(|m| path_key(&m.path))
        .collect();
    mods.retain(|m| {
        !m.archive
            || !Path::new(&m.path).parent().is_some_and(|folder| {
                let prefix = format!("{}/", path_key(&folder.to_string_lossy()));
                unpacked.iter().any(|path| path.starts_with(&prefix))
            })
    });
    for m in &mut mods {
        m.tags = store
            .tags
            .get(&m.project.to_lowercase())
            .cloned()
            .unwrap_or_default();
    }
    (mods, warnings)
}

pub(crate) fn set_mod_tags_core(id: String, tags: Vec<String>, state: &State) -> Result<()> {
    let mut store = state.store.lock().map_err(err)?;
    let project = store
        .mods
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.project.clone())
        .or_else(|| {
            scan_sources(&store.settings)
                .0
                .into_iter()
                .find(|m| m.id == id)
                .map(|m| m.project)
        })
        .ok_or("Mod no longer exists")?;
    let mut clean = vec![];
    let mut seen = HashSet::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > 60 || tag.chars().any(char::is_control) {
            return Err(
                "Tags must be at most 60 characters and contain no control characters".into(),
            );
        }
        if seen.insert(tag.to_lowercase()) {
            clean.push(tag.to_string());
        }
    }
    if clean.len() > 50 {
        return Err("Use at most 50 tags per mod".into());
    }
    let old = store.tags.clone();
    let key = project.to_lowercase();
    if clean.is_empty() {
        store.tags.remove(&key);
    } else {
        store.tags.insert(key, clean);
    }
    if let Err(error) = write_store(state, &store) {
        store.tags = old;
        return Err(error);
    }
    Ok(())
}

#[tauri::command(async)]
pub(crate) fn set_mod_tags(
    id: String,
    tags: Vec<String>,
    state: tauri::State<State>,
) -> Result<()> {
    set_mod_tags_core(id, tags, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn added_dates_survive_reload_and_legacy_catalogues_use_folder_creation_time() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let scanned = read_mod(&source, "source".into(), false).unwrap();
        assert!(scanned.added_at > 0);
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let mut saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        let original_date = saved.mods[0].added_at;
        assert!(original_date > 0);
        let id = saved.mods[0].id.clone();
        let date = |store: &Store| {
            catalogue(store)
                .0
                .into_iter()
                .find(|m| m.id == id)
                .unwrap()
                .added_at
        };
        assert_eq!(date(&saved), original_date);
        fs::write(
            Path::new(&saved.mods[0].path).join("thumbnail.png"),
            b"image",
        )
        .unwrap();
        assert_eq!(date(&saved), original_date);
        // Old saved stores did not have a date field; recover it without using
        // the time of each scan, so restarting never makes old mods look new.
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy["mods"][0]
            .as_object_mut()
            .unwrap()
            .remove("added_at");
        let legacy: Store = serde_json::from_value(legacy).unwrap();
        assert_eq!(date(&legacy), original_date);
        saved.mods[0].added_at = 1234;
        assert_eq!(date(&saved), 1234);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sources_preserve_files_deduplicate_overlaps_and_persist_tags_across_import() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let source = root.join("external/Clothing/Dresses/Outfit");
        fs::create_dir_all(&source).unwrap();
        let manifest = br#"{"ProjectName":"Outfit","FriendlyName":"Dress","bEnable":true,"NexusUrl":"https://www.nexusmods.com/inzoi/mods/42","Custom":{"rating":5}}"#;
        fs::write(source.join("mod_manifest.json"), manifest).unwrap();
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.source_dirs = vec![
            root.join("external").to_string_lossy().into_owned().into(),
            source
                .parent()
                .unwrap()
                .to_string_lossy()
                .into_owned()
                .into(),
        ];
        save_settings_core(settings, &state).unwrap();
        let (mods, warnings) = scan_sources(&state.store.lock().unwrap().settings);
        assert!(warnings.is_empty());
        assert_eq!(mods.len(), 2);
        let m = mods.iter().find(|m| m.project == "Outfit").unwrap();
        assert!(!m.managed && !m.enabled);
        assert_eq!(m.metadata["mod_manifest.json"]["Custom"]["rating"], 5);
        set_mod_tags_core(
            m.id.clone(),
            vec![" Clothes ".into(), "clothes".into(), "Favorites".into()],
            &state,
        )
        .unwrap();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert_eq!(saved.tags["outfit"], vec!["Clothes", "Favorites"]);
        assert_eq!(saved.settings.source_dirs.len(), 2);
        let catalogue = catalogue(&saved).0;
        assert_eq!(
            catalogue.iter().filter(|m| m.project == "Outfit").count(),
            2
        );
        assert!(catalogue
            .iter()
            .filter(|m| m.project == "Outfit")
            .all(|m| m.tags == ["Clothes", "Favorites"]));
        assert_eq!(
            fs::read(source.join("mod_manifest.json")).unwrap(),
            manifest
        );
        assert!(!source.join(".inzoimm-owner").exists());
        set_mod_tags_core(m.id.clone(), vec![], &state).unwrap();
        assert!(state.store.lock().unwrap().tags.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn old_settings_default_to_no_extra_sources_and_duplicate_sources_are_rejected() {
        let settings: Settings =
            serde_json::from_str(r#"{"mods_dir":"game","library_dir":"library"}"#).unwrap();
        assert!(settings.source_dirs.is_empty());
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.source_dirs.push(settings.mods_dir.clone().into());
        assert!(save_settings_core(settings, &state).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn each_source_keeps_its_scan_choice_and_legacy_sources_migrate() {
        let legacy: Settings = serde_json::from_str(r#"{"mods_dir":"game","library_dir":"library","source_dirs":["external",{"path":"other","scan_subfolders":false}]}"#).unwrap();
        assert!(legacy.source_dirs[0].scan_subfolders);
        assert!(!legacy.source_dirs[1].scan_subfolders);
        assert_eq!(
            serde_json::to_value(&legacy).unwrap()["source_dirs"][0]["path"],
            "external"
        );
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let first = root.join("first");
        let second = root.join("second");
        copy_tree(&source, &first.join("group/ModOne")).unwrap();
        copy_tree(&source, &second.join("group/ModTwo")).unwrap();
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.source_dirs = vec![
            SourceFolder {
                path: first.to_string_lossy().into_owned(),
                scan_subfolders: false,
            },
            SourceFolder {
                path: second.to_string_lossy().into_owned(),
                scan_subfolders: true,
            },
        ];
        save_settings_core(settings, &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        let found = scan_sources(&saved.settings).0;
        assert!(!found.iter().any(|m| Path::new(&m.path).starts_with(&first)));
        assert!(found
            .iter()
            .any(|m| Path::new(&m.path).starts_with(&second)));
        assert!(!saved.settings.source_dirs[0].scan_subfolders);
        assert!(saved.settings.source_dirs[1].scan_subfolders);
        assert!(!first.join("group/ModOne/.inzoimm-owner").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn configured_download_folder_is_an_automatic_source_without_copying_to_library() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let downloads = root.join("chosen-downloads");
        let unpacked = downloads.join("download-group/Hair_ABC");
        copy_tree(&source, &unpacked).unwrap();
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.downloads_dir = downloads.to_string_lossy().into_owned();
        settings.scan_subfolders = false;
        save_settings_core(settings, &state).unwrap();
        let store = state.store.lock().unwrap();
        let (mods, warnings) = catalogue(&store);
        assert!(warnings.is_empty());
        assert!(mods
            .iter()
            .any(|m| same_path(&m.path, &unpacked.to_string_lossy()) && !m.enabled));
        assert!(store.mods.is_empty());
        assert!(!Path::new(&store.settings.library_dir)
            .join("Nexus")
            .exists());
        assert!(unpacked.join("payload.pak").is_file());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalogue_hides_only_the_managed_deployment_and_refreshes_metadata() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let external = root.join("source");
        copy_tree(&source, &external).unwrap();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        fs::remove_dir_all(&source).unwrap();
        let id = state.store.lock().unwrap().mods[0].id.clone();
        toggle_mod_core(id, &state).unwrap();
        let mut store = state.store.lock().unwrap();
        store
            .settings
            .source_dirs
            .push(external.to_string_lossy().into_owned().into());
        fs::write(
            Path::new(&store.mods[0].path).join("metadata.json"),
            r#"{"NewField":"Refreshed"}"#,
        )
        .unwrap();
        let (mods, warnings) = catalogue(&store);
        assert!(warnings.is_empty());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods.iter().filter(|m| m.managed && m.enabled).count(), 1);
        assert_eq!(mods[0].metadata["metadata.json"]["NewField"], "Refreshed");
        assert!(mods
            .iter()
            .any(|m| same_path(&m.path, &external.to_string_lossy())));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}
