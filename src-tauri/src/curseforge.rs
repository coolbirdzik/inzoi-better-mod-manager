use super::*;
use std::time::Duration;

const GAME: u64 = 88849;
fn positive(raw: &str) -> Result<u64> {
    raw.parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("Invalid CurseForge ID".into())
}
fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 200
        && slug
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub(crate) fn parse_link(raw: &str) -> Result<NexusLink> {
    let url = url::Url::parse(raw).map_err(|_| "Paste an inZOI Nexus or CurseForge link")?;
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err("Invalid CurseForge URL".into());
    }
    let (mod_id, file_id, slug, public) = if url.scheme() == "curseforge"
        && url.host_str() == Some("install")
    {
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        if params.get("gameId").map(|id| id.as_ref()) != Some("88849") {
            return Err("Only inZOI CurseForge mods are supported".into());
        }
        let mod_id = positive(
            params
                .get("addonId")
                .ok_or("Missing CurseForge project ID")?,
        )?;
        let file_id = params
            .get("fileId")
            .map(|id| positive(id))
            .transpose()?
            .unwrap_or(0);
        (
            mod_id,
            file_id,
            None,
            format!("https://www.curseforge.com/projects/{mod_id}"),
        )
    } else {
        if url.scheme() != "https"
            || !matches!(
                url.host_str(),
                Some("curseforge.com" | "www.curseforge.com")
            )
        {
            return Err("Paste an inZOI Nexus or CurseForge link".into());
        }
        let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
        if parts.len() < 3 || parts[0] != "inzoi" || parts[1] != "mods" || !valid_slug(parts[2]) {
            return Err("Only inZOI CurseForge mod pages and file links are supported".into());
        }
        let file_id = match parts.as_slice() {
            [_, _, _] | [_, _, _, "files"] => 0,
            [_, _, _, "files" | "download", id] => positive(id)?,
            [_, _, _, "files", id, "download"] => positive(id)?,
            _ => return Err("Choose a CurseForge mod page or a specific file link".into()),
        };
        let public = format!(
            "https://www.curseforge.com/inzoi/mods/{}{}",
            parts[2],
            if file_id > 0 {
                format!("/files/{file_id}")
            } else {
                String::new()
            }
        );
        (0, file_id, Some(parts[2].to_string()), public)
    };
    Ok(NexusLink {
        provider: "curseforge".into(),
        slug,
        request_id: uuid::Uuid::new_v4().to_string(),
        game: "inzoi".into(),
        mod_id,
        file_id,
        url: public,
    })
}

pub(crate) fn api_client() -> Result<reqwest::Client> {
    nexus::configure_tls();
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|_| "Could not initialize CurseForge client".into())
}

pub(crate) async fn api_data(
    client: &reqwest::Client,
    url: url::Url,
    key: &str,
) -> Result<serde_json::Value> {
    let endpoint = url.path().to_string();
    let mut response = client
        .get(url)
        .header("x-api-key", key.trim())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "CurseForge API request timed out"
            } else {
                "Could not connect to CurseForge API (network/TLS/proxy)"
            }
        })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let hint = match status {
            401 => "The saved CurseForge API key was rejected.",
            403 => "Check API key access to inZOI; this is separate from Nexus.",
            404 => "The mod/file is unavailable or was removed.",
            429 => "Rate limit reached; try again later.",
            _ => "Try again or check the service status.",
        };
        return Err(format!(
            "CurseForge API returned HTTP {status} | GET {endpoint} | {hint}"
        ));
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "CurseForge response interrupted")?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("CurseForge response exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "CurseForge returned invalid JSON")?;
    body.get("data")
        .cloned()
        .ok_or("CurseForge returned no data".into())
}
pub(crate) fn endpoint(path: &str) -> Result<url::Url> {
    url::Url::parse(&format!("https://api.curseforge.com/v1/{path}")).map_err(err)
}
pub(crate) fn validate_mod(data: &serde_json::Value, requested: &NexusLink) -> Result<u64> {
    let id = data["id"]
        .as_u64()
        .filter(|id| *id > 0)
        .ok_or("CurseForge project not found")?;
    if data["gameId"].as_u64() != Some(GAME)
        || (requested.mod_id > 0 && id != requested.mod_id)
        || requested
            .slug
            .as_ref()
            .is_some_and(|slug| data["slug"].as_str() != Some(slug))
    {
        return Err("CurseForge project does not match this inZOI link".into());
    }
    if data["allowModDistribution"] == false || data["isAvailable"] == false {
        return Err("This author does not allow this download through the CurseForge API. Open the original mod page to download it.".into());
    }
    Ok(id)
}
fn validate_file(data: &serde_json::Value, mod_id: u64, file_id: u64) -> Result<&str> {
    if data["id"].as_u64() != Some(file_id)
        || data["modId"].as_u64() != Some(mod_id)
        || data["isAvailable"] == false
    {
        return Err("This CurseForge file is unavailable or belongs to another mod".into());
    }
    data["fileName"]
        .as_str()
        .filter(|name| {
            safe_project(name) && name.len() <= 240 && !name.chars().any(char::is_control)
        })
        .ok_or("CurseForge returned an invalid filename".into())
}
pub(crate) fn trusted_mirror(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url
            .host_str()
            .is_some_and(|host| host == "forgecdn.net" || host.ends_with(".forgecdn.net"))
}

