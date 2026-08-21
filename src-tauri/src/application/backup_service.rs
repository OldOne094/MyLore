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
            let _ = std::fs::remove_file(path);
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

        let size_bytes = std::fs::metadata(&dest)?.len();
        Ok(BackupReport {
            path: dest.display().to_string(),
            size_bytes,
            media_count: meta.media_count,
            asset_count: meta.asset_count,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    struct Harness {
        service: BackupService,
        pool: SqlitePool,
        db_path: PathBuf,
        data_dir: PathBuf,
    }

    async fn harness(name: &str) -> Harness {
        let (pool, db_path) = migrated_pool(name).await;
        let data_dir =
            std::env::temp_dir().join(format!("mylore-backup-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(data_dir.join("images")).expect("images dir");
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
            let _ = std::fs::remove_dir_all(&self.data_dir);
            self.pool.close().await;
            cleanup_files(&self.db_path);
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
}
