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
        if m.thumbnail
            .as_ref()
            .is_none_or(|path| !Path::new(path).is_file())
        {
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
