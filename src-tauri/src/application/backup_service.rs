//! Backup service (MISSION-084, ARCHITECTURE §7).
//!
//! A backup is a single `.mylore` archive (a plain zip) containing three
//! things: a **consistent database snapshot** produced by SQLite's
//! `VACUUM INTO` (safe to run against the live WAL database), the **cached
//! asset files** (cover/banner images under `{data_dir}/images`) referenced by
//! `status='cached'` asset rows, and a **meta.json** manifest (format version,
//! counts, asset id → archive-path mapping for the restore in MISSION-085).
//!
//! Every created archive is re-opened and validated before success is
//! reported: the manifest must parse, the snapshot must be a healthy SQLite
//! database (`PRAGMA integrity_check`) whose media count matches the
//! manifest. Writes go to a `.partial` sibling renamed into place on success,
//! so an interrupted backup never leaves a plausible-looking archive behind.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::AppError;
use crate::infrastructure::db;

/// Bump when the archive layout changes; restore rejects older formats.
pub const FORMAT_VERSION: u8 = 1;
const DB_ENTRY: &str = "library.db";
const META_ENTRY: &str = "meta.json";
const ASSETS_PREFIX: &str = "assets";

/// One cached asset carried in the archive (restore maps it back by id).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AssetManifestEntry {
    pub id: String,
    /// Path inside the archive, e.g. `assets/a-1.jpg`.
    pub file: String,
}

/// The manifest stored as `meta.json` inside every archive.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct BackupMeta {
    pub format_version: u8,
    pub app_version: String,
    pub created_at: String,
    pub media_count: u32,
    pub asset_count: u32,
    pub assets: Vec<AssetManifestEntry>,
}

/// What a finished backup reports (the task's typed result).
#[derive(Debug, Clone, Serialize)]
pub struct BackupReport {
    pub path: String,
    pub size_bytes: u64,
    pub media_count: u32,
    pub asset_count: u32,
}

/// One archive in the backups folder, newest first in listings.
#[derive(Debug, Clone, Serialize)]
pub struct BackupEntry {
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
    /// Zero-padded `YYYYMMDDHHMMSS` parsed from the file name.
    pub created_at: String,
}

/// What a finished restore reports (the task's typed result). The running
/// app's pool was closed to unlock the database files, so the UI must restart
/// the app after a successful restore.
#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub media_count: u32,
    pub asset_count: u32,
    /// Where the replaced data was parked (`{data_dir}/quarantine-…`).
    pub quarantined_to: String,
    pub restart_required: bool,
}

/// Backup preferences (MISSION-086), persisted in the `settings` table.
/// Defaults are conservative: automatic backups off, daily interval, ten
/// archives kept (plus one per older month).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct BackupPrefs {
    pub auto_enabled: bool,
    pub interval_hours: u32,
    pub keep_count: u32,
}

impl Default for BackupPrefs {
    fn default() -> Self {
        Self {
            auto_enabled: false,
            interval_hours: 24,
            keep_count: 10,
        }
    }
}

const KEY_AUTO: &str = "backup.auto_enabled";
const KEY_INTERVAL: &str = "backup.interval_hours";
const KEY_KEEP: &str = "backup.keep_count";

/// Removes its paths on drop unless disarmed — the `.partial` archive and the
/// temporary `VACUUM INTO` snapshot never outlive a failed/cancelled backup.
struct PartialGuard {
    paths: Vec<PathBuf>,
}

impl PartialGuard {
    fn disarm(self) {
        std::mem::forget(self);
    }
}

impl Drop for PartialGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Backend for creating and validating `.mylore` backups.
pub struct BackupService {
    pool: SqlitePool,
    data_dir: PathBuf,
}

