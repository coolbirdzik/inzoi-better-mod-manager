use super::*;

fn folder_path(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err("Choose an absolute folder path".into());
    }
    let folder = path
        .canonicalize()
        .map_err(|error| format!("Could not open folder {}: {error}", path.display()))?;
    if !folder.is_dir() {
        return Err("This path is not a folder".into());
    }
    Ok(dunce::simplified(&folder).to_path_buf())
}

// Folder buttons accept user-selected locations on any drive. Keep this a
// directory-only native command instead of granting the webview arbitrary file
// opening or arbitrary program execution through the opener plugin's scope.
#[tauri::command(async)]
pub(crate) fn open_folder(path: String) -> Result<()> {
    let folder = folder_path(&path)?;
    #[cfg(target_os = "windows")]
    let program = Some("explorer");
    #[cfg(target_os = "macos")]
    let program = Some("Finder");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = None::<&str>;
    tauri_plugin_opener::open_path(folder, program).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_with_spaces_unicode_and_svg_suffixes_keep_their_exact_names() {
        let (root, _, _) = crate::lifecycle_tests::fixture();
        for name in ["InZoi mod/Downloads", "Thư mục mod/Downloadssvg"] {
            let path = root.join(name);
            fs::create_dir_all(&path).unwrap();
            assert_eq!(folder_path(&path.to_string_lossy()).unwrap(), path);
            #[cfg(windows)]
            assert_eq!(
                folder_path(&path.to_string_lossy().replace('\\', "/")).unwrap(),
                path
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_paths_files_relative_paths_and_urls_are_not_launched_or_created() {
        let (root, _, source) = crate::lifecycle_tests::fixture();
        let missing = root.join("Downloadssvg");
        for path in [
            missing.to_string_lossy().into_owned(),
            source.join("payload.pak").to_string_lossy().into_owned(),
            "Downloads".into(),
            "https://example.com".into(),
        ] {
            assert!(folder_path(&path).is_err());
        }
        assert!(!missing.exists());
        assert_eq!(fs::read(source.join("payload.pak")).unwrap(), [1, 2, 3, 4]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Opens the existing folder in INZOIMM_OPEN_FOLDER with the system file manager"]
    fn opens_selected_folder_in_the_real_file_manager() {
        let path = std::env::var("INZOIMM_OPEN_FOLDER")
            .expect("Set INZOIMM_OPEN_FOLDER to an existing directory");
        open_folder(path).unwrap();
    }
}
