use super::Result;
use keyring::{Entry, Error};

const SERVICE: &str = "com.coolbirdzik.inzoi-better-mod-manager.nexus";
const ACCOUNT: &str = "api-key";

fn entry() -> Result<Entry> {
    // Use the same OS-account credential after updates, in dev, and in portable
    // copies. Never fall back to the mock store or a plaintext settings file.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Entry::new(SERVICE, ACCOUNT).map_err(|_| "nexusKeyLoadFailed".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("nexusKeyLoadFailed".into())
    }
}

fn load(entry: &Entry) -> Result<Option<String>> {
    match entry.get_password() {
        Ok(value) => Ok((!value.trim().is_empty()).then(|| value.trim().to_string())),
        Err(Error::NoEntry) => Ok(None),
        Err(_) => Err("nexusKeyLoadFailed".into()),
    }
}

fn save(entry: &Entry, api_key: &str) -> Result<()> {
    let api_key = api_key.trim();
    if api_key.len() > 2048 || api_key.chars().any(char::is_control) {
        return Err("nexusKeySaveFailed".into());
    }
    if api_key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(_) => Err("nexusKeySaveFailed".into()),
        }
    } else {
        entry
            .set_password(api_key)
            .map_err(|_| "nexusKeySaveFailed".into())
    }
}

#[tauri::command(async)]
pub(crate) fn load_nexus_api_key() -> Result<Option<String>> {
    load(&entry()?)
}

#[tauri::command(async)]
pub(crate) fn save_nexus_api_key(api_key: String) -> Result<()> {
    let entry = entry().map_err(|_| "nexusKeySaveFailed")?;
    save(&entry, &api_key)
}

fn curseforge_entry() -> Result<Entry> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Entry::new(
            "com.coolbirdzik.inzoi-better-mod-manager.curseforge",
            ACCOUNT,
        )
        .map_err(|_| "curseforgeKeyLoadFailed".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("curseforgeKeyLoadFailed".into())
    }
}
#[tauri::command(async)]
pub(crate) fn load_curseforge_api_key() -> Result<Option<String>> {
    load(&curseforge_entry()?).map_err(|_| "curseforgeKeyLoadFailed".into())
}
#[tauri::command(async)]
pub(crate) fn save_curseforge_api_key(api_key: String) -> Result<()> {
    save(
        &curseforge_entry().map_err(|_| "curseforgeKeySaveFailed")?,
        &api_key,
    )
    .map_err(|_| "curseforgeKeySaveFailed".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock() -> Entry {
        Entry::new_with_credential(Box::<keyring::mock::MockCredential>::default())
    }

    #[test]
    fn key_is_trimmed_replaced_and_deleted_without_touching_library_settings() {
        let entry = mock();
        assert!(load(&entry).unwrap().is_none());
        save(&entry, "  FIRST_FAKE_KEY  ").unwrap();
        assert!(load(&entry).unwrap().as_deref() == Some("FIRST_FAKE_KEY"));
        save(&entry, "SECOND_FAKE_KEY").unwrap();
        assert!(load(&entry).unwrap().as_deref() == Some("SECOND_FAKE_KEY"));
        save(&entry, " \n ").unwrap();
        assert!(load(&entry).unwrap().is_none());
        save(&entry, "").unwrap();
    }

    #[test]
    fn failures_never_echo_secrets_and_invalid_input_keeps_the_previous_key() {
        let entry = mock();
        save(&entry, "PREVIOUS_FAKE_KEY").unwrap();
        for invalid in ["FAKE\nKEY".to_string(), "x".repeat(2049)] {
            assert_eq!(save(&entry, &invalid).unwrap_err(), "nexusKeySaveFailed");
            assert!(load(&entry).unwrap().as_deref() == Some("PREVIOUS_FAKE_KEY"));
        }
        let mock = entry
            .get_credential()
            .downcast_ref::<keyring::mock::MockCredential>()
            .unwrap();
        mock.set_error(Error::Invalid("FAKE_SECRET".into(), "FAKE_SECRET".into()));
        assert_eq!(load(&entry).unwrap_err(), "nexusKeyLoadFailed");
        mock.set_error(Error::Invalid("FAKE_SECRET".into(), "FAKE_SECRET".into()));
        assert_eq!(
            save(&entry, "NEW_FAKE_KEY").unwrap_err(),
            "nexusKeySaveFailed"
        );
        assert!(load(&entry).unwrap().as_deref() == Some("PREVIOUS_FAKE_KEY"));
        mock.set_error(Error::Invalid("FAKE_SECRET".into(), "FAKE_SECRET".into()));
        assert_eq!(save(&entry, "").unwrap_err(), "nexusKeySaveFailed");
        assert!(load(&entry).unwrap().as_deref() == Some("PREVIOUS_FAKE_KEY"));
    }

    #[test]
    #[ignore = "Writes an isolated temporary test credential to the current OS account"]
    fn native_store_survives_recreating_the_entry_and_can_be_cleared() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Ok(entry) = Entry::new(SERVICE, &self.0) {
                    let _ = entry.delete_credential();
                }
            }
        }
        let _cleanup = Cleanup(account.clone());
        let first = Entry::new(SERVICE, &account).unwrap();
        assert!(load(&first).unwrap().is_none());
        save(&first, "  INZOIMM_FAKE_TEST_KEY  ").unwrap();
        drop(first);
        let reloaded = Entry::new(SERVICE, &account).unwrap();
        assert!(load(&reloaded).unwrap().as_deref() == Some("INZOIMM_FAKE_TEST_KEY"));
        save(&reloaded, "").unwrap();
        assert!(load(&Entry::new(SERVICE, &account).unwrap())
            .unwrap()
            .is_none());
        save(&reloaded, "").unwrap();
    }
}
