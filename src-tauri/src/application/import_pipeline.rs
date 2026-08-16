//! Import pipeline service (MISSION-067, ARCHITECTURE §6 / REQ-IMPORT-002).
//!
//! Orchestrates the pipeline's database-facing stages on top of the pure core
//! in `domain::import`:
//!
//!   - `preview` — parse (via an `ImportParser`) → validate → normalize → dedup
//!     → preview. Read-only; nothing is written.
//!   - `commit` — writes the confirmed `New` rows in **one transaction** with a
//!     savepoint per row, so a single failing row rolls back only itself and is
//!     reported; the batch never partially fails and never aborts silently.
//!   - `run` — chains both, for callers that import immediately.
//!
//! Per-item outcomes map to the report: `Committed` (created), `Skipped`
//! (invalid / already in library / duplicate / not selected), `Failed` (the
//! savepoint rolled back). The media aggregate (row + people/genre/tag links,
//! external ids, cover/banner assets) is built here and persisted through the
//! transaction-aware repository helpers.

use std::collections::HashSet;

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::domain::import::{
    self, ImportError, ImportParser, ImportPlan, ImportPreview, ImportReport, ImportRow,
    PreviewItem, ReportItem, RowOutcome, RowStatus,
};
use crate::error::AppError;
use crate::infrastructure::repositories::asset as asset_repo;
use crate::infrastructure::repositories::asset::AssetRecord;
use crate::infrastructure::repositories::media as media_repo;
use crate::infrastructure::repositories::media::{AltTitle, ExternalId, MediaRecord};

/// Import pipeline use-cases.
pub struct ImportPipeline {
    pool: SqlitePool,
}

impl ImportPipeline {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse → validate → normalize → dedup → preview. Writes nothing; the
    /// preview is what the confirm UI (MISSION-069) shows before a commit.
    pub async fn preview(
        &self,
        parser: &dyn ImportParser,
        source: &str,
    ) -> Result<ImportPreview, AppError> {
        let items = parser.parse(source).map_err(import_error_to_app)?;
        let candidates = media_repo::identity_candidates(&self.pool).await?;
        Ok(import::preview(&items, &candidates))
    }

    /// Commit the plan's `New` rows in one transaction, savepoint per row.
    /// Non-`New` rows (invalid / in-library / duplicate / not selected) are
    /// reported as skipped; a row that fails to insert rolls back to its own
    /// savepoint and is reported as failed.
    pub async fn commit(
        &self,
        preview: &ImportPreview,
        plan: &ImportPlan,
    ) -> Result<ImportReport, AppError> {
        let now = Utc::now().to_rfc3339();
        let plan_rows: HashSet<usize> = plan.rows.iter().copied().collect();
        let preview_rows: HashSet<usize> = preview.items.iter().map(|i| i.source_row).collect();

        let mut items: Vec<ReportItem> = Vec::with_capacity(preview.items.len());
        let mut committed = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;

        // Rows to write: `New` preview items the plan selected.
        let mut writes: Vec<(&PreviewItem, &ImportRow)> = Vec::new();
        for item in &preview.items {
            match item.outcome {
                RowOutcome::Invalid => {
                    skipped += 1;
                    items.push(ReportItem {
                        source_row: item.source_row,
                        title: item.title.clone().unwrap_or_default(),
                        status: RowStatus::Skipped,
                        media_id: None,
                        message: Some("invalid row".to_string()),
                    });
                }
                RowOutcome::InLibrary => {
                    skipped += 1;
                    items.push(ReportItem {
                        source_row: item.source_row,
                        title: item.title.clone().unwrap_or_default(),
                        status: RowStatus::Skipped,
                        media_id: item.matched_media_id.clone(),
                        message: Some("already in library".to_string()),
                    });
                }
                RowOutcome::Duplicate => {
                    skipped += 1;
                    items.push(ReportItem {
                        source_row: item.source_row,
                        title: item.title.clone().unwrap_or_default(),
                        status: RowStatus::Skipped,
                        media_id: item.matched_media_id.clone(),
                        message: Some("duplicate".to_string()),
                    });
                }
                RowOutcome::New => {
                    if plan_rows.contains(&item.source_row) {
                        if let Some(row) = &item.row {
                            writes.push((item, row));
                        }
                        continue;
                    }
                    skipped += 1;
                    items.push(ReportItem {
                        source_row: item.source_row,
                        title: item.title.clone().unwrap_or_default(),
                        status: RowStatus::Skipped,
                        media_id: None,
                        message: Some("not selected".to_string()),
                    });
                }
            }
        }

        if !writes.is_empty() {
            let mut tx = self.pool.begin().await?;
            for (item, row) in &writes {
                sqlx::query("SAVEPOINT import_row")
                    .execute(&mut *tx)
                    .await?;
                match insert_row(&mut tx, row, &now).await {
                    Ok(media_id) => {
                        sqlx::query("RELEASE import_row").execute(&mut *tx).await?;
                        committed += 1;
                        items.push(ReportItem {
                            source_row: item.source_row,
                            title: item.title.clone().unwrap_or_default(),
                            status: RowStatus::Committed,
                            media_id: Some(media_id),
                            message: None,
                        });
                    }
                    Err(error) => {
                        sqlx::query("ROLLBACK TO import_row")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("RELEASE import_row").execute(&mut *tx).await?;
                        failed += 1;
                        items.push(ReportItem {
                            source_row: item.source_row,
                            title: item.title.clone().unwrap_or_default(),
                            status: RowStatus::Failed,
                            media_id: None,
                            message: Some(error.to_string()),
                        });
                    }
                }
            }
            tx.commit().await?;
        }

        // Plan rows that reference nothing in the preview are reported skipped.
        let mut missing_rows: Vec<usize> = plan
            .rows
            .iter()
            .copied()
            .filter(|row| !preview_rows.contains(row))
            .collect();
        missing_rows.sort_unstable();
        for row in missing_rows {
            skipped += 1;
            items.push(ReportItem {
                source_row: row,
                title: String::new(),
                status: RowStatus::Skipped,
                media_id: None,
                message: Some("unknown source row".to_string()),
            });
        }

        items.sort_by_key(|item| item.source_row);

        Ok(ImportReport {
            total: preview.total,
            committed,
            skipped,
            failed,
            items,
        })
    }

