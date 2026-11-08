use super::*;
use std::time::Duration;
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASE_API: &str =
    "https://api.github.com/repos/coolbirdzik/inzoi-better-mod-manager/releases/latest";
const ASSET_PREFIX: &str = "/repos/coolbirdzik/inzoi-better-mod-manager/releases/assets/";

fn asset_url(asset: &serde_json::Value) -> Result<url::Url> {
    let url =
        url::Url::parse(asset["url"].as_str().ok_or("Missing release asset URL")?).map_err(err)?;
    if url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .path()
            .strip_prefix(ASSET_PREFIX)
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("Invalid GitHub release asset URL".into());
    }
    Ok(url)
}

fn release_assets(release: &serde_json::Value) -> Result<&Vec<serde_json::Value>> {
    if release["draft"] != false || release["prerelease"] != false {
        return Err("Only published stable app releases can be used for updates".into());
    }
    release["assets"]
        .as_array()
        .ok_or("Missing release assets".into())
}

async fn latest_release() -> Result<serde_json::Value> {
    // A separate public client: no Nexus/CurseForge keys or signed NXM URLs.
    let client = reqwest::Client::builder()
        .user_agent("inZOI-Better-Mod-Manager")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(err)?;
    let response = client
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(err)?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub release API returned HTTP {status}"));
    }
    response.json().await.map_err(err)
}

pub(crate) async fn check_fallback(
    app: &tauri::AppHandle,
    target: Option<&str>,
) -> Result<Option<Update>> {
    let release = latest_release().await?;
    let assets = release_assets(&release)?;
    let manifest = assets
        .iter()
        .find(|a| a["name"] == "latest.json")
        .ok_or("Published release has no latest.json")?;
    let mut builder = app
        .updater_builder()
        .endpoints(vec![asset_url(manifest)?])
        .map_err(err)?
        .header("Accept", "application/octet-stream")
        .map_err(err)?
        .timeout(Duration::from_secs(20));
    if let Some(target) = target {
        builder = builder.target(target);
    }
    let mut update = builder.build().map_err(err)?.check().await.map_err(err)?;
    if let Some(update) = &mut update {
        let version = release["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v');
        if update.version != version {
            return Err("Release manifest version does not match the published release".into());
        }
        // The same signed bytes are available via the API if github.com's
        // release download gateway is unavailable. Signature checks remain
        // entirely in the official updater for both installers and portable.
        let package = assets
            .iter()
            .find(|a| a["browser_download_url"].as_str() == Some(update.download_url.as_str()))
            .ok_or("Update package is not an asset of the published release")?;
        update.download_url = asset_url(package)?;
    }
    Ok(update)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateMetadata {
    rid: tauri::ResourceId,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw_json: serde_json::Value,
}

#[tauri::command]
pub(crate) async fn check_app_update_fallback(
    app: tauri::AppHandle,
    webview: tauri::Webview,
) -> Result<Option<UpdateMetadata>> {
    let Some(update) = check_fallback(&app, None).await? else {
        return Ok(None);
    };
    // Register the official Update resource so the existing plugin performs
    // download, signature verification and installation without a second path.
    let metadata = UpdateMetadata {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date: update.raw_json["pub_date"].as_str().map(str::to_string),
        body: update.body.clone(),
        raw_json: update.raw_json.clone(),
        rid: webview.resources_table().add(update),
    };
    Ok(Some(metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_only_accepts_assets_from_this_repository() {
        let asset = |url: &str| serde_json::json!({"url":url});
        assert!(asset_url(&asset(&format!("https://api.github.com{ASSET_PREFIX}123"))).is_ok());
        for url in ["http://api.github.com/repos/coolbirdzik/inzoi-better-mod-manager/releases/assets/123",
            "https://api.github.com/repos/other/repo/releases/assets/123",
            "https://evil.example/repos/coolbirdzik/inzoi-better-mod-manager/releases/assets/123",
            "https://user@api.github.com/repos/coolbirdzik/inzoi-better-mod-manager/releases/assets/123",
            "https://api.github.com/repos/coolbirdzik/inzoi-better-mod-manager/releases/assets/123?token=secret"] {
            assert!(asset_url(&asset(url)).is_err(), "{url}");
        }
    }
    #[test]
    fn draft_prerelease_and_missing_assets_are_not_used() {
        for release in [
            serde_json::json!({"draft":true,"prerelease":false,"assets":[]}),
            serde_json::json!({"draft":false,"prerelease":true,"assets":[]}),
            serde_json::json!({}),
        ] {
            assert!(release_assets(&release).is_err());
        }
        assert!(
            release_assets(&serde_json::json!({"draft":false,"prerelease":false,"assets":[]}))
                .is_ok()
        );
    }
    #[tokio::test]
    #[ignore = "Read-only GitHub API smoke check; requires internet"]
    async fn live_fallback_manifest_is_valid_without_downloading_or_installing_an_update() {
        nexus::configure_tls();
        let release = latest_release().await.unwrap();
        let assets = release_assets(&release).unwrap();
        let manifest = assets.iter().find(|a| a["name"] == "latest.json").unwrap();
        let client = reqwest::Client::builder()
            .user_agent("inZOI-Better-Mod-Manager")
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap();
        let response = client
            .get(asset_url(manifest).unwrap())
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let json: serde_json::Value = response.json().await.unwrap();
        let _: tauri_plugin_updater::RemoteRelease = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            format!("v{}", json["version"].as_str().unwrap()),
            release["tag_name"]
        );
        println!("GitHub API fallback manifest verified: {}", json["version"]);
    }
}