pub(crate) fn public_metadata(mut data: serde_json::Value) -> serde_json::Value {
    match &mut data {
        serde_json::Value::Object(fields) => {
            fields.remove("downloadUrl");
            for value in fields.values_mut() {
                *value = public_metadata(value.take());
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                *value = public_metadata(value.take());
            }
        }
        _ => {}
    }
    data
}

pub(crate) async fn download_file(
    request_id: &str,
    api_key: &str,
    request: &NexusLink,
    app: &tauri::AppHandle,
    root: &Path,
) -> Result<nexus::DownloadResult> {
    if api_key.trim().is_empty() {
        return Err("curseforgeApiKeyRequired".into());
    }
    let client = api_client()?;
    let project = if request.mod_id > 0 {
        api_data(
            &client,
            endpoint(&format!("mods/{}", request.mod_id))?,
            api_key,
        )
        .await?
    } else {
        let mut url = endpoint("mods/search")?;
        url.query_pairs_mut()
            .append_pair("gameId", &GAME.to_string())
            .append_pair(
                "slug",
                request.slug.as_deref().ok_or("Missing CurseForge slug")?,
            )
            .append_pair("pageSize", "50");
        let data = api_data(&client, url, api_key).await?;
        data.as_array()
            .and_then(|mods| {
                mods.iter()
                    .find(|data| data["slug"].as_str() == request.slug.as_deref())
            })
            .cloned()
            .ok_or("CurseForge mod was not found for inZOI")?
    };
    let mod_id = validate_mod(&project, request)?;
    let file_id = if request.file_id > 0 {
        request.file_id
    } else {
        project["mainFileId"]
            .as_u64()
            .filter(|id| *id > 0)
            .or_else(|| {
                project["latestFiles"].as_array().and_then(|files| {
                    files
                        .iter()
                        .filter(|file| file["isAvailable"] != false && file["releaseType"] == 1)
                        .filter_map(|file| file["id"].as_u64())
                        .max()
                })
            })
            .ok_or("No release file is available. Paste a specific CurseForge file link.")?
    };
    let file = api_data(
        &client,
        endpoint(&format!("mods/{mod_id}/files/{file_id}"))?,
        api_key,
    )
    .await?;
    let filename = validate_file(&file, mod_id, file_id)?;
    let extension = Path::new(filename)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["zip", "rar", "7z"].contains(&extension.as_str()) {
        return Err("This CurseForge file is not a ZIP, RAR or 7z archive".into());
    }
    let mut info = request.clone();
    info.mod_id = mod_id;
    info.file_id = file_id;
    if let Some(slug) = project["slug"].as_str().filter(|slug| valid_slug(slug)) {
        info.url = format!("https://www.curseforge.com/inzoi/mods/{slug}/files/{file_id}");
    }
    if let Some((path, bytes)) = download_storage::cached_file(root, &info)? {
        return Ok(nexus::DownloadResult {
            path: path.to_string_lossy().into_owned(),
            bytes,
            can_import: extension == "zip",
        });
    }
    let raw = api_data(
        &client,
        endpoint(&format!("mods/{mod_id}/files/{file_id}/download-url"))?,
        api_key,
    )
    .await?;
    let url = raw.as_str().and_then(|raw| url::Url::parse(raw).ok()).filter(trusted_mirror)
        .ok_or("CurseForge did not provide an allowed download URL. Open the original mod page to download it.")?;
    let title = project["name"].as_str().unwrap_or("CurseForge mod");
    let folder = download_storage::mod_folder(root, &info, title)?;
    let files = download_storage::file_folder(&folder, file_id)?;
    let pending = download_storage::PendingArchive::new(&files, filename)?;
    // This client has no API headers; credentials never travel to the CDN.
    let downloader = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(3600))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_mirror(attempt.url()) {
                attempt.error("Unsupported CurseForge download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not initialize CurseForge download client")?;
    let response = downloader
        .get(url)
        .send()
        .await
        .map_err(|_| "Could not start CurseForge download")?;
    if !response.status().is_success() {
        return Err(format!(
            "CurseForge download returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut record = metadata::download_record(&info, &public_metadata(file.clone()));
    record["mod"] = public_metadata(project);
    nexus::receive_archive(
        request_id,
        response,
        pending,
        record,
        app,
        extension == "zip",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    #[test]
    fn downloaded_curseforge_zip_imports_disabled_with_provenance_and_thumbnail() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let downloads = root.join("downloads");
        state.store.lock().unwrap().settings.downloads_dir =
            downloads.to_string_lossy().into_owned();
        let mut info =
            parse_link("curseforge://install?gameId=88849&addonId=123&fileId=987").unwrap();
        info.url = "https://www.curseforge.com/inzoi/mods/silk-hair/files/987".into();
        let parent = download_storage::mod_folder(&downloads, &info, "Silk Hair").unwrap();
        assert!(parent
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("[curseforge-123]"));
        let folder = download_storage::file_folder(&parent, info.file_id).unwrap();
        let buffer = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buffer);
        zip.start_file(
            "Hair/mod_manifest.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(br#"{"ProjectName":"SilkHair","FriendlyName":"Silk Hair"}"#)
            .unwrap();
        zip.start_file("Hair/hair.pak", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"original mod payload").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let pending = download_storage::PendingArchive::new(&folder, "hair.zip").unwrap();
        let mut file = pending.create().unwrap();
        file.write_all(&bytes).unwrap();
        drop(file);
        let mut record = metadata::download_record(
            &info,
            &serde_json::json!({"fileName":"hair.zip","displayName":"Silk Hair v1","fileLength":bytes.len()}),
        );
        record["mod"] = serde_json::json!({"id":123,"gameId":88849,"name":"Silk Hair","authors":[{"name":"Hair Creator"}],"logo":{"thumbnailUrl":"https://media.forgecdn.net/avatars/hair.jpg"}});
        let archive = pending
            .finish(
                record,
                bytes.len() as u64,
                format!("{:x}", Sha256::digest(&bytes)),
            )
            .unwrap();
        assert_eq!(
            download_storage::cached_file(&downloads, &info)
                .unwrap()
                .unwrap()
                .1,
            bytes.len() as u64
        );
        let items = library::catalogue(&state.store.lock().unwrap()).0;
        let card = items.iter().find(|m| m.archive).unwrap();
        assert!(card.source.contains("curseforge.com"));
        assert_eq!(card.author, "Hair Creator");
        assert!(card.metadata.contains_key("curseforge.json"));
        import_download_core(&archive.to_string_lossy(), &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        let m = saved.mods.iter().find(|m| m.project == "SilkHair").unwrap();
        assert!(m.managed && !m.enabled);
        assert_eq!(
            m.metadata["curseforge.json"]["Download"]["provider"],
            "curseforge"
        );
        assert_eq!(
            m.thumbnail.as_deref(),
            Some("https://media.forgecdn.net/avatars/hair.jpg")
        );
        assert_eq!(
            fs::read(Path::new(&m.path).join("hair.pak")).unwrap(),
            b"original mod payload"
        );
        assert!(!library::catalogue(&saved).0.iter().any(|m| m.archive));
        let nexus = nexus::parse_request("nxm://inzoi/mods/123/files/987")
            .unwrap()
            .info;
        let nexus_parent =
            download_storage::mod_folder(&downloads, &nexus, "Same numeric IDs").unwrap();
        assert_ne!(parent, nexus_parent);
        assert!(download_storage::cached_file(&downloads, &nexus)
            .unwrap()
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn temporary_download_urls_are_never_saved_in_metadata() {
        let data = public_metadata(
            serde_json::json!({"name":"Hair","downloadUrl":"SECRET_URL","latestFiles":[{"downloadUrl":"SECRET_URL","fileName":"hair.zip"}]}),
        );
        assert!(!data.to_string().contains("SECRET_URL"));
        assert_eq!(data["latestFiles"][0]["fileName"], "hair.zip");
    }

    #[test]
    fn api_transport_uses_curseforge_header_and_never_echoes_response_secrets() {
        nexus::configure_tls();
        use std::io::Read;
        for status in ["200 OK", "403 Forbidden"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 8192];
                let read = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]).to_lowercase();
                assert!(request.contains("x-api-key: fake_curseforge_key"));
                assert!(!request.contains("apikey:"));
                let body = r#"{"data":{"id":123},"secret":"RESPONSE_SECRET"}"#;
                write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap();
            let result = tauri::async_runtime::block_on(api_data(
                &client,
                url::Url::parse(&format!("http://{address}/v1/mods/123")).unwrap(),
                "FAKE_CURSEFORGE_KEY",
            ));
            if status.starts_with("200") {
                assert_eq!(result.unwrap()["id"], 123);
            } else {
                let error = result.unwrap_err();
                assert!(!error.contains("SECRET") && !error.contains("FAKE_CURSEFORGE_KEY"));
            }
            server.join().unwrap();
        }
    }
    #[test]
    fn links_are_game_scoped_and_strip_queries() {
        let info = parse_link(
            "https://www.curseforge.com/inzoi/mods/silk-hair/files/987/download?secret=hidden",
        )
        .unwrap();
        assert_eq!(info.file_id, 987);
        assert_eq!(info.slug.as_deref(), Some("silk-hair"));
        assert!(!info.url.contains("secret"));
        assert_eq!(
            parse_link("https://www.curseforge.com/inzoi/mods/silk-hair")
                .unwrap()
                .file_id,
            0
        );
        for bad in [
            "https://www.curseforge.com/minecraft/mc-mods/hair",
            "https://curseforge.com.evil.test/inzoi/mods/hair",
            "http://www.curseforge.com/inzoi/mods/hair",
            "https://www.curseforge.com/inzoi/mods/hair/files/0",
        ] {
            assert!(parse_link(bad).is_err());
        }
    }
    #[test]
    fn game_project_file_and_distribution_must_match() {
        let request =
            parse_link("https://www.curseforge.com/inzoi/mods/silk-hair/files/987").unwrap();
        let mut project = serde_json::json!({"id":123,"gameId":88849,"slug":"silk-hair","allowModDistribution":true});
        assert_eq!(validate_mod(&project, &request).unwrap(), 123);
        project["gameId"] = 432.into();
        assert!(validate_mod(&project, &request).is_err());
        project["gameId"] = GAME.into();
        project["allowModDistribution"] = false.into();
        assert!(validate_mod(&project, &request).is_err());
        assert!(validate_file(
            &serde_json::json!({"id":987,"modId":123,"fileName":"../outside.zip"}),
            123,
            987
        )
        .is_err());
        assert!(validate_file(
            &serde_json::json!({"id":987,"modId":456,"fileName":"mod.zip"}),
            123,
            987
        )
        .is_err());
    }
    #[test]
    fn downloads_only_use_forgecdn_https_without_credentials_or_custom_ports() {
        assert!(trusted_mirror(
            &url::Url::parse("https://edge.forgecdn.net/files/1/mod.zip").unwrap()
        ));
        for bad in [
            "http://edge.forgecdn.net/file.zip",
            "https://edge.forgecdn.net.evil.test/file.zip",
            "https://user:pass@edge.forgecdn.net/file.zip",
            "https://127.0.0.1/file.zip",
            "https://edge.forgecdn.net:444/file.zip",
        ] {
            assert!(!trusted_mirror(&url::Url::parse(bad).unwrap()));
        }
    }
}