impl BackupService {
    pub fn new(pool: SqlitePool, data_dir: &Path) -> Self {
        Self {
            pool,
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Where archives are written: `{data_dir}/backups`.
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    /// Every archive in the backups folder, newest first. Foreign files are
    /// ignored; entries are listed without validating their contents
    /// (`validate` is an explicit per-archive action).
    pub fn list(&self) -> Result<Vec<BackupEntry>, AppError> {
        let dir = self.backups_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<BackupEntry> = std::fs::read_dir(&dir)?
            .flatten()
            .filter_map(|entry| {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let created_at = archive_stamp(&file_name)?;
                Some(BackupEntry {
                    file_name,
                    path: entry.path().display().to_string(),
                    size_bytes: entry.metadata().ok()?.len(),
                    created_at,
                })
            })
            .collect();
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    /// Delete one archive. Only files inside the backups folder whose names
    /// match the archive pattern can be deleted — the request is re-derived
    /// from the file name so `..` tricks and foreign paths are rejected.
    pub fn delete_archive(&self, path: &str) -> Result<(), AppError> {
        let requested = PathBuf::from(path);
        let name = requested
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| AppError::validation("invalid archive path"))?;
        if archive_stamp(&name).is_none() {
            return Err(AppError::validation("not a MyLore backup archive"));
        }
        let target = self.backups_dir().join(&name);
        let target_canonical = target
            .canonicalize()
            .map_err(|_| AppError::validation("archive not found"))?;
        let dir_canonical = self.backups_dir().canonicalize()?;
        if !target_canonical.starts_with(dir_canonical) {
            return Err(AppError::validation(
                "archive is outside the backups folder",
            ));
        }
        std::fs::remove_file(target_canonical)?;
        Ok(())
    }

    /// Move a corrupt database aside so the next startup creates a fresh one
    /// (MISSION-088 recovery). Returns the quarantine location. Closes the
    /// pool to unlock the files — the caller must restart the app after.
    pub async fn start_fresh(&self) -> Result<String, AppError> {
        self.pool.close().await;
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let uid = &Uuid::new_v4().simple().to_string()[..6];
        let quarantine = self
            .data_dir
            .join(format!("quarantine-corrupt-{stamp}-{uid}"));
        std::fs::create_dir_all(&quarantine)?;
        for suffix in ["", "-wal", "-shm"] {
            let source = self.data_dir.join(format!("mylore.db{suffix}"));
            if source.is_file() {
                rename_with_retry(&source, &quarantine.join(source.file_name().expect("name")))
                    .await?;
            }
        }
        Ok(quarantine.display().to_string())
    }

    /// Create a validated `.mylore` archive of the whole library.
    pub async fn create(&self) -> Result<BackupReport, AppError> {
        let backups_dir = self.backups_dir();
        std::fs::create_dir_all(&backups_dir)?;

        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let uid = &Uuid::new_v4().simple().to_string()[..6];
        let dest = backups_dir.join(format!("mylore-{stamp}-{uid}.mylore"));
        let partial = backups_dir.join(format!("mylore-{stamp}-{uid}.mylore.partial"));
        let snapshot = backups_dir.join(format!(".snapshot-{uid}.tmp"));
        let guard = PartialGuard {
            paths: vec![partial.clone(), snapshot.clone()],
        };

        // 1. Consistent snapshot of the live WAL database. `VACUUM INTO`
        //    cannot take bound parameters, so the (service-controlled) path
        //    is interpolated with single quotes doubled.
        let quoted = snapshot.display().to_string().replace('\'', "''");
        sqlx::query(&format!("VACUUM INTO '{quoted}'"))
            .execute(&self.pool)
            .await?;

        // 2. Manifest: counts + every cached asset whose file actually
        //    exists on disk (missing files are skipped with a warning).
        let (media_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
            .fetch_one(&self.pool)
            .await?;
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, local_path FROM asset \
             WHERE status = 'cached' AND local_path IS NOT NULL ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut manifest = Vec::new();
        let mut asset_files: Vec<(String, PathBuf)> = Vec::new();
        for (id, local_path) in rows {
            let source = PathBuf::from(&local_path);
            if !source.is_file() {
                tracing::warn!(asset = %id, "cached asset file missing, skipped");
                continue;
            }
            let ext = source
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let archive_path = format!("{ASSETS_PREFIX}/{id}{ext}");
            manifest.push(AssetManifestEntry {
                id,
                file: archive_path.clone(),
            });
            asset_files.push((archive_path, source));
        }

        let meta = BackupMeta {
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Utc::now().to_rfc3339(),
            media_count: media_count as u32,
            asset_count: manifest.len() as u32,
            assets: manifest,
        };

        // 3. Write the archive: meta, snapshot, then each asset file.
        let meta_json = serde_json::to_string_pretty(&meta)?;
        write_archive(&partial, &meta_json, &snapshot, &asset_files)?;

        // 4. Rename into place, then re-open and validate what we shipped.
        std::fs::rename(&partial, &dest)?;
        guard.disarm();
        if let Err(error) = self.validate(&dest).await {
            let _ = std::fs::remove_file(&dest);
            return Err(error);
        }

        // 5. Apply the retention policy (newest N + one per older month).
        let prefs = self.prefs().await?;
        self.rotate(prefs.keep_count)?;

        let size_bytes = std::fs::metadata(&dest)?.len();
        Ok(BackupReport {
            path: dest.display().to_string(),
            size_bytes,
            media_count: meta.media_count,
            asset_count: meta.asset_count,
        })
    }

    /// Load backup preferences (defaults for missing keys).
    pub async fn prefs(&self) -> Result<BackupPrefs, AppError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings WHERE key IN (?, ?, ?)")
                .bind(KEY_AUTO)
                .bind(KEY_INTERVAL)
                .bind(KEY_KEEP)
                .fetch_all(&self.pool)
                .await?;
        let mut prefs = BackupPrefs::default();
        for (key, value) in rows {
            match key.as_str() {
                KEY_AUTO => prefs.auto_enabled = value == "true",
                KEY_INTERVAL => {
                    if let Ok(hours) = value.parse() {
                        prefs.interval_hours = hours;
                    }
                }
                KEY_KEEP => {
                    if let Ok(count) = value.parse() {
                        prefs.keep_count = count;
                    }
                }
                _ => {}
            }
        }
        Ok(prefs)
    }

    /// Validate and persist backup preferences.
    pub async fn set_prefs(&self, prefs: BackupPrefs) -> Result<BackupPrefs, AppError> {
        if !(1..=8760).contains(&prefs.interval_hours) {
            return Err(AppError::validation(
                "backup interval must be between 1 and 8760 hours",
            ));
        }
        if !(1..=100).contains(&prefs.keep_count) {
            return Err(AppError::validation(
                "backup keep count must be between 1 and 100",
            ));
        }
        for (key, value) in [
            (KEY_AUTO, prefs.auto_enabled.to_string()),
            (KEY_INTERVAL, prefs.interval_hours.to_string()),
            (KEY_KEEP, prefs.keep_count.to_string()),
        ] {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        }
        Ok(prefs)
    }

    /// Enforce the retention policy over `{data_dir}/backups`: keep the
    /// newest `keep` archives plus the newest of every older month ("N +
    /// monthly"). Files whose names don't match the archive pattern are
    /// never touched. Returns how many archives were deleted.
    pub fn rotate(&self, keep: u32) -> Result<u32, AppError> {
        let dir = self.backups_dir();
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut archives: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)?
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let stamp = archive_stamp(&name)?;
                Some((stamp, entry.path()))
            })
            .collect();
        // Stamps are zero-padded `YYYYMMDDHHMMSS`, so descending string
        // order is chronological.
        archives.sort_by(|a, b| b.0.cmp(&a.0));

