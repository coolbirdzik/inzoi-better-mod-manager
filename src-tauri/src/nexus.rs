use super::*;
use std::{collections::HashMap, io::Write, time::Duration};
#[derive(Clone)]
pub(crate) struct Request {
    pub info: NexusLink,
    key: Option<String>,
    expires: Option<String>,
}
pub(crate) fn parse_request(raw: &str) -> Result<Request> {
    let info = parse_link(raw)?;
    let u = url::Url::parse(raw).map_err(|_| "Invalid NXM URL")?;
    let query: HashMap<_, _> = u
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let key = query.get("key").cloned();
    let expires = query.get("expires").cloned();
    if key.is_some() != expires.is_some() {
        return Err("NXM download links must include both key and expires".into());
    }
    if let Some(value) = &expires {
        value.parse::<u64>().map_err(|_| "Invalid NXM expiry")?;
    }
    Ok(Request { info, key, expires })
}
fn api_url(request: &Request, download: bool) -> Result<url::Url> {
    let suffix = if download {
        "/download_link.json"
    } else {
        ".json"
    };
    let mut u = url::Url::parse(&format!(
        "https://api.nexusmods.com/v1/games/inzoi/mods/{}/files/{}{}",
        request.info.mod_id, request.info.file_id, suffix
    ))
    .map_err(err)?;
    if download {
        if let (Some(key), Some(expires)) = (&request.key, &request.expires) {
            u.query_pairs_mut()
                .append_pair("key", key)
                .append_pair("expires", expires);
        }
    }
    Ok(u)
}
fn trusted_mirror(u: &url::Url) -> bool {
    u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.host_str()
            .map(|h| {
                ["nexusmods.com", "nexus-cdn.com", "nexusmods.net"]
                    .iter()
                    .any(|d| h == *d || h.ends_with(&format!(".{d}")))
            })
            .unwrap_or(false)
}
async fn api_json(client: &reqwest::Client, url: url::Url, key: &str) -> Result<serde_json::Value> {
    let mut response = client
        .get(url)
        .header("apikey", key)
        .header("Application-Name", "inZOI Better Mod Manager")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|_| "Could not connect to Nexus API")?;
    if !response.status().is_success() {
        return Err(format!(
            "Nexus API returned HTTP {}. Check your API key and whether the NXM link has expired.",
            response.status().as_u16()
        ));
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Nexus API response was interrupted")?
    {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err("Nexus API response exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Nexus returned invalid JSON".into())
}
#[derive(Clone, Serialize)]
struct Progress {
    request_id: String,
    received: u64,
    total: Option<u64>,
}
#[derive(Serialize)]
pub(crate) struct DownloadResult {
    path: String,
    bytes: u64,
    can_import: bool,
}
#[tauri::command]
pub(crate) async fn download_nxm(
    request_id: String,
    api_key: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<DownloadResult> {
    if api_key.trim().is_empty() {
        return Err("Enter your Nexus API key in this session first".into());
    }
    let request = state
        .requests
        .lock()
        .map_err(err)?
        .get(&request_id)
        .cloned()
        .ok_or("Link expired from this app session; paste it again")?;
    if let Some(expiry) = &request.expires {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_secs();
        if expiry.parse::<u64>().map_err(err)? <= now {
            return Err(
                "This NXM link has expired. Click Download with Manager on Nexus again.".into(),
            );
        }
    }
    let downloads = state
        .file
        .parent()
        .ok_or("Missing application data directory")?
        .join("downloads")
        .join(uuid::Uuid::new_v4().to_string());
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!(
            "inZOI-Better-Mod-Manager/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|_| "Could not initialize HTTPS client")?;
    let metadata = api_json(&client, api_url(&request, false)?, api_key.trim()).await?;
    let filename = metadata
        .get("file_name")
        .and_then(|v| v.as_str())
        .filter(|s| safe_project(s))
        .ok_or("Nexus returned an unsafe archive name")?;
    let extension = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["zip", "7z", "rar"].contains(&extension.as_str()) {
        return Err("This Nexus file is not a ZIP, 7z or RAR mod archive".into());
    }
    let mirrors = api_json(&client, api_url(&request, true)?, api_key.trim()).await?;
    let url = mirrors
        .as_array()
        .and_then(|a| {
            a.iter()
                .filter_map(|v| v.get("URI").and_then(|x| x.as_str()))
                .filter_map(|s| url::Url::parse(s).ok())
                .find(trusted_mirror)
        })
        .ok_or("Nexus returned no supported HTTPS download mirror")?;
    // Deliberately use a separate client: the API key must never reach the CDN.
    let downloader = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(3600))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_mirror(attempt.url()) {
                attempt.error("Unsupported download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not initialize download client")?;
    let mut response = downloader
        .get(url)
        .send()
        .await
        .map_err(|_| "Could not start the archive download")?;
    if !response.status().is_success() {
        return Err(format!(
            "Download server returned HTTP {}",
            response.status().as_u16()
        ));
    }
    const MAX: u64 = 8 * 1024 * 1024 * 1024;
    let total = response.content_length();
    if total.is_some_and(|n| n > MAX) {
        return Err("Archive exceeds the 8 GiB download limit".into());
    }
    fs::create_dir_all(&downloads).map_err(err)?;
    let partial = downloads.join("download.part");
    let target = downloads.join(filename);
    let result: Result<u64> = async {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(err)?;
        let mut received = 0u64;
        let mut last = std::time::Instant::now();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The archive download was interrupted; retry the link")?
        {
            received += chunk.len() as u64;
            if received > MAX {
                return Err("Download exceeded the 8 GiB size limit".into());
            }
            file.write_all(&chunk).map_err(err)?;
            if last.elapsed() > Duration::from_millis(150) {
                let _ = app.emit(
                    "nexus-download-progress",
                    Progress {
                        request_id: request_id.clone(),
                        received,
                        total,
                    },
                );
                last = std::time::Instant::now();
            }
        }
        if received == 0 || total.is_some_and(|n| n != received) {
            return Err("The downloaded archive is incomplete".into());
        }
        file.sync_all().map_err(err)?;
        drop(file);
        fs::rename(&partial, &target).map_err(err)?;
        let _ = app.emit(
            "nexus-download-progress",
            Progress {
                request_id: request_id.clone(),
                received,
                total,
            },
        );
        Ok(received)
    }
    .await;
    match result {
        Ok(bytes) => {
            state.requests.lock().map_err(err)?.remove(&request_id);
            Ok(DownloadResult {
                path: target.to_string_lossy().into_owned(),
                bytes,
                can_import: extension == "zip",
            })
        }
        Err(e) => {
            let _ = fs::remove_file(&partial);
            let _ = fs::remove_dir(&downloads);
            Err(e)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_query_only_goes_to_download_api() {
        let r = parse_request(
            "nxm://inzoi/mods/123/files/456?key=secret&expires=9999999999&user_id=12",
        )
        .unwrap();
        let url = api_url(&r, true).unwrap();
        assert_eq!(url.host_str(), Some("api.nexusmods.com"));
        assert!(url.query().unwrap().contains("key=secret"));
        assert!(api_url(&r, false).unwrap().query().is_none());
        assert!(!serde_json::to_string(&r.info).unwrap().contains("secret"));
    }
    #[test]
    fn rejects_other_games_and_incomplete_auth() {
        assert!(parse_request("nxm://skyrim/mods/1/files/2").is_err());
        assert!(parse_request("nxm://inzoi/mods/1/files/2?key=secret").is_err());
    }
    #[test]
    fn mirrors_cannot_target_local_or_untrusted_hosts() {
        for raw in [
            "http://files.nexus-cdn.com/file.zip",
            "https://localhost/file.zip",
            "https://nexusmods.com.attacker.test/file.zip",
            "https://user:pass@files.nexus-cdn.com/file.zip",
            "https://127.0.0.1/file.zip",
        ] {
            assert!(!trusted_mirror(&url::Url::parse(raw).unwrap()));
        }
        assert!(trusted_mirror(
            &url::Url::parse("https://files.nexus-cdn.com/file.zip").unwrap()
        ));
    }
}
