//! OS secret store access (MISSION-063).
//!
//! Provider API keys live in the operating system's secure store (Windows
//! Credential Manager, macOS Keychain, Linux Secret Service) — never in the
//! database, the webview, or logs. `SecretStore` is a tiny injectable trait so
//! the settings service is unit-testable with an in-memory store; production
//! uses `OsKeyring`.

use std::collections::HashMap;
use std::sync::Mutex;

/// The keyring "service" segment that identifies MyLore credentials.
pub const KEYRING_SERVICE: &str = "mylore";

/// A minimal, injectable secret store. Errors are plain strings so the caller
/// decides how to surface them (they must never contain the secret).
pub trait SecretStore: Send + Sync {
    /// The stored secret for `user`, or None when absent/unavailable.
    fn get(&self, user: &str) -> Option<String>;
    /// Persist (or overwrite) `secret` for `user`.
    fn set(&self, user: &str, secret: &str) -> Result<(), String>;
    /// Remove the entry for `user`; deleting a missing entry is a no-op.
    fn delete(&self, user: &str) -> Result<(), String>;
}

/// Production store backed by the `keyring` crate (native OS credential store).
pub struct OsKeyring;

impl SecretStore for OsKeyring {
    fn get(&self, user: &str) -> Option<String> {
        keyring::Entry::new(KEYRING_SERVICE, user)
            .ok()
            .and_then(|entry| entry.get_password().ok())
    }

    fn set(&self, user: &str, secret: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, user).map_err(|e| e.to_string())?;
        entry.set_password(secret).map_err(|e| e.to_string())
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, user).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Test store that keeps secrets in memory. Prevents unit tests from touching
/// (or being blocked by) the real OS credential store.
#[derive(Default)]
pub struct InMemoryKeyring {
    inner: Mutex<HashMap<String, String>>,
}

impl InMemoryKeyring {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemoryKeyring {
    fn get(&self, user: &str) -> Option<String> {
        self.inner.lock().unwrap().get(user).cloned()
    }

    fn set(&self, user: &str, secret: &str) -> Result<(), String> {
        self.inner.lock().unwrap().insert(user.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        self.inner.lock().unwrap().remove(user);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_secrets() {
        let store = InMemoryKeyring::new();
        assert_eq!(store.get("tmdb"), None);
        store.set("tmdb", "sekret").unwrap();
        assert_eq!(store.get("tmdb").as_deref(), Some("sekret"));
        store.set("tmdb", "new-key").unwrap();
        assert_eq!(store.get("tmdb").as_deref(), Some("new-key"));
        store.delete("tmdb").unwrap();
        assert_eq!(store.get("tmdb"), None);
    }

    #[test]
    fn in_memory_delete_of_missing_entry_is_a_noop() {
        let store = InMemoryKeyring::new();
        assert!(store.delete("nope").is_ok());
    }

    #[test]
    fn in_memory_stores_are_isolated_per_user() {
        let store = InMemoryKeyring::new();
        store.set("tmdb", "a").unwrap();
        store.set("googlebooks", "b").unwrap();
        assert_eq!(store.get("tmdb").as_deref(), Some("a"));
        assert_eq!(store.get("googlebooks").as_deref(), Some("b"));
    }
}
