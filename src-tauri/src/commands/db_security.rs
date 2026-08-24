//! Database security commands (MISSION-112).
//!
//! Opt-in SQLCipher at-rest encryption, driven from Settings instead of the
//! environment. The passphrase lives in the same secret pipeline as provider
//! keys (`db.encryption` entry) and is loaded at startup before the database
//! opens; `MYLORE_DB_KEY` overrides it for power users.
//!
//! The migration primitive is SQLCipher's `PRAGMA rekey`: encrypting a live
//! plaintext database (or decrypting with an empty target) happens in place,
//! no file dance. The caller shows a "restart required" notice afterwards.

use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::error::AppError;
use crate::infrastructure::keyring::SecretStore;

/// Secret-pipeline entry name for the database passphrase.
pub const DB_KEY_ENTRY: &str = "db.encryption";

#[derive(serde::Serialize)]
pub struct DbSecurityStatus {
    /// Whether this build can encrypt at all (db-encryption feature).
    pub available: bool,
    /// Whether the on-disk library file is currently encrypted.
    pub encrypted: bool,
}

fn quote_passphrase(passphrase: &str) -> String {
    format!("'{}'", passphrase.replace('\'', "''"))
}

pub use crate::infrastructure::db::detect_encrypted;


/// Snapshot of the current at-rest security posture.
#[command]
pub async fn db_security_status(
    db_path: tauri::State<'_, std::path::PathBuf>,
    store: State<'_, Arc<dyn SecretStore>>,
) -> Result<DbSecurityStatus, AppError> {
    let available = cfg!(feature = "db-encryption");
    // Blocking file read inside async: tiny (16 bytes), acceptable.
    let encrypted = detect_encrypted(&db_path);
    let _ = &store; // presence documents where the key would live
    Ok(DbSecurityStatus {
        available,
        encrypted,
    })
}

/// Encrypt the live database in place and persist the passphrase.
#[command]
pub async fn db_enable_encryption(
    pool: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    passphrase: String,
) -> Result<(), AppError> {
    if !cfg!(feature = "db-encryption") {
        return Err(AppError::validation(
            "this build was compiled without database encryption support",
        ));
    }
    let trimmed = passphrase.trim();
    if trimmed.len() < 8 {
        return Err(AppError::validation(
            "passphrase must be at least 8 characters",
        ));
    }

    // rekey must run as a single statement; escape like the key pragma.
    let statement = format!("PRAGMA rekey = {};", quote_passphrase(trimmed));
    sqlx::query(&statement)
        .execute(pool.inner())
        .await
        .map_err(|e| AppError::internal(format!("rekey failed: {e}")))?;

    store
        .set(DB_KEY_ENTRY, trimmed)
        .map_err(|e| AppError::internal(format!("storing passphrase failed: {e}")))?;
    info!("database encryption enabled; restart recommended");
    Ok(())
}

/// Decrypt the live database back to plaintext and drop the stored passphrase.
#[command]
pub async fn db_disable_encryption(
    pool: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
) -> Result<(), AppError> {
    if !cfg!(feature = "db-encryption") {
        return Err(AppError::validation(
            "this build was compiled without database encryption support",
        ));
    }
    // Empty rekey target decrypts to a plaintext database.
    sqlx::query("PRAGMA rekey = '';")
        .execute(pool.inner())
        .await
        .map_err(|e| AppError::internal(format!("decrypt failed: {e}")))?;

    let _ = store.delete(DB_KEY_ENTRY);
    info!("database decryption enabled; restart recommended");
    Ok(())
}

#[cfg(all(test, feature = "db-encryption"))]
mod encryption_command_tests {
    use super::*;

    #[test]
    fn passphrase_quoting_escapes_single_quotes() {
        assert_eq!(quote_passphrase("plain"), "'plain'");
        assert_eq!(quote_passphrase("it's"), "'it''s'");
    }

    #[test]
    fn detect_encrypted_distinguishes_plaintext_header() {
        let dir = std::env::temp_dir().join(format!("ml-sec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.db");
        std::fs::write(&plain, b"SQLite format 3\0rest-of-header").unwrap();
        assert!(!detect_encrypted(&plain));

        let cipher = dir.join("cipher.db");
        std::fs::write(&cipher, [0xDEu8; 64]).unwrap();
        assert!(detect_encrypted(&cipher));

        std::fs::remove_dir_all(&dir).ok();
    }
}

/// MISSION-112 - the stored database passphrase, for "copy passphrase"
/// affordances when moving an encrypted archive to another machine. Local
/// only; the frontend copies it to the clipboard.
#[command]
pub fn db_get_passphrase(
    store: State<'_, Arc<dyn SecretStore>>,
) -> Result<Option<String>, AppError> {
    Ok(store.get(DB_KEY_ENTRY).map_err(AppError::internal)?)
}
