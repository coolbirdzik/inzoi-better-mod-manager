pub(crate) fn configure_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone)]
pub(crate) struct Request {
    pub info: NexusLink,
    key: Option<String>,
    expires: Option<String>,
    active: Arc<AtomicBool>,
    cancel: tokio::sync::watch::Sender<bool>,
}
pub(crate) fn parse_request(raw: &str) -> Result<Request> {
    if !raw.starts_with("nxm:") {
        return Ok(Request {
            info: curseforge::parse_link(raw)?,
            key: None,
            expires: None,
            active: Arc::new(AtomicBool::new(false)),
            cancel: tokio::sync::watch::channel(false).0,
        });
    }
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
    Ok(Request {
        info,
        key,
        expires,
        active: Arc::new(AtomicBool::new(false)),
        cancel: tokio::sync::watch::channel(false).0,
    })
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
fn mod_api_url(info: &NexusLink) -> Result<url::Url> {
    url::Url::parse(&format!(
        "https://api.nexusmods.com/v1/games/inzoi/mods/{}.json",
        info.mod_id
    ))
    .map_err(err)
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
pub(crate) fn api_client() -> Result<reqwest::Client> {
    configure_tls();
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!(
            "inZOI-Better-Mod-Manager/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|_| "Could not initialize HTTPS client".into())
}
pub(crate) async fn api_json(
    client: &reqwest::Client,
    url: url::Url,
    key: &str,
) -> Result<serde_json::Value> {
    if key.trim().is_empty() {
        return Err("nexusApiKeyRequired".into());
    }
    let endpoint = url.path().to_string();
    let mut response = client
        .get(url)
        .header("apikey", key.trim())
        .header("Application-Name", "inZOI Better Mod Manager")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "Nexus API request timed out"
            } else {
                "Could not connect to Nexus API (network/TLS/proxy)"
            }
        })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let hint = match status {
            401 => "The saved API key was rejected.",
            403 => "API access to this mod or account was denied.",
            404 => "The mod/file is unavailable or was removed.",
            429 => "Rate limit reached; try again later.",
            _ => "Try again or check the service status.",
        };
        return Err(format!(
            "Nexus API returned HTTP {status} | GET {endpoint} | {hint}"
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
    pub(crate) path: String,
    pub(crate) bytes: u64,
    pub(crate) can_import: bool,
}
pub(crate) fn downloads_root(settings: &Settings) -> PathBuf {
    if settings.downloads_dir.is_empty() {
        Path::new(&settings.library_dir).join("Downloads")
    } else {
        PathBuf::from(&settings.downloads_dir)
    }
}

struct ActiveDownload(Arc<AtomicBool>);
impl Drop for ActiveDownload {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn with_cancellation<T>(
    cancel: &mut tokio::sync::watch::Receiver<bool>,
    task: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    if *cancel.borrow() {
        return Err("downloadCancelled".into());
    }
    tokio::select! {
        biased;
        _ = cancel.changed() => Err("downloadCancelled".into()),
        result = task => result,
    }
}

#[tauri::command]
pub(crate) fn cancel_nxm(request_id: String, state: tauri::State<State>) -> Result<()> {
    if let Some(request) = state.requests.lock().map_err(err)?.get(&request_id) {
        request.cancel.send_replace(true);
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn download_nxm(
    request_id: String,
    api_key: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<DownloadResult> {
    configure_tls();
    let (request, mut cancel) = {
        let requests = state.requests.lock().map_err(err)?;
        let request = requests
            .get(&request_id)
            .cloned()
            .ok_or("Link expired from this app session; paste it again")?;
        if requests.values().any(|other| {
            other.info.provider == request.info.provider
                && other.info.mod_id == request.info.mod_id
                && other.info.slug == request.info.slug
                && other.active.load(Ordering::SeqCst)
        }) {
            return Err("Another file of this mod is already downloading".into());
        }
        if request.active.swap(true, Ordering::SeqCst) {
            return Err("This download is already running".into());
        }
        request.cancel.send_replace(false);
        let receiver = request.cancel.subscribe();
        (request, receiver)
    };
    let _active = ActiveDownload(request.active.clone());
    let root = downloads_root(&state.store.lock().map_err(err)?.settings);
    let result = with_cancellation(&mut cancel, async {
        if request.info.provider == "curseforge" {
            curseforge::download_file(&request_id, &api_key, &request.info, &app, &root).await
        } else {
            download_file(&request_id, &api_key, &request, &app, &root).await
        }
    })
    .await;
    if result.is_ok() {
        state.requests.lock().map_err(err)?.remove(&request_id);
    }
    result
}

async fn download_file(
    request_id: &str,
    api_key: &str,
    request: &Request,
    app: &tauri::AppHandle,
    root: &Path,
) -> Result<DownloadResult> {
    if let Some((path, bytes)) = download_storage::cached_file(root, &request.info)? {
        let can_import = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        return Ok(DownloadResult {
            path: path.to_string_lossy().into_owned(),
            bytes,
            can_import,
        });
    }
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
    let client = api_client()?;
    let metadata = api_json(&client, api_url(&request, false)?, &api_key).await?;
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
    // Every file/version needs the mod's image metadata, even when its parent
    // mod folder already exists. Missing optional details must not block a file.
    let mod_metadata = api_json(&client, mod_api_url(&request.info)?, api_key)
        .await
        .unwrap_or(serde_json::Value::Null);
    let title = mod_metadata
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| metadata.get("name").and_then(|v| v.as_str()))
        .unwrap_or_else(|| {
            Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Mod")
        });
    let mod_folder = download_storage::mod_folder(root, &request.info, title)?;
    let downloads = download_storage::file_folder(&mod_folder, request.info.file_id)?;
    let pending = download_storage::PendingArchive::new(&downloads, filename)?;
    let mirrors = api_json(&client, api_url(&request, true)?, &api_key).await?;
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
    let response = downloader
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
    let mut record = super::metadata::download_record(&request.info, &metadata);
    if !mod_metadata.is_null() {
        record["mod"] = mod_metadata;
    }
    receive_archive(
        request_id,
        response,
        pending,
        record,
        app,
        extension == "zip",
    )
    .await
}

pub(crate) async fn receive_archive(
    request_id: &str,
    mut response: reqwest::Response,
    pending: download_storage::PendingArchive,
    record: serde_json::Value,
    app: &tauri::AppHandle,
    can_import: bool,
) -> Result<DownloadResult> {
    const MAX: u64 = 8 * 1024 * 1024 * 1024;
    let total = response.content_length();
    if total.is_some_and(|n| n > MAX) {
        return Err("Archive exceeds the 8 GiB download limit".into());
    }
    let mut file = pending.create()?;
    let mut received = 0u64;
    let mut hasher = Sha256::new();
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
        hasher.update(&chunk);
        if last.elapsed() > Duration::from_millis(150) {
            let _ = app.emit(
                "nexus-download-progress",
                Progress {
                    request_id: request_id.to_string(),
                    received,
                    total,
                },
            );
            last = std::time::Instant::now();
        }
    }
    if received == 0
        || total.is_some_and(|n| n != received)
        || record["file"]["fileLength"]
            .as_u64()
            .is_some_and(|n| n != received)
    {
        return Err("The downloaded archive is incomplete".into());
    }
    file.sync_all().map_err(err)?;
    drop(file);
    let target = pending.finish(record, received, format!("{:x}", hasher.finalize()))?;
    let _ = app.emit(
        "nexus-download-progress",
        Progress {
            request_id: request_id.to_string(),
            received,
            total,
        },
    );
    Ok(DownloadResult {
        path: target.to_string_lossy().into_owned(),
        bytes: received,
        can_import,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_destination_defaults_to_managed_folder_and_custom_path_survives_reload() {
        let (root, state, _) = crate::lifecycle_tests::fixture();
        let mut settings = state.store.lock().unwrap().settings.clone();
        assert_eq!(
            downloads_root(&settings),
            Path::new(&settings.library_dir).join("Downloads")
        );
        settings.downloads_dir = root.join("custom-downloads").to_string_lossy().into_owned();
        save_settings_core(settings.clone(), &state).unwrap();
        let saved: Store = serde_json::from_slice(&fs::read(&state.file).unwrap()).unwrap();
        assert_eq!(
            downloads_root(&saved.settings),
            PathBuf::from(&settings.downloads_dir)
        );
        settings.downloads_dir = "relative/path".into();
        assert!(save_settings_core(settings, &state).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_interrupts_a_pending_transfer_and_releases_its_file_for_cleanup() {
        let root = std::env::temp_dir().join(format!("inzoimm-cancel-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let partial = root.join("download.part");
        let request = parse_request("nxm://inzoi/mods/1/files/2").unwrap();
        let mut receiver = request.cancel.subscribe();
        let active = request.active.clone();
        active.store(true, Ordering::SeqCst);
        let guard = ActiveDownload(active.clone());
        tauri::async_runtime::block_on(async {
            let transfer = async {
                let mut file = fs::File::create(&partial).unwrap();
                file.write_all(b"partial archive").unwrap();
                request.cancel.send_replace(true);
                std::future::pending::<Result<()>>().await
            };
            assert_eq!(
                with_cancellation(&mut receiver, transfer)
                    .await
                    .unwrap_err(),
                "downloadCancelled"
            );
        });
        drop(guard);
        assert!(!active.load(Ordering::SeqCst));
        fs::remove_dir_all(&root).unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn importing_a_downloaded_zip_keeps_archive_and_adds_disabled_managed_mod_with_metadata() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let download_root = root.join("custom-downloads");
        let downloads = download_root.join(format!("1-2-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&downloads).unwrap();
        let archive = downloads.join("hair.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file(
            "Hair/mod_manifest.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&fs::read(source.join("mod_manifest.json")).unwrap())
            .unwrap();
        zip.start_file("Hair/payload.pak", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"mod payload").unwrap();
        zip.finish().unwrap();
        let info = parse_request("nxm://inzoi/mods/1/files/2").unwrap().info;
        let image = "https://staticdelivery.nexusmods.com/mods/6919/images/1/preview.jpg";
        let mut record =
            super::super::metadata::download_record(&info, &serde_json::json!({"name":"Hair"}));
        record["mod"] = serde_json::json!({"name":"Hair","picture_url":image});
        fs::write(downloads.join(".inzoimm-nexus.json"), record.to_string()).unwrap();
        // A new default for future downloads must not move the file currently importing.
        let mut settings = state.store.lock().unwrap().settings.clone();
        settings.downloads_dir = root.join("future-downloads").to_string_lossy().into_owned();
        save_settings_core(settings, &state).unwrap();
        let existing = downloads.join("Hair_ABC");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("keep.txt"), b"existing user files").unwrap();
        assert!(import_download_core(&archive.to_string_lossy(), &state).is_err());
        assert_eq!(
            fs::read(existing.join("keep.txt")).unwrap(),
            b"existing user files"
        );
        assert!(state.store.lock().unwrap().mods.is_empty());
        fs::remove_dir_all(&existing).unwrap();
        let failed = State {
            file: root.join("blocked/store.json"),
            store: Mutex::new(state.store.lock().unwrap().clone()),
            pending: Mutex::new(vec![]),
            requests: Mutex::new(Default::default()),
            mod_updates: Mutex::new(Default::default()),
        };
        fs::create_dir_all(&failed.file).unwrap();
        assert!(import_download_core(&archive.to_string_lossy(), &failed).is_err());
        assert!(!existing.exists());
        assert!(archive.is_file());
        assert!(downloads.join(".inzoimm-nexus.json").is_file());
        assert!(failed.store.lock().unwrap().mods.is_empty());
        import_download_core(&archive.to_string_lossy(), &state).unwrap();
        let store = state.store.lock().unwrap();
        let m = &store.mods[0];
        assert!(m.managed && !m.enabled);
        assert_eq!(m.thumbnail.as_deref(), Some(image));
        let download_root = download_root.canonicalize().unwrap();
        assert_eq!(
            Path::new(&m.path).parent().unwrap(),
            downloads.canonicalize().unwrap()
        );
        assert_eq!(
            Path::new(m.library_root.as_ref().unwrap()),
            download_root.as_path()
        );
        assert!(!Path::new(&store.settings.library_dir)
            .join("Nexus")
            .exists());
        assert_eq!(
            fs::read(Path::new(&m.path).join("payload.pak")).unwrap(),
            b"mod payload"
        );
        assert_eq!(m.metadata["nexus.json"]["Download"]["file_id"], 2);
        assert!(archive.is_file());
        let id = m.id.clone();
        drop(store);
        remove_mod_core(id, &state).unwrap();
        assert!(archive.is_file());
        assert!(state.store.lock().unwrap().mods.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
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

#[cfg(test)]
mod transport_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn serve(response: Vec<u8>) -> (url::Url, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut chunk = [0u8; 1024];
            loop {
                let n = socket.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|x| x == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&request).into_owned());
            socket.write_all(&response).unwrap();
        });
        (
            url::Url::parse(&format!("http://{address}/test")).unwrap(),
            rx,
        )
    }
    #[test]
    fn sends_api_auth_and_reads_json_over_actual_transport() {
        configure_tls();
        let body = b"{\"file_name\":\"Hair.zip\"}";
        let mut response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
        response.extend(body);
        let (url, rx) = serve(response);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let data =
            tauri::async_runtime::block_on(api_json(&client, url, "  TEST_KEY_ONLY  ")).unwrap();
        assert_eq!(data["file_name"], "Hair.zip");
        let headers = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .to_lowercase();
        assert!(headers.contains("apikey: test_key_only"));
        assert!(!headers.contains("authorization:"));
        assert!(headers.contains("application-name: inzoi better mod manager"));
    }
    #[test]
    fn blank_api_key_is_rejected_before_any_network_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url =
            url::Url::parse(&format!("http://{}/test", listener.local_addr().unwrap())).unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let error = tauri::async_runtime::block_on(api_json(&client, url, "   ")).unwrap_err();
        assert_eq!(error, "nexusApiKeyRequired");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn api_failure_does_not_echo_response_secrets() {
        configure_tls();
        let body = b"TEST_KEY_ONLY echoed by server";
        let mut response = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend(body);
        let (url, _) = serve(response);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let error =
            tauri::async_runtime::block_on(api_json(&client, url, "TEST_KEY_ONLY")).unwrap_err();
        assert!(error.contains("403"));
        assert!(!error.contains("TEST_KEY_ONLY"));
    }
}

#[cfg(test)]
mod live_https_smoke {
    use super::*;
    #[test]
    #[ignore = "Read-only HTTPS smoke check against Nexus; requires internet"]
    fn nexus_https_initializes_and_rejects_invalid_key_safely() {
        configure_tls();
        let client = reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let endpoint =
            url::Url::parse("https://api.nexusmods.com/v1/games/inzoi/mods/1/files/1.json")
                .unwrap();
        let result = tauri::async_runtime::block_on(api_json(
            &client,
            endpoint,
            "INZOIMM_INVALID_SMOKE_KEY",
        ));
        let error = result.expect_err("A deliberately invalid API key should not authenticate");
        assert!(
            error.contains("HTTP 401") || error.contains("HTTP 403") || error.contains("HTTP 400"),
            "Unexpected HTTPS result: {error}"
        );
        assert!(!error.contains("INZOIMM_INVALID_SMOKE_KEY"));
        println!(
            "Nexus HTTPS transport reached the API; invalid-key response was safely reported."
        );
    }
}
