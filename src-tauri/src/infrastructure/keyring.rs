//! Secret storage for provider API keys and the database passphrase
//! (MISSION-063, hardened MISSION-098/112).
//!
//! `SecretStore` is the port; the production implementation is
//! `FileSecretStore`, which persists a **plaintext JSON map** inside the app's
//! data directory (`api_keys.json`). It is deliberately not cryptographic — the
//! library DB is also plaintext by default, and when SQLCipher encryption is
//! enabled (MISSION-112) the passphrase that unlocks it lives in this same
//! store. An OS-keyring-backed implementation was tried and retired: the
//! Windows Credential Manager round-trip proved unreliable (CredWrite succeeds
//! but CredRead returns NOT_FOUND on some configurations). `InMemoryKeyring`
//! backs tests.

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
        let inner = self
            .inner
            .lock()
            .map_err(|_| "secret store lock poisoned".to_string())?;
        Ok(inner.get(user).cloned())
    }

    fn set(&self, user: &str, secret: &str) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "secret store lock poisoned".to_string())?;
        inner.insert(user.into(), secret.into());
        self.flush(&inner)
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "secret store lock poisoned".to_string())?;
        inner.remove(user);
        self.flush(&inner)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn temp_store_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mylore-keyring-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn twin_instances_lose_writes_to_the_other_twin() {
        // MISSION-139 regression: two independent FileSecretStore instances
        // over the same file each hold a stale in-memory map, so a write
        // through one clobbers a write through the other on flush. This test
        // documents the failure the Arc-sharing fix eliminates in production.
        let path = temp_store_file("twins.json");
        let _ = std::fs::remove_file(&path);

        let twin_a = FileSecretStore::load(path.clone());
        let twin_b = FileSecretStore::load(path.clone());
        twin_a.set("tmdb", "key-a").unwrap();
        twin_b.set("db.encryption", "pass-b").unwrap();

        // Both maps were loaded empty; each flushed only its own entry, so the
        // file ends with exactly one of the two keys — a silent secret loss.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            !(on_disk.contains("tmdb") && on_disk.contains("db.encryption")),
            "two twins must not both survive (documents the lost-write bug)"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn shared_arc_persists_both_writers() {
        // The production shape (MISSION-139): one Arc<dyn SecretStore> shared
        // by the settings service and the db-security commands. A write through
        // either consumer is visible to — and persisted by — the same map.
        let path = temp_store_file("shared.json");
        let _ = std::fs::remove_file(&path);

        let shared: Arc<dyn SecretStore> = Arc::new(FileSecretStore::load(path.clone()));
        let settings_consumer = shared.clone();
        let security_consumer = shared.clone();

        settings_consumer.set("tmdb", "key-a").unwrap();
        security_consumer.set("db.encryption", "pass-b").unwrap();

        assert_eq!(shared.get("tmdb").unwrap().as_deref(), Some("key-a"));
        assert_eq!(
            shared.get("db.encryption").unwrap().as_deref(),
            Some("pass-b")
        );
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("tmdb") && on_disk.contains("db.encryption"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn operations_report_errors_on_a_poisoned_lock() {
        // MISSION-141: a poisoned lock must surface as an error, never panic
        // the request thread. Poison by panicking while holding the guard.
        let store = FileSecretStore::load(temp_store_file("poisoned.json"));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = store.inner.lock().unwrap();
            panic!("simulated panic while holding the secret-store lock");
        }));

        assert!(
            store.set("tmdb", "key").is_err(),
            "set on a poisoned store returns an error"
        );
        assert!(
            store.get("tmdb").is_err(),
            "get on a poisoned store returns an error"
        );
        assert!(
            store.delete("tmdb").is_err(),
            "delete on a poisoned store returns an error"
        );
    }
}
