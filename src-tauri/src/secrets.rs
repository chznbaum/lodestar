//! API keys in the OS keychain — never the vault. Only whitelisted keys are allowed;
//! the frontend can set a key and ask whether one is present, but never reads it back.
//! Real value comes from the OS keychain via the `apple-native` keyring backend.
//!
//! Each key's value is **cached in memory after its first read**, so the OS keychain is
//! touched at most once per key per session — otherwise macOS prompts for authorization on
//! every access (presence checks, each pipeline step, every retry → a flood of prompts).
//!
//! `is_present` uses a `SecItemCopyMatching` existence query (no `kSecReturnData`) so it
//! never decrypts the value and therefore never triggers an OS auth prompt.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const SECRET_KEYS: &[&str] = &["scrapingbee_api_key", "openrouter_api_key"];

/// Keychain service namespace for this app's secrets.
const SERVICE: &str = "dev.lodestar.lodestar";

/// In-memory value cache, populated on first read/write. Keeps keychain access (and its
/// per-access auth prompt) to once per key per session.
fn cache() -> &'static Mutex<HashMap<&'static str, String>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Validate against the whitelist, returning the canonical `&'static str` key.
fn canonical(key: &str) -> Result<&'static str, String> {
    SECRET_KEYS
        .iter()
        .copied()
        .find(|&k| k == key)
        .ok_or_else(|| format!("unknown secret key {key:?}; expected one of {SECRET_KEYS:?}"))
}

fn entry(key: &'static str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())
}

pub fn set_secret_value(key: &str, value: &str) -> Result<(), String> {
    let k = canonical(key)?;
    entry(k)?.set_password(value).map_err(|e| e.to_string())?;
    cache()
        .lock()
        .map_err(|e| e.to_string())?
        .insert(k, value.to_string());
    Ok(())
}

/// Backend-only: read a key's value (used by the scraper/LLM clients). Cached after first read.
#[allow(dead_code)]
pub fn get_secret(key: &str) -> Result<String, String> {
    let k = canonical(key)?;
    if let Some(v) = cache().lock().map_err(|e| e.to_string())?.get(k) {
        return Ok(v.clone());
    }
    let v = entry(k)?.get_password().map_err(|e| e.to_string())?;
    cache()
        .lock()
        .map_err(|e| e.to_string())?
        .insert(k, v.clone());
    Ok(v)
}

/// Check whether a key exists in the keychain without reading (and decrypting) its value.
///
/// On macOS in production builds: uses `SecItemCopyMatching` with no `kSecReturnData` —
/// returns success/not-found without triggering an auth prompt (no decryption occurs).
///
/// In test builds: the cfg routes to the keyring-based path so the mock keychain works.
/// On non-macOS: the keyring-based path is used.
pub fn is_present(key: &str) -> Result<bool, String> {
    let k = canonical(key)?;
    // Cache fast-path: a key set or read this session is known present without any keychain call.
    // This is what makes the round-trip test pass under the mock keychain.
    if cache().lock().map_err(|e| e.to_string())?.contains_key(k) {
        return Ok(true);
    }
    is_present_uncached(k)
}

/// Inner existence check, called only on a cache miss.
///
/// macOS production path: `SecItemCopyMatching` with `kSecClass=GenericPassword`,
/// `kSecAttrService="dev.lodestar.lodestar"`, `kSecAttrAccount=<key>`, `kSecMatchLimit=One`,
/// and **no `kSecReturnData`** — existence-only, no decryption, no OS auth prompt.
///
/// Gated out of test builds with `#[cfg(not(test))]` so mock-keychain tests don't bypass
/// the mock. The condition `any(not(target_os="macos"), test)` covers the fallback.
#[cfg(all(target_os = "macos", not(test)))]
fn is_present_uncached(k: &'static str) -> Result<bool, String> {
    use security_framework::item::{ItemClass, ItemSearchOptions};
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
    match ItemSearchOptions::new()
        .class(ItemClass::generic_password())
        .service(SERVICE)
        .account(k)
        .search()
    {
        Ok(_) => Ok(true),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

/// Fallback: keyring-based existence check (decrypts, but only used in tests behind the mock
/// and on non-macOS targets where there's no OS auth prompt).
#[cfg(any(not(target_os = "macos"), test))]
fn is_present_uncached(k: &'static str) -> Result<bool, String> {
    match entry(k)?.get_password() {
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
        assert_eq!(get_secret("openrouter_api_key").unwrap(), "sk-or-123"); // served from cache
    }

    #[test]
    fn rejects_unknown_key() {
        use_mock_keychain();
        assert!(set_secret_value("aws_secret", "x").is_err());
        assert!(is_present("aws_secret").is_err());
        assert!(get_secret("aws_secret").is_err());
    }
}
