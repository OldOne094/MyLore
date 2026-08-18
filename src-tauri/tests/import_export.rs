//! Import/export integration tests (MISSION-073).
//!
//! Drives the *real* pipeline — parsers → `ImportFileService` → `ImportPipeline`
//! commit → repositories — against a migrated file-backed database, using the
//! sample files in `tests/fixtures/import/`. Covers:
//!
//!   1. every import kind (MyLore JSON, AniList export, Goodreads CSV,
//!      StoryGraph CSV) detected + imported end-to-end with the profile user
//!      state persisted as tracking/review rows;
//!   2. the MISSION-071 JSON export **round-tripping** back through the
//!      MISSION-067 importer (titles + user state preserved, re-import dedups);
//!   3. CSV and Markdown exports containing the imported library.

use mylore_lib::application::export_service::ExportService;
use mylore_lib::application::import_file_service::{ImportFileKind, ImportFileService};
use mylore_lib::domain::export::ExportFormat;
use mylore_lib::infrastructure::db;
use mylore_lib::infrastructure::repositories::review as review_repo;
use mylore_lib::infrastructure::repositories::tracking as tracking_repo;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// A fully-migrated file-backed pool with a unique, cleaned-up path.
async fn migrated_pool(name: &str) -> (SqlitePool, PathBuf) {
    let dir = std::env::temp_dir().join(format!("mylore-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    cleanup_files(&path);
    let pool = db::init(&path).await.expect("init migrated database");
    (pool, path)
}

fn cleanup_files(path: &Path) {
    let base = path.display().to_string();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{base}{suffix}"));
    }
}

/// Read a sample file under `tests/fixtures/import/`.
fn fixture_import(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/import/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("import fixture file exists")
}

async fn count_media(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM media")
        .fetch_one(pool)
        .await
        .expect("count media")
}

async fn media_id(pool: &SqlitePool, title: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT id FROM media WHERE title_main = ?")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("media row")
}

/// Export the library to a unique temp path and return it.
async fn export_library(pool: SqlitePool, format: ExportFormat, name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mylore_it_{}_{}.tmp", std::process::id(), name));
    cleanup_files(&path);
    ExportService::new(pool)
        .stream_to_path(&path, format, |_, _| {})
        .await
        .expect("export");
    path
}

async fn read_export(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read export")
}

#[tokio::test]
async fn detects_and_imports_every_fixture_kind() {
    let cases = [
        ("mylore.json", ImportFileKind::Json, 2),
        ("anilist_export.json", ImportFileKind::AniList, 2),
        ("goodreads_export.csv", ImportFileKind::Goodreads, 3),
        ("storygraph_export.csv", ImportFileKind::Storygraph, 4),
    ];

    for (file, kind, expected) in cases {
        let db_name = format!("it_import_{}.db", kind.as_str());
        let (pool, path) = migrated_pool(&db_name).await;
        let service = ImportFileService::new(pool.clone());
        let source = fixture_import(file);

        assert_eq!(service.detect(&source).expect("detect"), kind, "{file}");
        let preview = service.preview(kind, &source, None).await.expect("preview");
        assert_eq!(preview.total, expected, "{file}");
        assert_eq!(preview.new, expected, "{file}");
        assert_eq!(preview.invalid, 0, "{file}");

        let report = service
            .commit(kind, &source, None, None)
            .await
            .expect("commit");
        assert_eq!(report.committed, expected, "{file}");
        assert_eq!(report.failed, 0, "{file}");
        assert_eq!(count_media(&pool).await, expected as i64, "{file}");

        pool.close().await;
        cleanup_files(&path);
    }
}

#[tokio::test]
async fn profile_fixtures_persist_tracking_and_review_state() {
    let (pool, path) = migrated_pool("it_profile_state.db").await;
    let service = ImportFileService::new(pool.clone());

    let source = fixture_import("goodreads_export.csv");
    service
        .commit(ImportFileKind::Goodreads, &source, None, None)
        .await
        .expect("goodreads commit");

    let name_of_wind = media_id(&pool, "The Name of the Wind").await;
    let sword = media_id(&pool, "Sword of the Dawn").await;

    // "currently-reading" → in_progress, no finish date; ISBN13 persisted.
    let tracking = tracking_repo::get_tracking(&pool, &sword)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "in_progress");
    assert_eq!(tracking.finished_at, None);
    let isbn = sqlx::query_scalar::<_, String>(
        "SELECT ext_id FROM media_external_id WHERE media_id = ? AND provider = 'isbn'",
    )
    .bind(&name_of_wind)
    .fetch_one(&pool)
    .await
    .expect("isbn external id");
    assert_eq!(isbn, "9780756404741");

    // 4/5 → 8/10, My Review persisted.
    let review = review_repo::get(&pool, &sword)
        .await
        .expect("review")
        .expect("review row");
    assert_eq!(review.rating, Some(8));
    assert_eq!(review.review.as_deref(), Some("Lovely."));

    // A "to-read" title gets a tracking row with no dates, no review.
    let silmarillion = media_id(&pool, "The Silmarillion").await;
    let tracking = tracking_repo::get_tracking(&pool, &silmarillion)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "planned");
    assert!(review_repo::get(&pool, &silmarillion)
        .await
        .expect("review")
        .is_none());

    pool.close().await;
    cleanup_files(&path);
}