    /// Preview then commit with the given plan, for callers that import
    /// immediately rather than showing the confirm UI first.
    pub async fn run(
        &self,
        parser: &dyn ImportParser,
        source: &str,
        plan: &ImportPlan,
    ) -> Result<ImportReport, AppError> {
        let preview = self.preview(parser, source).await?;
        self.commit(&preview, plan).await
    }
}

fn import_error_to_app(error: ImportError) -> AppError {
    match error {
        ImportError::Parse(message) => AppError::validation(message),
        ImportError::Unsupported(message) => AppError::internal(message),
    }
}

/// Insert one canonical row inside the caller's transaction, resolving people,
/// genres, tags and cover/banner assets, then the media aggregate itself.
async fn insert_row<'e>(
    tx: &mut sqlx::Transaction<'e, sqlx::Sqlite>,
    row: &ImportRow,
    now: &str,
) -> Result<String, AppError> {
    let id = format!("m-{}", Uuid::new_v4());

    let mut people = Vec::with_capacity(row.people.len());
    for (role, name) in &row.people {
        people.push(media_repo::ensure_person_in_tx(tx, name, role.as_str()).await?);
    }
    let mut genres = Vec::with_capacity(row.genres.len());
    for genre in &row.genres {
        genres.push(media_repo::ensure_genre_in_tx(tx, genre).await?);
    }
    let mut tags = Vec::with_capacity(row.tags.len());
    for tag in &row.tags {
        tags.push(media_repo::ensure_domain_tag_in_tx(tx, tag).await?);
    }

    let cover_asset_id = insert_asset_in_tx(tx, "cover", row.cover_url.as_deref(), now).await?;
    let banner_asset_id = insert_asset_in_tx(tx, "banner", row.banner_url.as_deref(), now).await?;

    let record = MediaRecord {
        id: id.clone(),
        content_type: row.content_type.as_str().to_string(),
        format: row.format.clone(),
        title_main: row.title.main().to_string(),
        title_original: row.title.original().map(str::to_string),
        synopsis: row.synopsis.clone(),
        pub_status: row.pub_status.as_str().to_string(),
        start_date: row.start_date.clone(),
        end_date: row.end_date.clone(),
        release_year: row.release_year,
        language: row.language.clone(),
        country: row.country.clone(),
        content_rating: row.content_rating.clone(),
        pages: row.pages,
        duration_min: row.duration_min,
        ep_count: row.ep_count,
        ch_count: row.ch_count,
        cover_asset_id,
        banner_asset_id,
        provider: None,
        provider_url: None,
        metadata_refreshed_at: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        alt_titles: row
            .title
            .alternatives()
            .iter()
            .map(|title| AltTitle {
                lang: String::new(),
                title: title.clone(),
            })
            .collect(),
        people,
        genres,
        tags,
        external_ids: row
            .external_ids
            .iter()
            .map(|id| ExternalId {
                provider: id.provider().as_str().to_string(),
                ext_id: id.value().to_string(),
                url: id.url().map(str::to_string),
            })
            .collect(),
        relations: Vec::new(),
    };

    media_repo::insert_in_tx(tx, &record).await?;
    Ok(id)
}

