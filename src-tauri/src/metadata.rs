use super::*;
use std::collections::BTreeMap;
use std::io::Read;

pub(crate) fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(err)?;
    let mut bytes = vec![];
    file.take(limit + 1).read_to_end(&mut bytes).map_err(err)?;
    if bytes.len() as u64 > limit {
        return Err("Metadata file is too large".into());
    }
    Ok(bytes)
}

pub(crate) fn download_record(info: &NexusLink, file: &serde_json::Value) -> serde_json::Value {
    // Persist the public page and file metadata, never the signed NXM request,
    // API key or download mirror URL.
    serde_json::json!({"provider":info.provider,"url": info.url, "mod_id": info.mod_id, "file_id": info.file_id, "file": file})
}

pub(crate) fn download_sidecar(folder: &Path) -> PathBuf {
    let curseforge = folder.join(".inzoimm-curseforge.json");
    if curseforge.is_file() {
        curseforge
    } else {
        folder.join(".inzoimm-nexus.json")
    }
}

// The download's own directory holds both the archive and its unpacked mods.
// Keep the configured download root as the grouping/removal boundary, even if
// the user selects a new default while this download is running.
pub(crate) fn download_destination(archive: &Path) -> Result<(PathBuf, PathBuf)> {
    let folder = archive.parent().ok_or("Missing download folder")?;
    let record = manifest_value(&read_limited(&download_sidecar(folder), 2 * 1024 * 1024)?)?;
    if let Some(root) = download_storage::download_root(folder, &record)? {
        return Ok((folder.to_path_buf(), root));
    }
    let mod_id = record["mod_id"].as_u64().ok_or("Missing download mod ID")?;
    let file_id = record["file_id"]
        .as_u64()
        .ok_or("Missing download file ID")?;
    let name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid download folder")?;
    let prefix = format!("{mod_id}-{file_id}-");
    let id = name
        .strip_prefix(&prefix)
        .ok_or("Not an app download folder")?;
    uuid::Uuid::parse_str(id).map_err(|_| "Not an app download folder")?;
    let root = folder.parent().ok_or("Missing downloads root")?;
    Ok((folder.to_path_buf(), root.to_path_buf()))
}

pub(crate) fn attach_download(archive: &Path, extracted: &Path) -> Result<()> {
    let Some(parent) = archive.parent() else {
        return Ok(());
    };
    let record = download_sidecar(parent);
    if !record.is_file() {
        return Ok(());
    }
    let download = manifest_value(&read_limited(&record, 2 * 1024 * 1024)?)?;
    for m in scan_with_options(extracted, true).0 {
        let path = Path::new(&m.path).join(if download["provider"] == "curseforge" {
            "curseforge.json"
        } else {
            "nexus.json"
        });
        let mut metadata = if path.is_file() {
            manifest_value(&read_limited(&path, 2 * 1024 * 1024)?)?
        } else {
            serde_json::json!({})
        };
        if !metadata.is_object() {
            metadata = serde_json::json!({"Archive metadata":metadata});
        }
        metadata["Download"] = download.clone();
        fs::write(path, serde_json::to_vec_pretty(&metadata).map_err(err)?).map_err(err)?;
    }
    Ok(())
}

pub(crate) fn read(
    folder: &Path,
    manifest: serde_json::Value,
) -> BTreeMap<String, serde_json::Value> {
    let mut result = BTreeMap::from([("mod_manifest.json".into(), manifest)]);
    for name in ["metadata.json", "nexus.json", "curseforge.json", "meta.ini"] {
        let path = folder.join(name);
        if !path.is_file() {
            continue;
        }
        let parsed = read_limited(&path, 2 * 1024 * 1024).and_then(|bytes| {
            if name != "meta.ini" {
                return manifest_value(&bytes);
            }
            let text = String::from_utf8(bytes).map_err(err)?;
            let mut section = String::new();
            let mut fields = serde_json::Map::new();
            for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
                if line.starts_with([';', '#']) || line.is_empty() {
                    continue;
                }
                if line.starts_with('[') && line.ends_with(']') {
                    section = line[1..line.len() - 1].to_string();
                } else if let Some((key, value)) = line.split_once('=') {
                    let key = if section.is_empty() {
                        key.trim().to_string()
                    } else {
                        format!("{section}.{}", key.trim())
                    };
                    fields.insert(key, value.trim().into());
                }
            }
            Ok(fields.into())
        });
        result.insert(
            name.into(),
            parsed.unwrap_or_else(|error| serde_json::json!({"Read error": error})),
        );
    }
    result
}

pub(crate) fn from_game_library(path: &Path) -> Result<BTreeMap<String, serde_json::Value>> {
    let library = manifest_value(&read_limited(path, 16 * 1024 * 1024)?)?;
    let mut result = BTreeMap::new();
    for installed in library
        .get("installedMods")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let Some(details) = installed.get("details") else {
            continue;
        };
        if value(details, "gameId").and_then(|v| v.as_u64()) != Some(88849) {
            continue;
        }
        let installed_file = value(installed, "installedFile");
        let mut saved_details = details.clone();
        if let Some(file) = installed_file {
            saved_details["Installed file"] = crate::curseforge::public_metadata(file.clone());
        }
        for file in installed_file.into_iter().chain(
            details
                .get("latestFiles")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten(),
        ) {
            for module in file
                .get("modules")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                if let Some(project) = module
                    .get("name")
                    .and_then(|v| v.as_str())
                    .filter(|name| safe_project(name))
                {
                    result
                        .entry(project.to_lowercase())
                        .or_insert_with(|| saved_details.clone());
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_all_local_fields_and_game_details_without_needing_a_logo() {
        let (root, _, source) = crate::lifecycle_tests::fixture();
        fs::write(
            source.join("meta.ini"),
            "[General]\nmodID=42\nurl=https://www.nexusmods.com/inzoi/mods/42\nnotes=Favorites",
        )
        .unwrap();
        let m = read_mod(&source, "test".into(), false).unwrap();
        assert_eq!(m.metadata["meta.ini"]["General.modID"], "42");
        let path = root.join("library.json");
        fs::write(&path, r#"{"installedMods":[{"details":{"gameId":88849,"summary":"Extra details","links":{"websiteUrl":"https://example.com/mod"},"latestFiles":[{"modules":[{"name":"Hair_ABC"}]}]}}]}"#).unwrap();
        let details = from_game_library(&path).unwrap();
        assert_eq!(details["hair_abc"]["summary"], "Extra details");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn downloaded_nexus_provenance_survives_import_without_signed_request_secrets() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let request = nexus::parse_request(
            "nxm://inzoi/mods/42/files/99?key=secret&expires=9999999999&user_id=12",
        )
        .unwrap();
        let record = download_record(
            &request.info,
            &serde_json::json!({"name":"Outfit", "version":"2.1"}),
        );
        assert!(!record.to_string().contains("secret"));
        fs::write(root.join(".inzoimm-nexus.json"), record.to_string()).unwrap();
        attach_download(&root.join("download.zip"), source.parent().unwrap()).unwrap();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let store = state.store.lock().unwrap();
        assert_eq!(
            store.mods[0].metadata["nexus.json"]["Download"]["url"],
            "https://www.nexusmods.com/inzoi/mods/42?tab=files&file_id=99"
        );
        assert_eq!(
            store.mods[0].metadata["nexus.json"]["Download"]["file"]["version"],
            "2.1"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