#[tokio::test]
async fn anilist_fixture_maps_media_and_user_state() {
    let (pool, path) = migrated_pool("it_anilist.db").await;
    let service = ImportFileService::new(pool.clone());

    let source = fixture_import("anilist_export.json");
    service
        .commit(ImportFileKind::AniList, &source, None, None)
        .await
        .expect("anilist commit");

    let fma = media_id(&pool, "Fullmetal Alchemist: Brotherhood").await;
    let content_type =
        sqlx::query_scalar::<_, String>("SELECT content_type FROM media WHERE id = ?")
            .bind(&fma)
            .fetch_one(&pool)
            .await
            .expect("content type");
    assert_eq!(content_type, "anime");
    let ep_count = sqlx::query_scalar::<_, Option<i64>>("SELECT ep_count FROM media WHERE id = ?")
        .bind(&fma)
        .fetch_one(&pool)
        .await
        .expect("ep count");
    assert_eq!(ep_count, Some(64));

    // COMPLETED + repeat=1 → Repeat status per the tracking invariant; the
    // finish date is dropped for the non-terminal Repeat bucket (and the entry
    // progress is kept).
    let tracking = tracking_repo::get_tracking(&pool, &fma)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "repeat");
    assert_eq!(tracking.repeat_count, 1);
    assert_eq!(tracking.current_position, Some(64));
    assert_eq!(
        tracking.finished_at, None,
        "non-terminal Repeat drops the finish date"
    );

    // 100/10 → 10.
    let review = review_repo::get(&pool, &fma)
        .await
        .expect("review")
        .expect("review");
    assert_eq!(review.rating, Some(10));
    assert_eq!(review.review.as_deref(), Some("A masterpiece."));

    // CURRENT + score 0 → in_progress with no rating; anilist external id set.
    let oshi = media_id(&pool, "Oshi no Ko").await;
    let tracking = tracking_repo::get_tracking(&pool, &oshi)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "in_progress");
    assert_eq!(tracking.current_position, Some(7));
    assert!(review_repo::get(&pool, &oshi)
        .await
        .expect("review")
        .is_none());
    let anilist_id = sqlx::query_scalar::<_, String>(
        "SELECT ext_id FROM media_external_id WHERE media_id = ? AND provider = 'anilist'",
    )
    .bind(&oshi)
    .fetch_one(&pool)
    .await
    .expect("anilist external id");
    assert_eq!(anilist_id, "300");

    pool.close().await;
    cleanup_files(&path);
}