/// Register a cover/banner URL as a `remote` asset inside the transaction,
/// resolving with its id; `None` URLs register nothing (MISSION-062).
async fn insert_asset_in_tx<'e>(
    tx: &mut sqlx::Transaction<'e, sqlx::Sqlite>,
    kind: &str,
    remote_url: Option<&str>,
    now: &str,
) -> Result<Option<String>, AppError> {
    let Some(url) = remote_url else {
        return Ok(None);
    };
    let id = format!("a-{}", Uuid::new_v4());
    asset_repo::insert_in_tx(
        tx,
        &AssetRecord {
            id: id.clone(),
            kind: kind.to_string(),
            remote_url: Some(url.to_string()),
            local_path: None,
            status: "remote".to_string(),
            mime_type: None,
            width: None,
            height: None,
            etag: None,
            last_fetched_at: None,
            created_at: now.to_string(),
        },
    )
    .await?;
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::domain::import::ParsedItem;
    use crate::infrastructure::repositories::media as media_repo;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    #[derive(Clone)]
    enum ParserBehavior {
        Ok(Vec<ParsedItem>),
        Err(ImportError),
    }

    struct FakeParser {
        behavior: Mutex<ParserBehavior>,
    }

    impl ImportParser for FakeParser {
        fn parse(&self, _source: &str) -> Result<Vec<ParsedItem>, ImportError> {
            match self.behavior.lock().unwrap().clone() {
                ParserBehavior::Ok(items) => Ok(items),
                ParserBehavior::Err(error) => Err(error),
            }
        }
    }

    fn parser(items: Vec<ParsedItem>) -> FakeParser {
        FakeParser {
            behavior: Mutex::new(ParserBehavior::Ok(items)),
        }
    }

    fn failing_parser() -> FakeParser {
        FakeParser {
            behavior: Mutex::new(ParserBehavior::Err(ImportError::Parse("boom".to_string()))),
        }
    }

    fn item(row: usize, title: &str) -> ParsedItem {
        ParsedItem {
            row,
            title: title.to_string(),
            title_original: None,
            alt_titles: Vec::new(),
            content_type: Some("novel".to_string()),
            format: Some("light_novel".to_string()),
            pub_status: Some("ongoing".to_string()),
            start_date: Some("2025-01-01".to_string()),
            end_date: None,
            release_year: Some("2025".to_string()),
            language: Some("ja".to_string()),
            country: Some("JP".to_string()),
            content_rating: None,
            pages: Some("320".to_string()),
            duration_min: None,
            ep_count: None,
            ch_count: None,
            synopsis: Some("A synopsis.".to_string()),
            people: vec![("author".to_string(), "Test Author".to_string())],
            genres: vec!["Fantasy".to_string()],
            tags: vec!["isekai".to_string()],
            external_ids: Vec::new(),
            cover_url: None,
            banner_url: None,
        }
    }

    async fn library_titles(pool: &SqlitePool) -> Vec<String> {
        sqlx::query_as::<_, (String,)>("SELECT title_main FROM media ORDER BY title_main")
            .fetch_all(pool)
            .await
            .expect("titles")
            .into_iter()
            .map(|(title,)| title)
            .collect()
    }

    fn sample_media(id: &str, title: &str) -> MediaRecord {
        MediaRecord {
            id: id.to_string(),
            content_type: "novel".to_string(),
            format: Some("light_novel".to_string()),
            title_main: title.to_string(),
            title_original: None,
            synopsis: None,
            pub_status: "ongoing".to_string(),
            start_date: None,
            end_date: None,
            release_year: None,
            language: None,
            country: None,
            content_rating: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count: None,
            cover_asset_id: None,
            banner_asset_id: None,
            provider: Some("anilist".to_string()),
            provider_url: None,
            metadata_refreshed_at: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
            alt_titles: Vec::new(),
            people: Vec::new(),
            genres: Vec::new(),
            tags: Vec::new(),
            external_ids: Vec::new(),
            relations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn preview_marks_new_rows_when_library_is_empty() {
        let (pool, path) = migrated_pool("import_pipeline_preview_empty.db").await;
        let service = ImportPipeline::new(pool.clone());
        let parser = parser(vec![item(1, "Sword of the Dawn"), item(2, "Berserk")]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        assert_eq!(preview.total, 2);
        assert_eq!(preview.valid, 2);
        assert_eq!(preview.new, 2);
        assert_eq!(preview.invalid, 0);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn preview_resolves_in_library_and_duplicate_from_library() {
        let (pool, path) = migrated_pool("import_pipeline_preview_match.db").await;
        let mut existing = sample_media("m-1", "Sword of the Dawn");
        existing.external_ids.push(media_repo::ExternalId {
            provider: "anilist".to_string(),
            ext_id: "42".to_string(),
            url: None,
        });
        media_repo::create(&pool, &existing)
            .await
            .expect("seed library");

        let service = ImportPipeline::new(pool.clone());
        let mut duplicate = item(2, "Sword of the Dawn");
        duplicate.external_ids = vec![("anilist".to_string(), "999".to_string(), None)];
        let mut in_library = item(3, "Sword of the Dawn");
        in_library.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let parser = parser(vec![item(1, "Berserk"), duplicate, in_library]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        assert_eq!(preview.new, 1);
        assert_eq!(preview.duplicates, 1);
        assert_eq!(preview.in_library, 1);
        assert_eq!(preview.items[1].outcome, RowOutcome::Duplicate);
        assert_eq!(preview.items[1].matched_media_id.as_deref(), Some("m-1"));
        assert_eq!(preview.items[2].outcome, RowOutcome::InLibrary);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn commit_writes_planned_rows_and_skips_others() {
        let (pool, path) = migrated_pool("import_pipeline_commit.db").await;
        let service = ImportPipeline::new(pool.clone());
        let parser = parser(vec![item(1, "Alpha"), item(2, "Beta"), item(3, "Gamma")]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        let report = service
            .commit(&preview, &ImportPlan { rows: vec![1, 3] })
            .await
            .expect("commit");

        assert_eq!(report.committed, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.total, 3);
        assert_eq!(library_titles(&pool).await, vec!["Alpha", "Gamma"]);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn commit_links_people_genres_tags_assets_and_external_ids() {
        let (pool, path) = migrated_pool("import_pipeline_commit_links.db").await;
        let service = ImportPipeline::new(pool.clone());

        let mut full = item(1, "Sword of the Dawn");
        full.cover_url = Some("https://cdn.example/cover.jpg".to_string());
        full.banner_url = Some("https://cdn.example/banner.jpg".to_string());
        full.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let parser = parser(vec![full]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        let report = service
            .commit(&preview, &ImportPlan::all_new(&preview))
            .await
            .expect("commit");
        assert_eq!(report.committed, 1);

        let stored = media_repo::get(&pool, &report.items[0].media_id.clone().unwrap())
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(stored.title_main, "Sword of the Dawn");
        assert_eq!(stored.content_type, "novel");
        assert_eq!(stored.pub_status, "ongoing");
        assert_eq!(stored.start_date.as_deref(), Some("2025-01-01"));
        assert_eq!(stored.pages, Some(320));
        assert_eq!(
            stored.genres,
            vec!["fantasy".to_string()],
            "seed genre reused"
        );
        assert_eq!(stored.tags, vec!["isekai".to_string()]);
        assert_eq!(stored.external_ids.len(), 1);
        assert_eq!(stored.external_ids[0].ext_id, "42");
        assert_eq!(
            stored.provider, None,
            "file imports carry no metadata provider"
        );

        let cover = stored.cover_asset_id.expect("cover asset linked");
        let banner = stored.banner_asset_id.expect("banner asset linked");
        assert_ne!(cover, banner);
        let cover_row = asset_repo::get(&pool, &cover).await.expect("get").unwrap();
        assert_eq!(cover_row.kind, "cover");
        assert_eq!(
            cover_row.remote_url.as_deref(),
            Some("https://cdn.example/cover.jpg")
        );
        assert_eq!(cover_row.status, "remote");

        let people = sqlx::query_as::<_, (String, String, String)>(
            "SELECT p.id, p.name, p.role FROM person p JOIN media_person mp \
             ON mp.person_id = p.id WHERE mp.media_id = ?",
        )
        .bind(&report.items[0].media_id.clone().unwrap())
        .fetch_all(&pool)
        .await
        .expect("people");
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].1, "Test Author");
        assert_eq!(people[0].2, "author");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn commit_rolls_back_only_the_failed_row() {
        let (pool, path) = migrated_pool("import_pipeline_savepoint.db").await;
        let service = ImportPipeline::new(pool.clone());

        // Both rows carry the same (provider, ext_id): the second insert must
        // violate the global media_external_id PK and roll back its savepoint.
        let mut first = item(1, "Alpha");
        first.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let mut second = item(2, "Beta");
        second.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let parser = parser(vec![first, second]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        assert_eq!(preview.new, 2, "no library overlap, so both rows look new");
        let report = service
            .commit(&preview, &ImportPlan::all_new(&preview))
            .await
            .expect("commit");

        assert_eq!(report.committed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(
            library_titles(&pool).await,
            vec!["Alpha"],
            "Beta rolled back"
        );
        let failed = report
            .items
            .iter()
            .find(|item| item.status == RowStatus::Failed)
            .expect("failed row reported");
        assert_eq!(failed.source_row, 2);
        assert!(failed.message.is_some());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn commit_with_empty_plan_skips_everything() {
        let (pool, path) = migrated_pool("import_pipeline_commit_none.db").await;
        let service = ImportPipeline::new(pool.clone());
        let parser = parser(vec![item(1, "Alpha"), item(2, "Beta")]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        let report = service
            .commit(&preview, &ImportPlan::none())
            .await
            .expect("commit");

        assert_eq!(report.committed, 0);
        assert_eq!(report.skipped, 2);
        assert!(library_titles(&pool).await.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn commit_reports_skipped_non_new_rows() {
        let (pool, path) = migrated_pool("import_pipeline_commit_skip.db").await;
        let mut existing = sample_media("m-1", "Sword of the Dawn");
        existing.external_ids.push(media_repo::ExternalId {
            provider: "anilist".to_string(),
            ext_id: "42".to_string(),
            url: None,
        });
        media_repo::create(&pool, &existing)
            .await
            .expect("seed library");

        let service = ImportPipeline::new(pool.clone());
        let mut dup = item(2, "Sword of the Dawn");
        dup.external_ids = vec![("anilist".to_string(), "999".to_string(), None)];
        let mut in_library = item(3, "Sword of the Dawn");
        in_library.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let mut invalid = item(4, " ");
        invalid.content_type = None;
        let parser = parser(vec![item(1, "Alpha"), dup, in_library, invalid]);

        let preview = service.preview(&parser, "source").await.expect("preview");
        let report = service
            .commit(&preview, &ImportPlan::all_new(&preview))
            .await
            .expect("commit");

        assert_eq!(report.committed, 1);
        assert_eq!(report.skipped, 3);
        assert_eq!(report.failed, 0);
        assert_eq!(
            library_titles(&pool).await,
            vec!["Alpha", "Sword of the Dawn"]
        );

        let messages: Vec<&str> = report
            .items
            .iter()
            .filter(|item| item.status == RowStatus::Skipped)
            .filter_map(|item| item.message.as_deref())
            .collect();
        assert!(messages.contains(&"invalid row"));
        assert!(messages.contains(&"already in library"));
        assert!(messages.contains(&"duplicate"));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn run_chains_preview_and_commit() {
        let (pool, path) = migrated_pool("import_pipeline_run.db").await;
        let service = ImportPipeline::new(pool.clone());
        let parser = parser(vec![item(1, "Alpha"), item(2, "Beta")]);

        let report = service
            .run(&parser, "source", &ImportPlan { rows: vec![1] })
            .await
            .expect("run");
        assert_eq!(report.committed, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(library_titles(&pool).await, vec!["Alpha"]);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn preview_parse_error_propagates() {
        let (pool, path) = migrated_pool("import_pipeline_parse_error.db").await;
        let service = ImportPipeline::new(pool.clone());

        let err = service
            .preview(&failing_parser(), "source")
            .await
            .expect_err("parse error");
        assert!(err.to_string().contains("boom"));

        pool.close().await;
        cleanup_files(&path);
    }
}
