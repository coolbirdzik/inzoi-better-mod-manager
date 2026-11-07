use super::*;
use std::collections::BTreeMap;
use std::io::Read;

fn trusted_image(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("media.forgecdn.net")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none())
    .then(|| url.to_string())
}

fn nexus_image(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("staticdelivery.nexusmods.com" | "images.nexusmods.com")
        )
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none())
    .then(|| url.to_string())
}

pub(crate) fn from_nexus(metadata: &BTreeMap<String, serde_json::Value>) -> Option<String> {
    let record = metadata.get("nexus.json")?;
    [
        "/Download/mod/picture_url",
        "/mod/picture_url",
        "/picture_url",
    ]
    .into_iter()
    .filter_map(|key| record.pointer(key).and_then(|value| value.as_str()))
    .find_map(nexus_image)
}

pub(crate) fn from_curseforge(metadata: &BTreeMap<String, serde_json::Value>) -> Option<String> {
    let record = metadata
        .get("curseforge.json")
        .or_else(|| metadata.get("Game library"))?;
    [
        "/Download/mod/logo/thumbnailUrl",
        "/Download/mod/logo/url",
        "/mod/logo/thumbnailUrl",
        "/logo/thumbnailUrl",
        "/logo/url",
    ]
    .into_iter()
    .filter_map(|key| record.pointer(key).and_then(|v| v.as_str()))
    .find_map(trusted_image)
}

// inZOI keeps CurseForge logos outside the installed mod folders. Match the
// package's exact project name, never the mod's human-readable display name.
pub(crate) fn from_game_library(path: &Path) -> Result<BTreeMap<String, String>> {
    let file = fs::File::open(path).map_err(err)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Game mod metadata exceeds 16 MB".into());
    }
    let library = manifest_value(&bytes)?;
    let mut images = BTreeMap::new();
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
        let Some(logo) = details.get("logo") else {
            continue;
        };
        let Some(image) = ["thumbnailUrl", "url"].into_iter().find_map(|key| {
            value(logo, key)
                .and_then(|v| v.as_str())
                .and_then(trusted_image)
        }) else {
            continue;
        };
        for file in details
            .get("latestFiles")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
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
                    images.insert(project.to_lowercase(), image.clone());
                }
            }
        }
    }
    Ok(images)
}

pub(crate) fn apply(mods: &mut [Mod], images: &BTreeMap<String, String>) {
    for m in mods {
        if m.thumbnail.as_ref().is_none_or(|path| {
            !Path::new(path).is_file()
                && nexus_image(path).is_none()
                && trusted_image(path).is_none()
        }) {
            if let Some(url) = images.get(&m.project.to_lowercase()) {
                m.thumbnail = Some(url.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_nexus_image_appears_for_existing_managed_mods_without_overwriting_local_images() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let image = "https://staticdelivery.nexusmods.com/mods/6919/images/476/preview.jpg";
        fs::write(source.join("nexus.json"), serde_json::json!({"Download":{"mod_id":476,"mod":{"picture_url":image,"uploaded_users_profile_url":"https://images.nexusmods.com/avatars/author.jpg"}}}).to_string()).unwrap();
        import_folder_core(source.to_string_lossy().into_owned(), &state).unwrap();
        let mut saved = state.store.lock().unwrap().clone();
        let managed = PathBuf::from(&saved.mods[0].path);
        // Older library records did not persist a thumbnail even though their
        // local nexus.json already contained the primary image.
        saved.mods[0].thumbnail = None;
        let mut mods = library::catalogue(&saved).0;
        let m = mods.iter().find(|m| m.managed).unwrap();
        assert_eq!(m.thumbnail.as_deref(), Some(image));
        apply(
            &mut mods,
            &BTreeMap::from([(
                "hair_abc".into(),
                "https://media.forgecdn.net/avatars/other.jpg".into(),
            )]),
        );
        assert_eq!(
            mods.iter()
                .find(|m| m.managed)
                .unwrap()
                .thumbnail
                .as_deref(),
            Some(image)
        );
        let local = managed.join("thumbnail.png");
        fs::write(&local, b"local image").unwrap();
        assert_eq!(
            read_mod(&managed, "test".into(), true)
                .unwrap()
                .thumbnail
                .as_deref(),
            local.to_str()
        );
        assert_eq!(
            library::catalogue(&saved)
                .0
                .iter()
                .find(|m| m.managed)
                .unwrap()
                .thumbnail
                .as_deref(),
            local.to_str()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nexus_image_sources_are_restricted_to_public_nexus_cdn_urls() {
        for raw in [
            "http://staticdelivery.nexusmods.com/image.jpg",
            "https://staticdelivery.nexusmods.com.attacker.test/image.jpg",
            "https://user:secret@staticdelivery.nexusmods.com/image.jpg",
            "https://staticdelivery.nexusmods.com:8443/image.jpg",
            "https://localhost/image.jpg",
            "file:///private/image.jpg",
        ] {
            assert!(nexus_image(raw).is_none(), "{raw}");
        }
        assert!(nexus_image("https://staticdelivery.nexusmods.com/mods/6919/image.jpg").is_some());
        assert!(nexus_image("https://images.nexusmods.com/image.jpg").is_some());
        let metadata = BTreeMap::from([(
            "nexus.json".into(),
            serde_json::json!({"Download":{"mod":{"picture_url":null}}}),
        )]);
        assert!(from_nexus(&metadata).is_none());
    }

    #[test]
    fn game_metadata_maps_logos_by_exact_project_and_keeps_local_images() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let metadata = root.join("library.json");
        fs::write(&metadata, r#"{"installedMods":[{"details":{"gameId":88849,"logo":{"thumbnailUrl":"https://media.forgecdn.net/avatars/hair.jpg"},"latestFiles":[{"modules":[{"name":"HAIR_ABC"}]}]}}]}"#).unwrap();
        let content = fs::read(&metadata).unwrap();
        fs::write(&metadata, [b"\xef\xbb\xbf".as_slice(), &content].concat()).unwrap();
        let images = from_game_library(&metadata).unwrap();
        let mut mods = scan(Path::new(&state.store.lock().unwrap().settings.mods_dir)).0;
        apply(&mut mods, &images);
        assert_eq!(
            mods[0].thumbnail.as_deref(),
            Some("https://media.forgecdn.net/avatars/hair.jpg")
        );
        let local = source.join("thumbnail.png");
        fs::write(&local, b"image").unwrap();
        mods[0].thumbnail = Some(local.to_string_lossy().into_owned());
        apply(&mut mods, &images);
        assert_eq!(mods[0].thumbnail.as_deref(), local.to_str());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_public_forgecdn_https_images_are_accepted() {
        for url in [
            "http://media.forgecdn.net/image.png",
            "https://media.forgecdn.net.evil.test/image.png",
            "https://user:secret@media.forgecdn.net/image.png",
            "https://localhost/image.png",
            "file:///private/image.png",
        ] {
            assert!(trusted_image(url).is_none());
        }
        assert!(trusted_image("https://media.forgecdn.net/avatars/image.png").is_some());
    }
}
