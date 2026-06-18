//! API keys in the OS keychain — never the vault. Only whitelisted keys are allowed;
//! the frontend can set a key and ask whether one is present, but never reads it back.
//! Real value comes from the OS keychain via the `apple-native` keyring backend.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const SECRET_KEYS: &[&str] = &["scrapingbee_api_key", "openrouter_api_key"];

/// Keychain service namespace for this app's secrets.
const SERVICE: &str = "dev.lodestar.lodestar";

/// One cached `Entry` per whitelisted key, reused across calls.
///
/// Reuse is correct for the real OS keychain — an `Entry` is only a handle to the
/// (service, key) record, not a cached copy of the value, so each `get_password`
/// still reads the live keychain. It is also *required* by the test mock, whose store
/// lives in the entry itself (`CredentialPersistence::EntryOnly`): a fresh `Entry` per
/// call would never see a value set on a previous, separate `Entry`.
fn entries() -> &'static Mutex<HashMap<&'static str, Arc<keyring::Entry>>> {
    static ENTRIES: OnceLock<Mutex<HashMap<&'static str, Arc<keyring::Entry>>>> = OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The cached keychain entry for a whitelisted key, or an error for an unknown key.
fn entry(key: &str) -> Result<Arc<keyring::Entry>, String> {
    let canonical = SECRET_KEYS
        .iter()
        .copied()
        .find(|&k| k == key)
        .ok_or_else(|| format!("unknown secret key {key:?}; expected one of {SECRET_KEYS:?}"))?;
    let mut map = entries().lock().map_err(|e| e.to_string())?;
    if let Some(e) = map.get(canonical) {
        return Ok(e.clone());
    }
    let e = Arc::new(keyring::Entry::new(SERVICE, canonical).map_err(|e| e.to_string())?);
    map.insert(canonical, e.clone());
    Ok(e)
}

pub fn set_secret_value(key: &str, value: &str) -> Result<(), String> {
    entry(key)?.set_password(value).map_err(|e| e.to_string())
}

/// Backend-only: read a key's value (used by the scraper/LLM clients in Phase A).
#[allow(dead_code)]
pub fn get_secret(key: &str) -> Result<String, String> {
    entry(key)?.get_password().map_err(|e| e.to_string())
}

pub fn is_present(key: &str) -> Result<bool, String> {
    match entry(key)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_secret(key: String, value: String) -> Result<(), String> {
    set_secret_value(&key, &value)
}

#[tauri::command]
pub fn secret_present(key: String) -> Result<bool, String> {
    is_present(&key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static MOCK: Once = Once::new();
    fn use_mock_keychain() {
        // Route all keyring calls to an in-memory mock store (no real keychain / prompts).
        // `Once` guarantees the mock builder is installed before any entry is created,
        // even under parallel tests.
        MOCK.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[test]
    fn set_get_present_round_trip() {
        use_mock_keychain();
        assert!(!is_present("openrouter_api_key").unwrap());
        set_secret_value("openrouter_api_key", "sk-or-123").unwrap();
        assert!(is_present("openrouter_api_key").unwrap());
        assert_eq!(get_secret("openrouter_api_key").unwrap(), "sk-or-123");
    }

    #[test]
    fn rejects_unknown_key() {
        use_mock_keychain();
        assert!(set_secret_value("aws_secret", "x").is_err());
        assert!(is_present("aws_secret").is_err());
        assert!(get_secret("aws_secret").is_err());
    }
}