#[tokio::test]
async fn json_export_round_trips_through_the_importer() {
    // Seed library A from the MyLore fixture, then export it as JSON.
    let (pool_a, path_a) = migrated_pool("it_roundtrip_a.db").await;
    let service_a = ImportFileService::new(pool_a.clone());
    let source_a = fixture_import("mylore.json");
    service_a
        .commit(ImportFileKind::Json, &source_a, None, None)
        .await
        .expect("seed import");
    let export_path = export_library(pool_a.clone(), ExportFormat::Json, "roundtrip").await;
    let exported = read_export(&export_path).await;

    let value: serde_json::Value = serde_json::from_str(&exported).expect("export is valid JSON");
    let items = value.as_array().expect("export is an array");
    assert_eq!(items.len(), 2);
    assert!(
        items[0]["my_status"].is_string(),
        "user state rides the export"
    );

    // Import the export into a fresh library B — titles *and* user state.
    let (pool_b, path_b) = migrated_pool("it_roundtrip_b.db").await;
    let service_b = ImportFileService::new(pool_b.clone());
    let report = service_b
        .commit(ImportFileKind::Json, &exported, None, None)
        .await
        .expect("re-import");
    assert_eq!(report.committed, 2);
    assert_eq!(count_media(&pool_b).await, 2);

    let sword = media_id(&pool_b, "Sword of the Dawn").await;
    let tracking = tracking_repo::get_tracking(&pool_b, &sword)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "in_progress");
    assert_eq!(tracking.current_position, Some(120));
    assert_eq!(tracking.started_at.as_deref(), Some("2026-01-05"));
    let review = review_repo::get(&pool_b, &sword)
        .await
        .expect("review")
        .expect("review row");
    assert_eq!(review.rating, Some(8));
    assert_eq!(review.review.as_deref(), Some("Lovely."));

    let berserk = media_id(&pool_b, "Berserk").await;
    let tracking = tracking_repo::get_tracking(&pool_b, &berserk)
        .await
        .expect("tracking")
        .expect("tracking row");
    assert_eq!(tracking.core_status, "completed");
    assert_eq!(tracking.finished_at.as_deref(), Some("2026-08-17"));

    // Importing the same export again dedups to skips — nothing is duplicated.
    let again = service_b
        .commit(ImportFileKind::Json, &exported, None, None)
        .await
        .expect("re-import again");
    assert_eq!(again.committed, 0);
    assert_eq!(again.skipped, 2);
    assert_eq!(count_media(&pool_b).await, 2, "library unchanged");

    pool_a.close().await;
    pool_b.close().await;
    cleanup_files(&path_a);
    cleanup_files(&path_b);
    cleanup_files(&export_path);
}

#[tokio::test]
async fn csv_and_markdown_exports_include_the_imported_library() {
    let (pool, path) = migrated_pool("it_exports.db").await;
    let service = ImportFileService::new(pool.clone());
    let source = fixture_import("goodreads_export.csv");
    service
        .commit(ImportFileKind::Goodreads, &source, None, None)
        .await
        .expect("goodreads commit");

    let csv_path = export_library(pool.clone(), ExportFormat::Csv, "library_csv").await;
    let csv = read_export(&csv_path).await;
    assert!(
        csv.lines()
            .next()
            .unwrap()
            .starts_with("title,title_original"),
        "fixed CSV header, first line: {:?}",
        csv.lines().next().unwrap()
    );
    assert!(
        csv.contains("The Name of the Wind"),
        "CSV has the imported titles"
    );
    assert!(
        csv.contains("9780756404741"),
        "CSV carries the isbn external id"
    );

    let md_path = export_library(pool.clone(), ExportFormat::Markdown, "library_md").await;
    let md = read_export(&md_path).await;
    assert!(
        md.contains("# The Name of the Wind"),
        "Markdown has a per-title section"
    );
    assert!(md.contains("book"), "Markdown lists the content type");

    cleanup_files(&csv_path);
    cleanup_files(&md_path);
    pool.close().await;
    cleanup_files(&path);
}