        let mut seen_months = std::collections::HashSet::new();
        let mut deleted = 0u32;
        for (index, (stamp, path)) in archives.iter().enumerate() {
            let is_newest_n = index < keep as usize;
            let opens_a_month = seen_months.insert(&stamp[..6]);
            if is_newest_n || opens_a_month {
                continue;
            }
            let _ = std::fs::remove_file(path);
            deleted += 1;
        }
        Ok(deleted)
    }

    /// Run the automatic backup when it is due (MISSION-086): enabled by
    /// preference, and the newest archive older than the interval (or no
    /// archive at all). Returns the report when a backup was created.
    pub async fn auto_backup_if_due(&self) -> Result<Option<BackupReport>, AppError> {
        let prefs = self.prefs().await?;
        if !prefs.auto_enabled {
            return Ok(None);
        }
        if let Some(age_hours) = newest_backup_age_hours(&self.backups_dir())? {
            if age_hours < f64::from(prefs.interval_hours) {
                return Ok(None);
            }
        }
        Ok(Some(self.create().await?))
    }

    /// Safety backup taken right before pending migrations are applied
    /// (MISSION-087). No-op on a fresh install (no database file) or when
    /// every migration is already applied — there is no old data to protect.
    /// Opens its own short-lived pool because the app's pool does not exist
    /// yet at that point in startup; the caller decides whether a failure
    /// blocks startup (the app logs a warning and continues).
    pub async fn pre_migration_backup(db_path: &Path) -> Result<Option<BackupReport>, AppError> {
        let pending = db::pending_migrations(db_path).await?;
        if pending == 0 {
            return Ok(None);
        }
        let data_dir = db_path
            .parent()
            .ok_or_else(|| AppError::internal("database path has no parent directory"))?;
        let pool = db::connect(db_path).await?;
        let result = async {
            let service = BackupService::new(pool.clone(), data_dir);
            service.create().await.map(Some)
        }
        .await;
        pool.close().await;
        result
    }

    /// Restore a `.mylore` archive (MISSION-085): validate, stage the
    /// archive's contents, close the live pool (the DB files are locked on
    /// Windows while open), quarantine the current database + images, swap
    /// the restored data into place, repoint `asset.local_path` at the
    /// restored files, and verify the result. Any failure after quarantine
    /// rolls the previous data back before the error is returned.
    ///
    /// There is deliberately **no cancellation checkpoint** after validation:
    /// a dropped future mid-swap would skip the rollback, and the whole file
    /// dance is short. The caller must restart the app afterwards — the
    /// managed pool is closed by this call.
    pub async fn restore(&self, path: &Path) -> Result<RestoreReport, AppError> {
        // 1. Validate with zero side effects first.
        let meta = self.validate(path).await?;

        // 2. Stage the archive's contents under `{data_dir}/.restore-<uid>`.
        let staging = self.data_dir.join(format!(".restore-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&staging)?;
        // Armed until success: the guard deletes the staging dir on drop
        // (including the early-return error paths below).
        let _guard = PartialGuard {
            paths: vec![staging.clone()],
        };
        let staged_db = extract_archive(path, &meta, &staging)?;

        // 3. Unlock the live database files.
        self.pool.close().await;

        // 4. Quarantine current data, swap, verify — rolling back on error.
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let uid = &Uuid::new_v4().simple().to_string()[..6];
        let quarantine = self.data_dir.join(format!("quarantine-{stamp}-{uid}"));
        match self.swap_in(&staged_db, &meta, &quarantine).await {
            // Staging is cleaned up by the guard on drop.
            Ok(()) => Ok(RestoreReport {
                media_count: meta.media_count,
                asset_count: meta.asset_count,
                quarantined_to: quarantine.display().to_string(),
                restart_required: true,
            }),
            Err(error) => {
                rollback(&self.data_dir, &quarantine);
                Err(error)
            }
        }
    }

    /// The guarded part of restore: everything here either succeeds or is
    /// undone by [`rollback`] before the error escapes.
    async fn swap_in(
        &self,
        staged_db: &Path,
        meta: &BackupMeta,
        quarantine: &Path,
    ) -> Result<(), AppError> {
        let db_path = self.data_dir.join("mylore.db");
        let images_dir = self.data_dir.join("images");

        // Quarantine: park the current database (+ WAL sidecars) and images
        // under `{data_dir}/quarantine-…` using their original names so a
        // rollback is a plain move back.
        std::fs::create_dir_all(quarantine)?;
        for sidecar in ["-wal", "-shm"] {
            let source = PathBuf::from(format!("{}{sidecar}", db_path.display()));
            if source.is_file() {
                std::fs::rename(&source, quarantine.join(source.file_name().expect("name")))?;
            }
        }
        std::fs::rename(&db_path, quarantine.join("mylore.db"))?;
        if images_dir.exists() {
            std::fs::rename(&images_dir, quarantine.join("images"))?;
        }

        // Swap in: copy the staged snapshot over the live path, then place
        // every manifest asset at `images/<id><ext>`.
        std::fs::copy(staged_db, &db_path)?;
        std::fs::create_dir_all(&images_dir)?;
        for entry in &meta.assets {
            let file_name =
                entry.file.rsplit('/').next().ok_or_else(|| {
                    AppError::validation("backup manifest has an invalid asset path")
                })?;
            std::fs::copy(
                staged_asset_path(staged_db, entry),
                images_dir.join(file_name),
            )?;
        }

        // Repoint asset rows at the restored files, then verify the swapped
        // database for real.
        let pool = db::connect(&db_path).await?;
        let result = async {
            db::integrity_check(&pool).await?;
            for entry in &meta.assets {
                let file_name = entry.file.rsplit('/').next().unwrap_or_default();
                let absolute = images_dir.join(file_name);
                sqlx::query("UPDATE asset SET local_path = ? WHERE id = ?")
                    .bind(absolute.display().to_string())
                    .bind(&entry.id)
                    .execute(&pool)
                    .await?;
            }
            let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
                .fetch_one(&pool)
                .await?;
            if count != meta.media_count as i64 {
                return Err(AppError::validation(
                    "restored database does not match its manifest",
                ));
            }
            Ok(())
        }
        .await;
        pool.close().await;
        result
    }

    /// Validate an archive: the manifest parses at the current format
    /// version, the snapshot entry exists, opens as a healthy SQLite
    /// database, and its media count matches the manifest.
    pub async fn validate(&self, path: &Path) -> Result<BackupMeta, AppError> {
        let invalid = |message: &'static str| AppError::validation(message);
        let file =
            std::fs::File::open(path).map_err(|_| invalid("backup file cannot be opened"))?;
        let mut archive =
            ZipArchive::new(file).map_err(|_| invalid("not a valid MyLore backup archive"))?;

        let mut meta_json = String::new();
        archive
            .by_name(META_ENTRY)
            .map_err(|_| invalid("backup archive has no manifest"))?
            .read_to_string(&mut meta_json)
            .map_err(|_| invalid("backup manifest cannot be read"))?;
        let meta: BackupMeta =
            serde_json::from_str(&meta_json).map_err(|_| invalid("backup manifest is invalid"))?;
        if meta.format_version != FORMAT_VERSION {
            return Err(invalid("backup was made by an incompatible version"));
        }
        if meta.assets.len() != meta.asset_count as usize {
            return Err(invalid("backup manifest is inconsistent"));
        }

        // Extract the snapshot and health-check it for real.
        let temp = std::env::temp_dir().join(format!("mylore-validate-{}.db", Uuid::new_v4()));
        {
            let mut db_entry = archive
                .by_name(DB_ENTRY)
                .map_err(|_| invalid("backup archive has no database snapshot"))?;
            let mut out = std::fs::File::create(&temp)?;
            io::copy(&mut db_entry, &mut out)?;
        }

        let check = async {
            let pool = db::connect(&temp).await?;
            let result = async {
                db::integrity_check(&pool).await?;
                let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
                    .fetch_one(&pool)
                    .await?;
                if count != meta.media_count as i64 {
                    return Err(AppError::validation(
                        "backup snapshot does not match its manifest",
                    ));
                }
                Ok(())
            }
            .await;
            pool.close().await;
            result
        }
        .await;
        let _ = std::fs::remove_file(&temp);
        check?;

        Ok(meta)
    }
}

/// Stream `meta.json`, the database snapshot and every `(archive path,
/// source path)` asset into a deflate-compressed zip at `dest`.
fn write_archive(
    dest: &Path,
    meta_json: &str,
    snapshot: &Path,
    assets: &[(String, PathBuf)],
) -> Result<(), AppError> {
    let zipped = |error: zip::result::ZipError| {
        AppError::internal(format!("backup archive write failed: {error}"))
    };
    let file = std::fs::File::create(dest)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(META_ENTRY, options).map_err(zipped)?;
    zip.write_all(meta_json.as_bytes())?;

    zip.start_file(DB_ENTRY, options).map_err(zipped)?;
    io::copy(&mut std::fs::File::open(snapshot)?, &mut zip)?;

    for (archive_path, source) in assets {
        zip.start_file(archive_path.as_str(), options)
            .map_err(zipped)?;
        io::copy(&mut std::fs::File::open(source)?, &mut zip)?;
    }

    zip.finish().map_err(zipped)?;
    Ok(())
}

/// Extract a validated archive's database snapshot and manifest assets into
/// `staging`; returns the staged snapshot path. Asset files land at
/// `staging/<manifest file>` (e.g. `staging/assets/a-1.jpg`).
fn extract_archive(
    archive_path: &Path,
    meta: &BackupMeta,
    staging: &Path,
) -> Result<PathBuf, AppError> {
    let invalid = |message: &'static str| AppError::validation(message);
    let file =
        std::fs::File::open(archive_path).map_err(|_| invalid("backup file cannot be opened"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|_| invalid("not a valid MyLore backup archive"))?;

    let staged_db = staging.join(DB_ENTRY);
    {
        let mut db_entry = archive
            .by_name(DB_ENTRY)
            .map_err(|_| invalid("backup archive has no database snapshot"))?;
        let mut out = std::fs::File::create(&staged_db)?;
        io::copy(&mut db_entry, &mut out)?;
    }
    std::fs::create_dir_all(staging.join(ASSETS_PREFIX))?;
    for entry in &meta.assets {
        let mut asset_entry = archive
            .by_name(&entry.file)
            .map_err(|_| invalid("backup archive is missing a manifest asset"))?;
        let mut out = std::fs::File::create(staging.join(&entry.file))?;
        io::copy(&mut asset_entry, &mut out)?;
    }
    Ok(staged_db)
}

/// The staged copy of a manifest asset (next to the staged snapshot).
fn staged_asset_path(staged_db: &Path, entry: &AssetManifestEntry) -> PathBuf {
    staged_db
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&entry.file)
}

/// Best-effort undo of a failed swap: remove anything swapped in, then move
/// every quarantined item back to its original place.
fn rollback(data_dir: &Path, quarantine: &Path) {
    tracing::warn!("restore failed, rolling back from quarantine");
    remove_any(data_dir.join("mylore.db"));
    for sidecar in ["-wal", "-shm"] {
        remove_any(data_dir.join(format!("mylore.db{sidecar}")));
    }
    let _ = std::fs::remove_dir_all(data_dir.join("images"));
    if let Ok(entries) = std::fs::read_dir(quarantine) {
        for entry in entries.flatten() {
            let dest = data_dir.join(entry.file_name());
            remove_any(&dest);
            let _ = std::fs::rename(entry.path(), dest);
        }
    }
    let _ = std::fs::remove_dir(quarantine);
}

/// Remove a file or directory, whichever it happens to be.
fn remove_any(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

/// Rename with a short retry loop — on Windows, SQLite's WAL/SHM handles can
/// linger briefly after a pool closes, and an immediate rename fails with
/// "file in use".
async fn rename_with_retry(source: &Path, dest: &Path) -> Result<(), AppError> {
    let mut last_error = None;
    for _ in 0..20 {
        match std::fs::rename(source, dest) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
    Err(AppError::Io(last_error.expect("at least one attempt")))
}

/// The zero-padded `YYYYMMDDHHMMSS` stamp embedded in an archive file name
/// (`mylore-<stamp>-<uid>.mylore`), or `None` for foreign names.
fn archive_stamp(file_name: &str) -> Option<String> {
    let rest = file_name.strip_prefix("mylore-")?.strip_suffix(".mylore")?;
    let mut parts = rest.split('-');
    let date = parts.next()?;
    let time = parts.next()?;
    if date.len() == 8
        && time.len() == 6
        && date.chars().all(|c| c.is_ascii_digit())
        && time.chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("{date}{time}"))
    } else {
        None
    }
}

/// Age in hours of the newest archive in `dir`, or `None` when there is none
/// (including when the directory does not exist yet).
fn newest_backup_age_hours(dir: &Path) -> Result<Option<f64>, AppError> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut newest: Option<std::time::SystemTime> = None;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".mylore") {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        newest = Some(match newest {
            Some(current) if current >= modified => current,
            _ => modified,
        });
    }
    Ok(newest.map(|modified| {
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        age.as_secs_f64() / 3600.0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        service: BackupService,
        pool: SqlitePool,
        db_path: PathBuf,
        data_dir: PathBuf,
    }

    async fn harness(name: &str) -> Harness {
        // Production layout: `{data_dir}/mylore.db`, so restore's file dance
        // runs against the real paths.
        let data_dir =
            std::env::temp_dir().join(format!("mylore-backup-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let db_path = data_dir.join("mylore.db");
        let pool = crate::infrastructure::db::init(&db_path)
            .await
            .expect("init");
        let service = BackupService::new(pool.clone(), &data_dir);
        Harness {
            service,
            pool,
            db_path,
            data_dir,
        }
    }

    impl Harness {
        async fn cleanup(self) {
            self.pool.close().await;
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }

    async fn seed_media(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn seed_cached_asset(pool: &SqlitePool, id: &str, local_path: &str) {
        sqlx::query(
            "INSERT INTO asset (id, kind, status, local_path, created_at)
             VALUES (?, 'cover', 'cached', ?, '2026-01-01')",
        )
        .bind(id)
        .bind(local_path)
        .execute(pool)
        .await
        .expect("seed asset");
    }

    #[tokio::test]
    async fn create_produces_a_valid_self_checked_archive() {
        let h = harness("roundtrip.db").await;
        seed_media(&h.pool, "m-1").await;
        let cache = h.data_dir.join("images").join("cache");
        std::fs::create_dir_all(&cache).expect("cache dir");
        let image_path = cache.join("a-1.jpg");
        std::fs::write(&image_path, b"jpeg-bytes").expect("write image");
        seed_cached_asset(&h.pool, "a-1", &image_path.display().to_string()).await;

        let report = h.service.create().await.expect("create backup");
        assert!(report.size_bytes > 0);
        assert_eq!(report.media_count, 1);
        assert_eq!(report.asset_count, 1);
        assert!(Path::new(&report.path).is_file());

        // The archive validates standalone and carries all three layers.
        let meta = h
            .service
            .validate(Path::new(&report.path))
            .await
            .expect("validate");
        assert_eq!(meta.format_version, FORMAT_VERSION);
        assert_eq!(meta.media_count, 1);
        assert_eq!(meta.asset_count, 1);
        assert_eq!(meta.assets[0].id, "a-1");
        assert!(meta.assets[0].file.starts_with("assets/a-1"));

        let file = std::fs::File::open(&report.path).expect("open archive");
        let archive = ZipArchive::new(file).expect("read archive");
        let names: Vec<String> = archive.file_names().map(str::to_string).collect();
        assert!(names.contains(&"meta.json".to_string()));
        assert!(names.contains(&"library.db".to_string()));
        assert!(names.iter().any(|n| n.starts_with("assets/a-1")));

        h.cleanup().await;
    }

    #[tokio::test]
    async fn validate_rejects_garbage_and_incomplete_archives() {
        let h = harness("reject.db").await;
        let dir = h.service.backups_dir();
        std::fs::create_dir_all(&dir).expect("backups dir");

        let garbage = dir.join("garbage.mylore");
        std::fs::write(&garbage, b"not a zip at all").expect("write garbage");
        assert!(h.service.validate(&garbage).await.is_err());

        // A well-formed zip without the database snapshot must fail too.
        let incomplete = dir.join("incomplete.mylore");
        let file = std::fs::File::create(&incomplete).expect("create file");
        let mut zip = ZipWriter::new(file);
        zip.start_file(
            META_ENTRY,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .expect("start meta entry");
        zip.write_all(b"{\"format_version\":1}")
            .expect("write meta");
        zip.finish().expect("finish zip");
        assert!(h.service.validate(&incomplete).await.is_err());

        h.cleanup().await;
    }

    #[tokio::test]
    async fn missing_asset_files_are_skipped_not_fatal() {
        let h = harness("missing_assets.db").await;
        seed_media(&h.pool, "m-1").await;
        seed_cached_asset(&h.pool, "a-gone", "Z:/definitely/missing/a.jpg").await;

        let report = h.service.create().await.expect("create backup");
        assert_eq!(report.media_count, 1);
        assert_eq!(report.asset_count, 0, "the missing file is skipped");
        assert!(h.service.validate(Path::new(&report.path)).await.is_ok());

        h.cleanup().await;
    }

    /// A fresh pool over the (possibly swapped) live database.
    async fn reopened(db_path: &Path) -> SqlitePool {
        crate::infrastructure::db::connect(db_path)
            .await
            .expect("reopen")
    }

    #[tokio::test]
    async fn restore_roundtrip_replaces_live_data_and_repoints_assets() {
        let h = harness("restore_roundtrip.db").await;
        seed_media(&h.pool, "m-1").await;
        let cache = h.data_dir.join("images").join("cache");
        std::fs::create_dir_all(&cache).expect("cache dir");
        let image_path = cache.join("a-1.jpg");
        std::fs::write(&image_path, b"jpeg-bytes").expect("write image");
        seed_cached_asset(&h.pool, "a-1", &image_path.display().to_string()).await;

        let backup = h.service.create().await.expect("create backup");

        // Mutate the live library after the backup: an extra title and a
        // deleted asset row must both disappear on restore.
        seed_media(&h.pool, "m-2").await;
        sqlx::query("DELETE FROM asset WHERE id = 'a-1'")
            .execute(&h.pool)
            .await
            .expect("delete asset");

        let report = h
            .service
            .restore(Path::new(&backup.path))
            .await
            .expect("restore");
        assert_eq!(report.media_count, 1);
        assert_eq!(report.asset_count, 1);
        assert!(report.restart_required);
        assert!(
            Path::new(&report.quarantined_to).is_dir(),
            "old data parked"
        );

        // The restored database holds exactly the backed-up state, and the
        // asset row points at the restored file under `images/`.
        let pool = reopened(&h.db_path).await;
        let (media,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(media, 1, "the post-backup m-2 is gone");
        let (local_path,): (String,) =
            sqlx::query_as("SELECT local_path FROM asset WHERE id = 'a-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        pool.close().await;
        assert!(
            Path::new(&local_path).is_file(),
            "asset repointed at {local_path}"
        );
        assert!(local_path.contains("images"));

        h.cleanup().await;
    }

    #[tokio::test]
    async fn restore_rejects_invalid_archives_without_touching_live_data() {
        let h = harness("restore_invalid.db").await;
        seed_media(&h.pool, "m-1").await;

        let garbage = h.service.backups_dir().join("garbage.mylore");
        std::fs::create_dir_all(h.service.backups_dir()).expect("backups dir");
        std::fs::write(&garbage, b"not a zip").expect("write garbage");

        assert!(h.service.restore(&garbage).await.is_err());

        let pool = reopened(&h.db_path).await;
        let (media,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        assert_eq!(media, 1, "live data untouched");
        let quarantines: Vec<_> = std::fs::read_dir(&h.data_dir)
            .expect("data dir")
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("quarantine-"))
            .collect();
        assert!(
            quarantines.is_empty(),
            "no quarantine created on early failure"
        );

        h.cleanup().await;
    }

    #[tokio::test]
    async fn rotate_keeps_newest_n_plus_one_per_older_month() {
        let h = harness("rotate.db").await;
        let dir = h.service.backups_dir();
        std::fs::create_dir_all(&dir).expect("backups dir");
        for name in [
            "mylore-20260810-120000-aaaaaa.mylore",
            "mylore-20260805-120000-bbbbbb.mylore",
            "mylore-20260801-120000-cccccc.mylore",
            "mylore-20260715-120000-dddddd.mylore",
            "mylore-20260701-120000-eeeeee.mylore",
            "mylore-20260620-120000-ffffff.mylore",
            "foreign-file.mylore",
        ] {
            std::fs::write(dir.join(name), b"x").expect("write archive stub");
        }

        let deleted = h.service.rotate(2).expect("rotate");
        assert_eq!(deleted, 2, "the older August + the older July");

        let remaining: Vec<String> = std::fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(remaining.contains(&"mylore-20260810-120000-aaaaaa.mylore".to_string()));
        assert!(remaining.contains(&"mylore-20260805-120000-bbbbbb.mylore".to_string()));
        assert!(remaining.contains(&"mylore-20260715-120000-dddddd.mylore".to_string()));
        assert!(remaining.contains(&"mylore-20260620-120000-ffffff.mylore".to_string()));
        assert_eq!(
            remaining.len(),
            5,
            "4 kept archives + the untouched foreign file"
        );

        h.cleanup().await;
    }

    #[tokio::test]
    async fn prefs_default_roundtrip_and_validation() {
        let h = harness("prefs.db").await;

        let defaults = h.service.prefs().await.expect("default prefs");
        assert!(!defaults.auto_enabled);
        assert_eq!(defaults.interval_hours, 24);
        assert_eq!(defaults.keep_count, 10);

        let stored = h
            .service
            .set_prefs(BackupPrefs {
                auto_enabled: true,
                interval_hours: 12,
                keep_count: 3,
            })
            .await
            .expect("set prefs");
        let loaded = h.service.prefs().await.expect("reload prefs");
        assert_eq!(loaded.auto_enabled, stored.auto_enabled);
        assert_eq!(loaded.interval_hours, 12);
        assert_eq!(loaded.keep_count, 3);

        for bad in [
            BackupPrefs {
                auto_enabled: true,
                interval_hours: 0,
                keep_count: 3,
            },
            BackupPrefs {
                auto_enabled: true,
                interval_hours: 9000,
                keep_count: 3,
            },
            BackupPrefs {
                auto_enabled: true,
                interval_hours: 12,
                keep_count: 0,
            },
            BackupPrefs {
                auto_enabled: true,
                interval_hours: 12,
                keep_count: 101,
            },
        ] {
            assert!(
                h.service.set_prefs(bad).await.is_err(),
                "must reject out of range"
            );
        }
        let unchanged = h.service.prefs().await.expect("prefs after rejects");
        assert_eq!(unchanged.interval_hours, 12);
        assert_eq!(unchanged.keep_count, 3);

        h.cleanup().await;
    }

    #[tokio::test]
    async fn auto_backup_runs_only_when_due() {
        let h = harness("auto_backup.db").await;

        // Disabled by default: never runs.
        assert!(h
            .service
            .auto_backup_if_due()
            .await
            .expect("check")
            .is_none());

        // Enabled with no archives at all: due immediately.
        h.service
            .set_prefs(BackupPrefs {
                auto_enabled: true,
                interval_hours: 24,
                keep_count: 5,
            })
            .await
            .expect("enable");
        let report = h
            .service
            .auto_backup_if_due()
            .await
            .expect("run")
            .expect("created");
        assert_eq!(report.media_count, 0);
        assert!(Path::new(&report.path).is_file());

        // A fresh archive means not due for another interval.
        assert!(h
            .service
            .auto_backup_if_due()
            .await
            .expect("check again")
            .is_none());

        h.cleanup().await;
    }

    #[tokio::test]
    async fn pre_migration_backup_runs_only_when_a_migration_is_pending() {
        let h = harness("pre_migration.db").await;
        seed_media(&h.pool, "m-1").await;

        // Fully migrated database (and a missing file): nothing to protect.
        assert!(BackupService::pre_migration_backup(&h.db_path)
            .await
            .expect("fully migrated")
            .is_none());
        let missing = h.data_dir.join("does-not-exist.db");
        assert!(BackupService::pre_migration_backup(&missing)
            .await
            .expect("fresh install")
            .is_none());

        // Simulate an older schema by forgetting the newest migration: the
        // hook must now snapshot the pre-migration database.
        sqlx::query(
            "DELETE FROM _sqlx_migrations \
             WHERE version = (SELECT MAX(version) FROM _sqlx_migrations)",
        )
        .execute(&h.pool)
        .await
        .expect("forget latest migration");

        let report = BackupService::pre_migration_backup(&h.db_path)
            .await
            .expect("pending migration")
            .expect("backup created");
        assert_eq!(report.media_count, 1);
        assert!(h.service.validate(Path::new(&report.path)).await.is_ok());

        h.cleanup().await;
    }

    #[tokio::test]
    async fn pre_migration_backup_surfaces_corrupt_databases() {
        let dir = std::env::temp_dir().join(format!("mylore-premig-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("data dir");
        let corrupt = dir.join("mylore.db");
        std::fs::write(&corrupt, b"garbage".repeat(512)).expect("write corrupt db");

        // A corrupt database cannot be snapshotted; the error surfaces so
        // startup can log it and decide to continue.
        assert!(BackupService::pre_migration_backup(&corrupt).await.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_returns_archives_newest_first_and_delete_is_path_safe() {
        let h = harness("list_delete.db").await;
        let dir = h.service.backups_dir();
        std::fs::create_dir_all(&dir).expect("backups dir");
        for name in [
            "mylore-20260801-100000-aaaaaa.mylore",
            "mylore-20260802-100000-bbbbbb.mylore",
            "not-an-archive.mylore",
        ] {
            std::fs::write(dir.join(name), b"x").expect("write stub");
        }

        let entries = h.service.list().expect("list");
        assert_eq!(
            entries
                .iter()
                .map(|e| e.file_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "mylore-20260802-100000-bbbbbb.mylore",
                "mylore-20260801-100000-aaaaaa.mylore",
            ],
            "newest first, foreign names ignored"
        );
        assert_eq!(entries[0].created_at, "20260802100000");

        // Deleting re-derives the path from the file name: a matching name
        // inside the backups folder works…
        let target = dir.join("mylore-20260801-100000-aaaaaa.mylore");
        h.service
            .delete_archive(&target.display().to_string())
            .expect("delete archive");
        assert!(!target.exists(), "archive removed");

        // …but the request is re-derived from the file name: deleting via any
        // other path removes the archive inside the backups folder and never
        // touches the outside copy…
        let outside = h.data_dir.join("mylore-20260802-100000-bbbbbb.mylore");
        std::fs::write(&outside, b"x").expect("write outside copy");
        h.service
            .delete_archive(&outside.display().to_string())
            .expect("re-derived delete");
        assert!(outside.exists(), "the outside file was not touched");
        assert!(!dir.join("mylore-20260802-100000-bbbbbb.mylore").exists());

        // …and names that are not archives are rejected outright.
        let foreign = h.data_dir.join("notes.txt");
        std::fs::write(&foreign, b"x").expect("write foreign");
        assert!(h
            .service
            .delete_archive(&foreign.display().to_string())
            .is_err());

        h.cleanup().await;
    }

    #[tokio::test]
    async fn start_fresh_quarantines_the_database() {
        let h = harness("start_fresh.db").await;
        seed_media(&h.pool, "m-1").await;

        let quarantined_to = h.service.start_fresh().await.expect("start fresh");
        let quarantine = PathBuf::from(&quarantined_to);
        assert!(quarantine.is_dir());
        assert!(
            quarantine.join("mylore.db").is_file(),
            "the database was parked"
        );
        assert!(
            !h.db_path.exists(),
            "the live path is free for a fresh start"
        );

        h.cleanup().await;
    }

    #[tokio::test]
    async fn restore_rolls_back_when_the_swap_fails() {
        let h = harness("restore_rollback.db").await;
        seed_media(&h.pool, "m-1").await;
        let cache = h.data_dir.join("images");
        std::fs::create_dir_all(&cache).expect("images dir");
        let image_path = cache.join("a-1.jpg");
        std::fs::write(&image_path, b"jpeg-bytes").expect("write image");
        seed_cached_asset(&h.pool, "a-1", &image_path.display().to_string()).await;
        let backup = h.service.create().await.expect("create backup");
        seed_media(&h.pool, "m-2").await;

        // Drive the guarded part of restore directly and sabotage it: the
        // staged asset file vanishes, so the swap fails *after* quarantine —
        // the dangerous window where live data has already been moved away.
        let meta = h
            .service
            .validate(Path::new(&backup.path))
            .await
            .expect("validate");
        let staging = h.data_dir.join(".restore-sabotage");
        std::fs::create_dir_all(&staging).expect("staging");
        let staged_db = extract_archive(Path::new(&backup.path), &meta, &staging).expect("extract");
        std::fs::remove_file(staging.join(&meta.assets[0].file)).expect("sabotage staged asset");

        h.pool.close().await;
        let quarantine = h.data_dir.join("quarantine-test");
        let result = h.service.swap_in(&staged_db, &meta, &quarantine).await;
        assert!(
            result.is_err(),
            "the missing staged asset must fail the swap"
        );

        // swap_in leaves the dangerous half-swapped state in place; restore()
        // owns the rollback. Drive it and verify the undo.
        rollback(&h.data_dir, &quarantine);

        assert!(!quarantine.exists(), "quarantine moved back");
        assert!(cache.join("a-1.jpg").is_file(), "original images restored");

        let pool = reopened(&h.db_path).await;
        let (media,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        assert_eq!(media, 2, "the mutated live data survived intact");

        let _ = std::fs::remove_dir_all(&staging);
        h.cleanup().await;
    }
}
