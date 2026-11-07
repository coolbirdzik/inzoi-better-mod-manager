use super::*;
use std::sync::Mutex;

const LIMIT: usize = 500;
static LOG_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    id: String,
    run_id: String,
    timestamp: u64,
    mod_id: String,
    mod_name: String,
    project: String,
    provider: String,
    status: String,
    code: String,
    duration_ms: u64,
    detail: String,
}
pub(crate) fn code(error: &str) -> &'static str {
    let text = error.to_lowercase();
    if text.contains("apikeyrequired") {
        "missing_key"
    } else if text.contains("http 401") {
        "http_401"
    } else if text.contains("http 403") {
        "http_403"
    } else if text.contains("http 404") {
        "http_404"
    } else if text.contains("http 429") {
        "http_429"
    } else if text.contains("timed out") || text.contains("timeout") {
        "timeout"
    } else if text.contains("connect") || text.contains("tls") || text.contains("certificate") {
        "network"
    } else if text.contains("not found") || text.contains("no longer exists") {
        "missing_mod"
    } else {
        "other"
    }
}
fn limited(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}
fn redact(text: &str, key: &str) -> String {
    let text = if key.trim().is_empty() {
        text.to_string()
    } else {
        text.replace(key.trim(), "[REDACTED]")
    };
    let mut rest = text.as_str();
    let mut clean = String::new();
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = ["https://", "http://", "nxm://"]
            .iter()
            .filter_map(|scheme| lower.find(scheme))
            .min()
        else {
            clean.push_str(rest);
            break;
        };
        clean.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
            .unwrap_or(rest.len());
        if let Ok(mut url) = url::Url::parse(&rest[..end]) {
            url.set_query(None);
            url.set_fragment(None);
            let _ = url.set_username("");
            let _ = url.set_password(None);
            clean.push_str(url.as_str());
        } else {
            clean.push_str("[URL]");
        }
        rest = &rest[end..];
    }
    clean
}
impl Entry {
    pub(crate) fn new(
        log_id: Option<String>,
        run_id: Option<String>,
        id: &str,
        m: Option<&Mod>,
        provider: &str,
        status: &str,
        detail: &str,
        duration_ms: u64,
        key: &str,
    ) -> Self {
        let uuid = |raw: Option<String>| {
            raw.and_then(|raw| uuid::Uuid::parse_str(&raw).ok())
                .unwrap_or_else(uuid::Uuid::new_v4)
                .to_string()
        };
        Self {
            id: uuid(log_id),
            run_id: uuid(run_id),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            mod_id: limited(&redact(id, key), 1024),
            mod_name: limited(&redact(m.map(|m| m.name.as_str()).unwrap_or(id), key), 256),
            project: limited(
                &redact(m.map(|m| m.project.as_str()).unwrap_or(""), key),
                256,
            ),
            provider: limited(provider, 30),
            status: limited(status, 30),
            code: if status == "failed" {
                code(detail)
            } else {
                status
            }
            .to_string(),
            duration_ms,
            detail: limited(&redact(detail, key), 2000),
        }
    }
}
fn path(store_file: &Path) -> Result<PathBuf> {
    Ok(store_file
        .parent()
        .ok_or("Missing app data folder")?
        .join("logs/mod-update-checks.jsonl"))
}
fn read(path: &Path) -> Result<Vec<Entry>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = metadata::read_limited(path, 16 * 1024 * 1024)?;
    let text = String::from_utf8(bytes).map_err(err)?;
    let mut entries: Vec<_> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .collect();
    if entries.len() > LIMIT {
        entries.drain(..entries.len() - LIMIT);
    }
    Ok(entries)
}
pub(crate) fn append(store_file: &Path, entry: Entry) -> Result<()> {
    let _guard = LOG_LOCK.lock().map_err(err)?;
    let path = path(store_file)?;
    let mut entries = read(&path).unwrap_or_default();
    entries.retain(|old| old.id != entry.id);
    entries.push(entry);
    if entries.len() > LIMIT {
        entries.drain(..entries.len() - LIMIT);
    }
    fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
    let mut text = String::new();
    for entry in entries {
        text.push_str(&serde_json::to_string(&entry).map_err(err)?);
        text.push('\n');
    }
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, text).map_err(err)?;
    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::remove_file(temp);
        return Err(err(error));
    }
    Ok(())
}
#[derive(Serialize)]
pub(crate) struct Snapshot {
    path: String,
    entries: Vec<Entry>,
}
#[tauri::command(async)]
pub(crate) fn read_mod_check_log(state: tauri::State<State>) -> Result<Snapshot> {
    let _guard = LOG_LOCK.lock().map_err(err)?;
    let path = path(&state.file)?;
    Ok(Snapshot {
        entries: read(&path)?,
        path: path.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_secrets_and_signed_urls_before_persisting_and_keeps_history() {
        let (root, state, source) = crate::lifecycle_tests::fixture();
        let m = read_mod(&source, "mod".into(), false).unwrap();
        let entry=Entry::new(None,None,"mod",Some(&m),"nexus","failed","HTTP 401 GET https://api.nexusmods.com/v1/files?key=SIGNED_SECRET&expires=123 apikey FAKE_API_SECRET url=\"HTTPS://user:password@api.nexusmods.com/v1/files?key=QUOTED_SECRET#TOKEN\"",12,"FAKE_API_SECRET");
        append(&state.file, entry.clone()).unwrap();
        let text = fs::read_to_string(path(&state.file).unwrap()).unwrap();
        assert!(!text.contains("FAKE_API_SECRET") && !text.contains("SIGNED_SECRET"));
        assert!(
            !text.contains("QUOTED_SECRET")
                && !text.contains("password")
                && !text.contains("#TOKEN")
        );
        assert!(text.contains("http_401") && text.contains("HTTP 401"));
        append(&state.file, entry).unwrap();
        assert_eq!(read(&path(&state.file).unwrap()).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_keys_are_distinct_from_http_or_network_failures() {
        assert_eq!(code("curseforgeApiKeyRequired"), "missing_key");
        assert_eq!(code("Nexus API returned HTTP 429"), "http_429");
        assert_eq!(
            code("Could not connect to Nexus API (TLS/proxy/network)"),
            "network"
        );
        assert_eq!(code("Request timed out"), "timeout");
    }
}
