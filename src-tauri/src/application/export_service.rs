//! Export service (MISSION-071, ARCHITECTURE §6 / REQ-EXPORT-001).
//!
//! Streams the whole library to a user-chosen path as JSON (import-round-
//! trippable), CSV, or human-readable Markdown. Media rows are fetched one at a
//! time and written incrementally, so memory stays bounded regardless of
//! library size; a progress callback ticks `(done, total)` after every row so
//! the MISSION-070 task runner can stream status to the UI.
//!
//! The file is written to a `*.partial` sibling and renamed over the target
//! only on success — dropping the future (task cancellation) removes the
//! partial, so a cancelled export never leaves a half-written file at the
//! user's chosen path.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::domain::export::{
    render_markdown, row_to_csv, row_to_json, ExportExternalId, ExportFormat, ExportPerson,
    ExportRow, CSV_HEADERS,
};
use crate::error::AppError;
use crate::infrastructure::repositories::{asset, collection, media, review, tracking};

/// The typed result of a completed export (rides the task's `result`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportReport {
    pub format: String,
    pub total: i64,
    pub path: String,
}

/// A partial-file guard: writes go to `<final>.partial` and `commit` renames
/// it over the target; dropping without committing deletes the partial.
struct PartialExport {
    final_path: PathBuf,
    partial_path: PathBuf,
    committed: bool,
}

impl PartialExport {
    fn new(final_path: &Path) -> Result<Self, AppError> {
        let mut partial = final_path.to_path_buf();
        let name = partial
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "export".to_string());
        partial.set_file_name(format!("{name}.partial"));
        Ok(Self {
            final_path: final_path.to_path_buf(),
            partial_path: partial,
            committed: false,
        })
    }

    fn partial(&self) -> &Path {
        &self.partial_path
    }

    fn commit(mut self) -> Result<(), AppError> {
        std::fs::rename(&self.partial_path, &self.final_path)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for PartialExport {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.partial_path);
        }
    }
}

pub struct ExportService {
    pool: SqlitePool,
}

