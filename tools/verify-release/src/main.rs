use base64::Engine;
use std::{env, fs, path::Path};
fn decode(s: &str) -> Result<String, String> {
    String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn asset_name(url: &str, prefix: &str) -> Result<String, String> {
    let encoded = url
        .strip_prefix(prefix)
        .ok_or("Asset URL is outside the expected GitHub release")?;
    let name = percent_encoding::percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|e| e.to_string())?
        .into_owned();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|c| "/\\:\0?#".contains(c))
    {
        return Err("Invalid asset filename".into());
    }
    Ok(name)
}
fn verify(config: &Path, manifest: &Path, assets: &Path, tag: &str) -> Result<usize, String> {
    let cfg: serde_json::Value =
        serde_json::from_slice(&fs::read(config).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let version = cfg["version"].as_str().ok_or("Config has no version")?;
    if tag != format!("v{version}") {
        return Err("Release tag does not match the signed build configuration".into());
    }
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if data["version"].as_str().map(|s| s.trim_start_matches('v')) != Some(version) {
        return Err("Updater manifest version mismatch".into());
    }
    let key = minisign_verify::PublicKey::decode(&decode(
        cfg["plugins"]["updater"]["pubkey"]
            .as_str()
            .ok_or("No updater public key")?,
    )?)
    .map_err(|e| e.to_string())?;
    let platforms = data["platforms"]
        .as_object()
        .ok_or("No updater platforms")?;
    for required in ["windows-x86_64", "darwin-aarch64", "darwin-x86_64"] {
        if !platforms.keys().any(|p| {
            p == required || p == &format!("{required}-nsis") || p == &format!("{required}-app")
        }) {
            return Err(format!("Missing updater platform: {required}"));
        }
    }
    let prefix =
        format!("https://github.com/coolbirdzik/inzoi-better-mod-manager/releases/download/{tag}/");
    for (platform, entry) in platforms {
        let name = asset_name(entry["url"].as_str().ok_or("Missing asset URL")?, &prefix)?;
        let signature = minisign_verify::Signature::decode(&decode(
            entry["signature"]
                .as_str()
                .ok_or("Missing artifact signature")?,
        )?)
        .map_err(|e| e.to_string())?;
        let bytes =
            fs::read(assets.join(&name)).map_err(|e| format!("Missing asset {name}: {e}"))?;
        key.verify(&bytes, &signature, true)
            .map_err(|e| format!("Invalid {platform} signature: {e}"))?;
        println!("Verified {platform}: {name} ({} bytes)", bytes.len());
    }
    Ok(platforms.len())
}
fn main() {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        eprintln!(
            "Usage: verify-inzoi-release <tag-config.json> <latest.json> <assets-dir> <vTAG>"
        );
        std::process::exit(2)
    }
    match verify(Path::new(&a[1]), Path::new(&a[2]), Path::new(&a[3]), &a[4]) {
        Ok(count) => println!("Verified {count} signed updater entries; ready to publish."),
        Err(e) => {
            eprintln!("Release verification failed: {e}");
            std::process::exit(1)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_urls_outside_release_or_path_escape() {
        let prefix =
            "https://github.com/coolbirdzik/inzoi-better-mod-manager/releases/download/v0.1.1/";
        for s in [
            "https://other.example/a.zip",
            &format!("{prefix}../bad.exe"),
            &format!("{prefix}%2e%2e%2fbad.exe"),
            &format!("{prefix}C%3Aevil.exe"),
        ] {
            assert!(asset_name(s, prefix).is_err())
        }
        assert_eq!(
            asset_name(&format!("{prefix}Better%20Mod.exe"), prefix).unwrap(),
            "Better Mod.exe"
        );
    }
}
