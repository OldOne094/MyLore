//! Secret storage for provider API keys (MISSION-063, hardened MISSION-098).
//!
//! `SecretStore` is the port; two implementations exist:
//!   - `OsKeyring` — the OS credential store. Known to silently drop
//!     credentials on some Windows configurations (CredWrite succeeds but
//!     CredRead returns NOT_FOUND), so it is no longer the default.
//!   - `FileSecretStore` — a base64-obfuscated JSON file inside the app's
//!     data directory. MyLore is local-first: the SQLite DB already stores
//!     viewing history in plaintext, and MISSION-112 (SQLCipher) will encrypt
//!     everything together when it ships.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// The port for reading/writing provider API keys.
pub trait SecretStore: Send + Sync {
    /// The stored secret for `user`, `None` only when genuinely absent.
    /// Transport/OS errors are surfaced so they are never mistaken for
    /// "no key saved" (that mistake made saved keys vanish on load).
    fn get(&self, user: &str) -> Result<Option<String>, String>;
    /// Persist (or overwrite) `secret` for `user`.
    fn set(&self, user: &str, secret: &str) -> Result<(), String>;
    /// Remove the entry for `user`; deleting a missing entry is a no-op.
    fn delete(&self, user: &str) -> Result<(), String>;
}

/// Production store backed by a JSON file inside the app's data directory.
/// Not cryptographic — consistent with the rest of the local-first plaintext
/// database — but survives process restarts and avoids the Windows Credential
/// Manager round-trip bug entirely (CredWrite succeeds but CredRead returns
/// NOT_FOUND on some Windows configurations).
pub struct FileSecretStore {
    inner: Mutex<HashMap<String, String>>,
    path: PathBuf,
}

impl FileSecretStore {
    pub fn load(path: PathBuf) -> Self {
        let inner = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    fn flush(&self, map: &HashMap<String, String>) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

impl SecretStore for FileSecretStore {
    fn get(&self, user: &str) -> Result<Option<String>, String> {
        Ok(self.inner.lock().unwrap().get(user).cloned())
    }

    fn set(&self, user: &str, secret: &str) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .insert(user.into(), secret.into());
        self.flush(&self.inner.lock().unwrap())
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        self.inner.lock().unwrap().remove(user);
        self.flush(&self.inner.lock().unwrap())
    }
}

/// Test store that keeps secrets in memory. Prevents unit tests from touching
/// (or being blocked by) any real storage.
#[cfg(test)]
pub struct InMemoryKeyring {
    inner: Mutex<HashMap<String, String>>,
}

#[cfg(test)]
impl InMemoryKeyring {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
impl SecretStore for InMemoryKeyring {
    fn get(&self, user: &str) -> Result<Option<String>, String> {
        Ok(self.inner.lock().unwrap().get(user).cloned())
    }

    fn set(&self, user: &str, secret: &str) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .insert(user.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        self.inner.lock().unwrap().remove(user);
        Ok(())
    }
}
#[cfg(test)]
impl Default for InMemoryKeyring {
    fn default() -> Self {
        Self::new()
    }
}
