//! Backup & restore integration tests (MISSION-096).
//!
//! Exercises the full `.mylore` lifecycle through the public application
//! services against real migrated databases — the same path production code
//! takes:
//!   1. build a library (media, nodes, tracking, review, collection, cached
//!      cover asset),
//!   2. create a validated backup,
//!   3. heavily mutate the live library,
//!   4. restore the archive and assert the exact pre-mutation world returns,
//!   5. re-back-up and confirm the retention policy holds.

use mylore_lib::application::backup_service::BackupService;
use mylore_lib::application::media_service::{AddMediaInput, MediaService};
use mylore_lib::infrastructure::db;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn temp_data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mylore-integration-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create data dir");
    dir
}

async fn open_database(data_dir: &Path) -> SqlitePool {
    let db_path = data_dir.join("mylore.db");
    db::init(&db_path).await.expect("init database")
}

struct Library {
    pool: SqlitePool,
    service: MediaService,
}

async fn seed_library(data_dir: &Path) -> Library {
    let pool = open_database(data_dir).await;
    let service = MediaService::new(pool.clone());
    let id = service
        .add_media(AddMediaInput {
            title: "Integration Target".into(),
            content_type: "novel".into(),
            format: Some("light_novel".into()),
            pub_status: Some("ongoing".into()),
            synopsis: Some("Before the storm.".into()),
            release_year: Some(2026),
            language: Some("ja".into()),
            country: None,
            pages: Some(300),
            duration_min: None,
            ep_count: None,
            ch_count: None,
            genres: vec!["fantasy".into()],
        })
        .await
        .expect("add media")
        .to_string();

    // Content nodes so restore has something to re-parent.
    sqlx::query(
        "INSERT INTO content_node (id, media_id, kind, position, created_at) \
         VALUES ('n-1', ?, 'chapter', 1, '2026-01-01')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .expect("seed node");

    // A cached cover asset with a real file on disk.
    let images = data_dir.join("images").join("covers");
    std::fs::create_dir_all(&images).expect("images dir");
    let cover = images.join("cover-a.jpg");
    std::fs::write(&cover, b"cover-bytes").expect("write cover");
    sqlx::query(
        "INSERT INTO asset (id, kind, status, local_path, mime_type, created_at) \
         VALUES ('a-cover', 'cover', 'cached', ?, 'image/jpeg', '2026-01-01')",
    )
    .bind(cover.display().to_string())
    .execute(&pool)
    .await
    .expect("seed asset");
    sqlx::query("UPDATE media SET cover_asset_id = 'a-cover' WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await
        .expect("link cover");

    // Tracking + review + a collection membership.
    sqlx::query("INSERT INTO tracking (media_id, core_status, updated_at) VALUES (?, 'in_progress', '2026-01-01')")
        .bind(&id)
        .execute(&pool)
        .await
        .expect("seed tracking");
    sqlx::query(
        "INSERT INTO review (media_id, rating, favorite, created_at, updated_at) \
         VALUES (?, 9, 1, '2026-01-02', '2026-01-02')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .expect("seed review");
    sqlx::query(
        "INSERT INTO collection (id, name, created_at) VALUES ('c-1', 'Favourites', '2026-01-01')",
    )
    .execute(&pool)
    .await
    .expect("seed collection");
    sqlx::query("INSERT INTO collection_member (collection_id, media_id, added_at) VALUES ('c-1', ?, '2026-01-03')")
        .bind(&id)
        .execute(&pool)
        .await
        .expect("seed membership");

    Library { pool, service }
}

#[tokio::test]
async fn restore_brings_back_the_exact_pre_mutation_world() {
    let data_dir = temp_data_dir("lifecycle");
    let library = seed_library(&data_dir).await;
    let backups = BackupService::new(library.pool.clone(), &data_dir);

    // 1. Snapshot the healthy library.
    let report = backups.create().await.expect("create backup");
    assert_eq!(report.media_count, 1);
    assert_eq!(report.asset_count, 1);

    // 2. Mutate hard: new title, deleted node, rewritten review, dropped
    //    membership.
    library
        .service
        .add_media(AddMediaInput {
            title: "Post Backup Addition".into(),
            content_type: "anime".into(),
            format: None,
            pub_status: Some("ongoing".into()),
            synopsis: None,
            release_year: None,
            language: None,
            country: None,
            pages: None,
            duration_min: None,
            ep_count: Some(12),
            ch_count: None,
            genres: vec![],
        })
        .await
        .expect("add second media");
    sqlx::query("DELETE FROM content_node WHERE id = 'n-1'")
        .execute(&library.pool)
        .await
        .expect("delete node");
    let survivor: String =
        sqlx::query_as("SELECT id FROM media WHERE title_main = 'Integration Target'")
            .fetch_one(&library.pool)
            .await
            .expect("find survivor");
    sqlx::query("UPDATE review SET rating = 3 WHERE media_id = ?")
        .bind(&survivor)
        .execute(&library.pool)
        .await
        .expect("rewrite review");
    sqlx::query("DELETE FROM collection_member WHERE media_id = ?")
        .bind(&survivor)
        .execute(&library.pool)
        .await
        .expect("drop membership");

    // 3. Restore — the mutated world is replaced by the archived one.
    let restore = backups.restore(Path::new(&report.path)).await.expect("restore");
    assert!(restore.restart_required);
    assert!(Path::new(&restore.quarantined_to).is_dir());

    let check = db::connect(&data_dir.join("mylore.db")).await.expect("reopen");
    let (media_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM media").fetch_one(&check).await.unwrap();
    assert_eq!(media_count, 1, "the post-backup addition is gone");

    let (nodes,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM content_node WHERE media_id = ? AND id = 'n-1'",
    )
    .bind(&survivor)
    .fetch_one(&check)
    .await
    .unwrap();
    assert_eq!(nodes, 1, "the node came back on the survivor");

    let (rating,): (i64,) = sqlx::query_as("SELECT rating FROM review WHERE media_id = ?")
        .bind(&survivor)
        .fetch_one(&check)
        .await
        .unwrap();
    assert_eq!(rating, 9, "the pre-merge review returned");

    let (memberships,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM collection_member WHERE media_id = ?")
            .bind(&survivor)
            .fetch_one(&check)
            .await
            .unwrap();
    assert_eq!(memberships, 1, "the membership returned");

    let (cover_path,): (String,) =
        sqlx::query_as("SELECT local_path FROM asset WHERE id = 'a-cover'")
            .fetch_one(&check)
            .await
            .unwrap();
    assert!(
        Path::new(&cover_path).is_file(),
        "asset repointed at its restored file"
    );
    check.close().await;

    // 4. The quarantined mutated database still exists for forensics.
    let quarantine_db = Path::new(&restore.quarantined_to).join("mylore.db");
    assert!(quarantine_db.is_file());

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn retention_keeps_newest_n_plus_monthly_across_sessions() {
    let data_dir = temp_data_dir("retention");
    let pool = open_database(&data_dir).await;
    let backups = BackupService::new(pool.clone(), &data_dir);

    let dir = backups.backups_dir();
    std::fs::create_dir_all(&dir).expect("backups dir");
    for name in [
        "mylore-20260820-120000-aaaaaa.mylore",
        "mylore-20260821-120000-bbbbbb.mylore",
        "mylore-20260822-120000-cccccc.mylore",
        "mylore-20260715-120000-dddddd.mylore",
        "mylore-20260701-120000-eeeeee.mylore",
    ] {
        std::fs::write(dir.join(name), b"stub").expect("write stub");
    }

    let deleted = backups.rotate(2).expect("rotate");
    assert_eq!(deleted, 2, "the older August pair goes; July keeps one");
    let remaining: Vec<String> = std::fs::read_dir(&dir)
        .expect("read dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(remaining.len(), 3);

    pool.close().await;
    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn tampered_archives_are_caught_by_validation() {
    let data_dir = temp_data_dir("tamper");
    let pool = open_database(&data_dir).await;
    let backups = BackupService::new(pool.clone(), &data_dir);

    let report = backups.create().await.expect("create backup");
    let path = PathBuf::from(&report.path);

    // Flip bytes in the middle of the snapshot region: the manifest parses,
    // but SQLite must reject the corrupted database page.
    let mut bytes = std::fs::read(&path).expect("read archive");
    let mid = bytes.len() / 2;
    for byte in &mut bytes[mid..mid + 64] {
        *byte ^= 0xFF;
    }
    std::fs::write(&path, &bytes).expect("write tampered archive");

    assert!(backups.validate(&path).await.is_err(), "tampering detected");

    pool.close().await;
    let _ = std::fs::remove_dir_all(&data_dir);
}