impl ExportService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Stream the library to `path` in `format`, ticking `on_progress(done,
    /// total)` after each row. Resolves with the export report once the file
    /// is renamed into place.
    pub async fn stream_to_path<F>(
        &self,
        path: &Path,
        format: ExportFormat,
        mut on_progress: F,
    ) -> Result<ExportReport, AppError>
    where
        F: FnMut(usize, usize),
    {
        let total = media::count(&self.pool, &media::MediaFilter::default()).await?;
        let ids = media::list_ids(&self.pool).await?;
        let collections = collection::media_collection_names(&self.pool).await?;
        let progress_total = total.max(1) as usize;

        let partial = PartialExport::new(path)?;
        let file = File::create(partial.partial())?;
        let mut writer = BufWriter::new(file);

        match format {
            ExportFormat::Json => {
                writer.write_all(b"[")?;
                for (index, id) in ids.iter().enumerate() {
                    let row = self.record(id, &collections).await?;
                    if index > 0 {
                        writer.write_all(b",\n")?;
                    }
                    serde_json::to_writer(&mut writer, &row_to_json(&row)?)?;
                    on_progress(index + 1, progress_total);
                }
                writer.write_all(b"]\n")?;
            }
            ExportFormat::Csv => {
                let mut csv_writer = csv::WriterBuilder::new()
                    .has_headers(false)
                    .from_writer(&mut writer);
                csv_writer
                    .write_record(CSV_HEADERS)
                    .map_err(std::io::Error::other)?;
                for (index, id) in ids.iter().enumerate() {
                    let row = self.record(id, &collections).await?;
                    csv_writer
                        .write_record(row_to_csv(&row))
                        .map_err(std::io::Error::other)?;
                    on_progress(index + 1, progress_total);
                }
                csv_writer.flush()?;
            }
            ExportFormat::Markdown => {
                writer.write_all(b"# MyLore library export\n\n")?;
                for (index, id) in ids.iter().enumerate() {
                    let row = self.record(id, &collections).await?;
                    writer.write_all(render_markdown(&row).as_bytes())?;
                    writer.write_all(b"\n\n---\n\n")?;
                    on_progress(index + 1, progress_total);
                }
            }
        }

        writer.flush()?;
        drop(writer);
        partial.commit()?;

        Ok(ExportReport {
            format: format.as_str().to_string(),
            total,
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// Assemble one flattened export row for a media id: the media aggregate,
    /// resolved link names, its asset urls, and the user data (tracking status,
    /// review, collections).
    async fn record(
        &self,
        id: &str,
        collections: &HashMap<String, Vec<String>>,
    ) -> Result<ExportRow, AppError> {
        let record = media::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::internal("media vanished during export"))?;

        let people = {
            let info = media::person_info(&self.pool, &record.people).await?;
            record
                .people
                .iter()
                .filter_map(|person_id| {
                    info.get(person_id).map(|(name, role)| ExportPerson {
                        role: role.clone(),
                        name: name.clone(),
                    })
                })
                .collect()
        };
        let genres = {
            let names = media::genre_names(&self.pool, &record.genres).await?;
            record
                .genres
                .iter()
                .filter_map(|genre_id| names.get(genre_id).cloned())
                .collect()
        };
        let tags = {
            let names = media::tag_info(&self.pool, &record.tags).await?;
            record
                .tags
                .iter()
                .filter_map(|tag_id| names.get(tag_id).map(|(name, _)| name.clone()))
                .collect()
        };

        let mut asset_ids = Vec::new();
        let mut asset_kinds: HashMap<String, &str> = HashMap::new();
        if let Some(cover) = &record.cover_asset_id {
            asset_kinds.insert(cover.clone(), "cover");
            asset_ids.push(cover.clone());
        }
        if let Some(banner) = &record.banner_asset_id {
            asset_kinds.insert(banner.clone(), "banner");
            asset_ids.push(banner.clone());
        }
        let mut cover_url = None;
        let mut banner_url = None;
        for asset in asset::list_by_ids(&self.pool, &asset_ids).await? {
            if let Some(url) = asset.remote_url {
                match asset_kinds.get(&asset.id).copied() {
                    Some("cover") => cover_url = Some(url),
                    Some("banner") => banner_url = Some(url),
                    _ => {}
                }
            }
        }

        let tracking = tracking::get_tracking(&self.pool, id).await?;
        let my_status = tracking.as_ref().map(|row| row.core_status.clone());
        let my_review = review::get(&self.pool, id).await?;

        Ok(ExportRow {
            title: record.title_main,
            title_original: record.title_original,
            alt_titles: record
                .alt_titles
                .iter()
                .map(|alt| alt.title.clone())
                .collect(),
            content_type: record.content_type,
            format: record.format,
            pub_status: record.pub_status,
            start_date: record.start_date,
            end_date: record.end_date,
            release_year: record.release_year,
            language: record.language,
            country: record.country,
            content_rating: record.content_rating,
            pages: record.pages,
            duration_min: record.duration_min,
            ep_count: record.ep_count,
            ch_count: record.ch_count,
            synopsis: record.synopsis,
            people,
            genres,
            tags,
            external_ids: record
                .external_ids
                .iter()
                .map(|id| ExportExternalId {
                    provider: id.provider.clone(),
                    value: id.ext_id.clone(),
                    url: id.url.clone(),
                })
                .collect(),
            cover_url,
            banner_url,
            my_status,
            my_rating: my_review.as_ref().and_then(|r| r.rating),
            my_review: my_review.as_ref().and_then(|r| r.review.clone()),
            my_short_review: my_review.as_ref().and_then(|r| r.short_review.clone()),
            my_notes: my_review.as_ref().and_then(|r| r.notes.clone()),
            progress: tracking.as_ref().and_then(|t| t.current_position),
            started_at: tracking.as_ref().and_then(|t| t.started_at.clone()),
            completed_at: tracking.as_ref().and_then(|t| t.finished_at.clone()),
            repeat_count: tracking.as_ref().map(|t| t.repeat_count).unwrap_or(0),
            favorite: my_review.as_ref().map(|r| r.favorite).unwrap_or(false),
            collections: collections.get(id).cloned().unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::export::CSV_HEADERS;
    use crate::infrastructure::repositories::collection::CollectionRecord;
    use crate::infrastructure::repositories::review::ReviewRecord;
    use crate::infrastructure::repositories::tracking::TrackingRecord;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};
    use std::fs;

    fn sample_media(id: &str, title: &str) -> media::MediaRecord {
        media::MediaRecord {
            id: id.to_string(),
            content_type: "novel".to_string(),
            format: Some("light_novel".to_string()),
            title_main: title.to_string(),
            title_original: Some("夜明けの剣".to_string()),
            synopsis: Some("A test synopsis".to_string()),
            pub_status: "ongoing".to_string(),
            start_date: Some("2025-01-01".to_string()),
            end_date: None,
            release_year: Some(2025),
            language: Some("ja".to_string()),
            country: Some("JP".to_string()),
            content_rating: None,
            pages: Some(320),
            duration_min: None,
            ep_count: None,
            ch_count: Some(12),
            cover_asset_id: None,
            banner_asset_id: None,
            provider: Some("anilist".to_string()),
            provider_url: None,
            metadata_refreshed_at: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-02".to_string(),
            alt_titles: vec![media::AltTitle {
                lang: "en".to_string(),
                title: "Dawn's Sword".to_string(),
            }],
            people: vec!["p-1".to_string()],
            genres: vec!["fantasy".to_string()],
            tags: vec!["isekai".to_string()],
            external_ids: vec![media::ExternalId {
                provider: "anilist".to_string(),
                ext_id: "42".to_string(),
                url: Some("https://anilist.co/anime/42".to_string()),
            }],
            relations: Vec::new(),
        }
    }

    async fn seed_library(pool: &SqlitePool) {
        // `person` is not seeded by migrations; `genre`/`tag` reference the
        // migration-0002 rows (`fantasy`, `isekai`).
        sqlx::query("INSERT INTO person (id, name, role) VALUES ('p-1', 'Jane', 'author')")
            .execute(pool)
            .await
            .expect("person");

        media::create(pool, &sample_media("m-1", "Sword of the Dawn"))
            .await
            .expect("create m1");
        let mut second = sample_media("m-2", "Berserk");
        // External ids are globally unique — give the second title its own.
        second.external_ids[0].ext_id = "99".to_string();
        media::create(pool, &second).await.expect("create m2");

        let tracking = TrackingRecord {
            media_id: "m-1".to_string(),
            core_status: "in_progress".to_string(),
            custom_status_id: None,
            started_at: Some("2026-01-01".to_string()),
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: None,
            auto_track: 1,
            updated_at: "2026-01-02".to_string(),
        };
        tracking::upsert_tracking(pool, &tracking)
            .await
            .expect("tracking");

        let review = ReviewRecord {
            media_id: "m-1".to_string(),
            rating: Some(8),
            review: Some("Lovely.".to_string()),
            short_review: None,
            notes: Some("read with tea".to_string()),
            favorite: true,
            is_spoiler: false,
            moods: vec![],
            pace: None,
            content_warnings: vec![],
            warnings_acknowledged_at: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-02".to_string(),
        };
        review::upsert(pool, &review).await.expect("review");

        collection::create(
            pool,
            &CollectionRecord {
                id: "c-1".to_string(),
                name: "Favorites shelf".to_string(),
                is_smart: false,
                filter_def: None,
                sort_order: 0,
                created_at: "2026-01-01".to_string(),
            },
        )
        .await
        .expect("collection");
        collection::add_member(pool, "c-1", "m-1", 0, "2026-01-01")
            .await
            .expect("member");
    }

    async fn seed_and_export(format: ExportFormat, db_name: &str) -> (PathBuf, ExportReport) {
        let (pool, _path) = migrated_pool(db_name).await;
        seed_library(&pool).await;

        let export_path = std::env::temp_dir().join(format!("mylore_export_test.{}.tmp", format));
        let _ = fs::remove_file(&export_path);
        let _ = fs::remove_file(
            export_path.with_file_name(format!("mylore_export_test.{}.tmp.partial", format)),
        );

        let service = ExportService::new(pool);
        let report = service
            .stream_to_path(&export_path, format, |_, _| {})
            .await
            .expect("stream");
        (export_path, report)
    }

    #[tokio::test]
    async fn json_export_writes_import_compatible_array() {
        let (path, report) = seed_and_export(ExportFormat::Json, "export_service_json.db").await;
        assert_eq!(report.format, "json");
        assert_eq!(report.total, 2);
        assert_eq!(report.path, path.to_string_lossy());

        let raw = fs::read_to_string(&path).expect("read");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let items = value.as_array().expect("array");
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first["title"], "Berserk");
        assert!(first["my_status"].is_null());
        assert_eq!(first["favorite"], false);

        let second = &items[1];
        assert_eq!(second["title"], "Sword of the Dawn");
        assert_eq!(second["alt_titles"], serde_json::json!(["Dawn's Sword"]));
        assert_eq!(second["people"][0]["name"], "Jane");
        assert_eq!(second["genres"], serde_json::json!(["Fantasy"]));
        assert_eq!(second["external_ids"][0]["value"], "42");
        assert_eq!(second["my_status"], "in_progress");
        assert_eq!(second["my_rating"], 8);
        assert_eq!(second["my_review"], "Lovely.");
        assert_eq!(second["favorite"], true);
        assert_eq!(
            second["collections"],
            serde_json::json!(["Favorites shelf"])
        );

        fs::remove_file(&path).expect("cleanup");
    }

    #[tokio::test]
    async fn csv_export_has_header_then_records() {
        let (path, report) = seed_and_export(ExportFormat::Csv, "export_service_csv.db").await;
        assert_eq!(report.total, 2);

        let raw = fs::read_to_string(&path).expect("read");
        let mut reader = csv::Reader::from_reader(raw.as_bytes());
        let headers = reader.headers().expect("headers");
        assert_eq!(headers.iter().collect::<Vec<_>>(), CSV_HEADERS);

        let mut rows = reader.records();
        let first = rows.next().expect("row1").expect("record");
        assert_eq!(first.get(0), Some("Berserk"));
        assert_eq!(first.get(25), Some(""));
        assert_eq!(first.get(33), Some("0"));
        assert_eq!(first.get(34), Some("false"));

        let second = rows.next().expect("row2").expect("record");
        assert_eq!(second.get(0), Some("Sword of the Dawn"));
        assert_eq!(second.get(2), Some("Dawn's Sword"));
        assert_eq!(second.get(17), Some("Jane"));
        assert_eq!(second.get(20), Some("Fantasy"));
        assert_eq!(second.get(22), Some("anilist:42"));
        assert_eq!(second.get(25), Some("in_progress"));
        assert_eq!(second.get(26), Some("8"));
        assert_eq!(second.get(31), Some("2026-01-01"));
        assert_eq!(second.get(34), Some("true"));
        assert_eq!(second.get(35), Some("Favorites shelf"));
        assert!(rows.next().is_none());

        fs::remove_file(&path).expect("cleanup");
    }

    #[tokio::test]
    async fn markdown_export_renders_each_title() {
        let (path, report) = seed_and_export(ExportFormat::Markdown, "export_service_md.db").await;
        assert_eq!(report.total, 2);

        let raw = fs::read_to_string(&path).expect("read");
        assert!(raw.starts_with("# MyLore library export\n\n"));
        assert!(raw.contains("# Sword of the Dawn (夜明けの剣)"));
        assert!(raw.contains("**Author:** Jane"));
        assert!(raw.contains("**My data:** Status: in_progress · My rating: 8/10 · started 2026-01-01 · Favorite · Collections: Favorites shelf"));
        assert!(raw.contains("# Berserk"));
        assert!(raw.contains("\n---\n\n"));

        fs::remove_file(&path).expect("cleanup");
    }

    #[tokio::test]
    async fn empty_library_exports_valid_empty_files() {
        let (pool, _path) = migrated_pool("export_service_empty_1.db").await;
        let export_path = std::env::temp_dir().join("mylore_export_empty.tmp");
        let _ = fs::remove_file(&export_path);

        let service = ExportService::new(pool);
        let report = service
            .stream_to_path(&export_path, ExportFormat::Json, |_, _| {})
            .await
            .expect("stream");
        assert_eq!(report.total, 0);
        assert_eq!(fs::read_to_string(&export_path).expect("read"), "[]\n");

        fs::remove_file(&export_path).expect("cleanup");
    }

    #[tokio::test]
    async fn progress_ticks_per_row() {
        let (pool, _path) = migrated_pool("export_service_progress_1.db").await;
        seed_library(&pool).await;
        let export_path = std::env::temp_dir().join("mylore_export_progress.tmp");
        let _ = fs::remove_file(&export_path);

        let service = ExportService::new(pool);
        let ticks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let ticker = ticks.clone();
        service
            .stream_to_path(&export_path, ExportFormat::Csv, move |done, total| {
                ticker.lock().unwrap().push((done, total));
            })
            .await
            .expect("stream");

        assert_eq!(*ticks.lock().unwrap(), vec![(1, 2), (2, 2)]);

        fs::remove_file(&export_path).expect("cleanup");
        cleanup_files(&export_path);
    }

    #[tokio::test]
    async fn no_partial_file_left_on_error() {
        let (pool, _path) = migrated_pool("export_service_partial_1.db").await;
        seed_library(&pool).await;
        let export_path = std::env::temp_dir().join("mylore_export_partial.tmp");

        // A target path whose parent does not exist → the write fails before
        // anything is created, and no partial remains.
        let missing_parent = export_path
            .parent()
            .unwrap()
            .join("does-not-exist")
            .join("out.json");
        let service = ExportService::new(pool);
        let result = service
            .stream_to_path(&missing_parent, ExportFormat::Json, |_, _| {})
            .await;
        assert!(result.is_err());
        assert!(!missing_parent.exists());

        cleanup_files(&export_path);
    }
}
