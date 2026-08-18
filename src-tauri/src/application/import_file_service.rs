//! File-import use-cases (MISSION-068, MISSION-072): the **MyLore JSON format**,
//! **CSV with a column mapping**, and the **AniList / Goodreads / StoryGraph**
//! profile exports, all on top of the MISSION-067 pipeline.
//!
//! The service picks the right `ImportParser` for the requested `ImportFileKind`
//! and hands off to `ImportPipeline` — preview (parse → dedup → per-item
//! outcomes) and commit (one transaction, savepoint per row, per-item report).
//! `commit` with a `None` plan imports every `New` row of the preview (the
//! "just import the new titles" default the mapping UI uses).

use sqlx::SqlitePool;

use crate::application::import_pipeline::import_error_to_app;
use crate::application::import_pipeline::ImportPipeline;
use crate::domain::import::{ImportParser, ImportPlan, ImportPreview, ImportReport};
use crate::error::AppError;
use crate::infrastructure::parsers::shared::sniff_csv_columns;
use crate::infrastructure::parsers::{
    csv_headers, AniListParser, CsvMapping, CsvParser, GoodreadsParser, JsonParser,
    StorygraphParser,
};

/// Which file format to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFileKind {
    /// MyLore's own JSON export (a top-level array of item objects).
    Json,
    /// Any CSV with a user-supplied column mapping.
    Csv,
    /// The AniList user export (media list collection JSON).
    AniList,
    /// The Goodreads library CSV export.
    Goodreads,
    /// The StoryGraph CSV export.
    Storygraph,
}

impl ImportFileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::AniList => "anilist",
            Self::Goodreads => "goodreads",
            Self::Storygraph => "storygraph",
        }
    }

    /// A profile export (no column mapping, built-in user state).
    pub fn is_profile(self) -> bool {
        matches!(self, Self::AniList | Self::Goodreads | Self::Storygraph)
    }

    /// Sniff a file's format from its content. JSON files split into the
    /// MyLore array vs the AniList collection export; CSV files into the
    /// Goodreads / StoryGraph exports vs a generic mapped CSV.
    pub fn detect(source: &str) -> Result<Self, AppError> {
        let trimmed = source.trim_start().trim_start_matches('\u{feff}');
        if trimmed.starts_with('[') {
            return Ok(Self::Json);
        }
        if trimmed.starts_with('{') {
            return Ok(if is_anilist_export(trimmed) {
                Self::AniList
            } else {
                Self::Json
            });
        }

        let headers = sniff_csv_columns(source, b',').map_err(import_error_to_app)?;
        let contains = |names: &[&str]| {
            names
                .iter()
                .all(|name| headers.iter().any(|header| header == name))
        };
        if contains(&["book id", "title", "author"]) {
            Ok(Self::Goodreads)
        } else if contains(&["reading status", "title"]) {
            Ok(Self::Storygraph)
        } else {
            Ok(Self::Csv)
        }
    }
}

/// True when the JSON object is the AniList export shape.
fn is_anilist_export(source: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(source)
        .map(|value| value.get("mediaListCollection").is_some())
        .unwrap_or(false)
}

impl std::str::FromStr for ImportFileKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "anilist" => Ok(Self::AniList),
            "goodreads" => Ok(Self::Goodreads),
            "storygraph" => Ok(Self::Storygraph),
            _ => Err(AppError::validation(format!(
                "unknown import format: {value:?}"
            ))),
        }
    }
}

/// File-import use-cases over the shared pipeline.
pub struct ImportFileService {
    pipeline: ImportPipeline,
}

impl ImportFileService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pipeline: ImportPipeline::new(pool),
        }
    }

    /// Detect which import kind a file is (for the frontend's auto-routing).
    pub fn detect(&self, source: &str) -> Result<ImportFileKind, AppError> {
        ImportFileKind::detect(source)
    }

    /// Parse + dedup a file into a preview. Read-only.
    pub async fn preview(
        &self,
        kind: ImportFileKind,
        source: &str,
        mapping: Option<&CsvMapping>,
    ) -> Result<ImportPreview, AppError> {
        let parser = Self::parser(kind, mapping)?;
        self.pipeline.preview(parser.as_ref(), source).await
    }

    /// Preview then commit the plan's rows in one transaction. `None` plan →
    /// import every `New` row.
    pub async fn commit(
        &self,
        kind: ImportFileKind,
        source: &str,
        mapping: Option<&CsvMapping>,
        plan: Option<&ImportPlan>,
    ) -> Result<ImportReport, AppError> {
        let preview = self.preview(kind, source, mapping).await?;
        let plan = match plan {
            Some(plan) => plan.clone(),
            None => ImportPlan::all_new(&preview),
        };
        self.pipeline.commit(&preview, &plan).await
    }

    /// Read just the header row of a CSV (for the mapping UI).
    pub fn headers(&self, source: &str, delimiter: &str) -> Result<Vec<String>, AppError> {
        csv_headers(source, delimiter).map_err(import_error_to_app)
    }

    fn parser(
        kind: ImportFileKind,
        mapping: Option<&CsvMapping>,
    ) -> Result<Box<dyn ImportParser>, AppError> {
        match kind {
            ImportFileKind::Json => Ok(Box::new(JsonParser)),
            ImportFileKind::AniList => Ok(Box::new(AniListParser)),
            ImportFileKind::Goodreads => Ok(Box::new(GoodreadsParser)),
            ImportFileKind::Storygraph => Ok(Box::new(StorygraphParser)),
            ImportFileKind::Csv => {
                let mapping = mapping
                    .ok_or_else(|| AppError::validation("CSV import requires a column mapping"))?;
                Ok(Box::new(CsvParser::new(mapping.clone())))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::import::{RowOutcome, RowStatus};
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn csv_source() -> &'static str {
        "Title,Author,Genres,Pages\n\
         Sword of the Dawn,Test Author,\"Fantasy, Adventure\",320\n\
         Berserk,Kentaro Miura,Seinen,380"
    }

    fn csv_mapping() -> CsvMapping {
        CsvMapping {
            title: Some("Title".to_string()),
            author: Some("Author".to_string()),
            genres: Some("Genres".to_string()),
            pages: Some("Pages".to_string()),
            default_content_type: Some("novel".to_string()),
            ..CsvMapping::default()
        }
    }

    #[tokio::test]
    async fn detect_sniffs_formats() {
        assert_eq!(
            ImportFileKind::detect("[\n { \"title\": \"Sword\" } ]").expect("json"),
            ImportFileKind::Json
        );
        assert_eq!(
            ImportFileKind::detect("  {\"title\":\"Sword\"}").expect("json"),
            ImportFileKind::Json
        );
        assert_eq!(
            ImportFileKind::detect(
                r#"{"mediaListCollection":{"lists":[{"name":"Anime","entries":[]}]}}"#
            )
            .expect("anilist"),
            ImportFileKind::AniList
        );
        assert_eq!(
            ImportFileKind::detect("Title,Author,Genres,Pages\nSword,Jane,\"Fantasy\",320")
                .expect("csv"),
            ImportFileKind::Csv
        );
        assert_eq!(
            ImportFileKind::detect(
                "\"Book Id\",\"Title\",\"Author\",\"My Rating\",\"Exclusive Shelf\"\n1,Sword,Jane,4,read"
            )
            .expect("goodreads"),
            ImportFileKind::Goodreads
        );
        assert_eq!(
            ImportFileKind::detect(
                "Title,Author,Reading Status,Page Count,Page Number Read\nSword,Jane,Read,320,320"
            )
            .expect("storygraph"),
            ImportFileKind::Storygraph
        );
    }

    #[tokio::test]
    async fn detect_prefers_title_author_shape_for_goodreads() {
        let source = "\"Book Id\",\"Title\",\"Author\"\n1,Sword,Jane\n";
        assert_eq!(
            ImportFileKind::detect(source).expect("kind"),
            ImportFileKind::Goodreads
        );
    }

    #[tokio::test]
    async fn profile_commit_imports_media_and_user_state() {
        let (pool, path) = migrated_pool("import_file_goodreads.db").await;
        let service = ImportFileService::new(pool.clone());
        let source = "\"Book Id\",\"Title\",\"Author\",\"My Rating\",\"Date Read\",\
            \"Exclusive Shelf\",\"My Review\"\n\
            1,\"Sword of the Dawn\",\"Jane Doe\",\"4\",\"2026/01/05\",\"read\",\"Lovely.\"\n";

        let report = service
            .commit(ImportFileKind::Goodreads, source, None, None)
            .await
            .expect("commit");
        assert_eq!(report.committed, 1);
        let media_id = report.items[0].media_id.clone().unwrap();

        let tracking = sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT core_status, current_position FROM tracking WHERE media_id = ?",
        )
        .bind(&media_id)
        .fetch_one(&pool)
        .await
        .expect("tracking");
        assert_eq!(tracking.0, "completed");
        assert_eq!(tracking.1, None);

        let review = sqlx::query_as::<_, (Option<i64>, String)>(
            "SELECT rating, review FROM review WHERE media_id = ?",
        )
        .bind(&media_id)
        .fetch_one(&pool)
        .await
        .expect("review");
        assert_eq!(review.0, Some(8));
        assert_eq!(review.1, "Lovely.");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn profile_rows_without_state_import_plain_metadata() {
        let (pool, path) = migrated_pool("import_file_anilist.db").await;
        let service = ImportFileService::new(pool.clone());
        let source = r#"{"mediaListCollection":{"lists":[{"entries":[
            {"media":{"id":1,"idMal":2,"title":{"romaji":"Sword of the Dawn"},
             "type":"MANGA","format":"NOVEL","startDate":{"year":2025},
             "chapters":40,"genres":["Fantasy"]}}
        ]}]}}"#;

        let report = service
            .commit(ImportFileKind::AniList, source, None, None)
            .await
            .expect("commit");
        assert_eq!(report.committed, 1);
        let media_id = report.items[0].media_id.clone().unwrap();

        let content_type =
            sqlx::query_scalar::<_, String>("SELECT content_type FROM media WHERE id = ?")
                .bind(&media_id)
                .fetch_one(&pool)
                .await
                .expect("content type");
        assert_eq!(content_type, "novel");

        let tracking_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tracking WHERE media_id = ?")
                .bind(&media_id)
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(tracking_count, 0, "no user state → no tracking row");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn json_preview_marks_rows_new_in_an_empty_library() {
        let (pool, path) = migrated_pool("import_file_json_preview.db").await;
        let service = ImportFileService::new(pool.clone());
        let source = r#"[{"title":"Sword of the Dawn","content_type":"novel","pages":320},{"title":"Berserk","content_type":"manga"}]"#;

        let preview = service
            .preview(ImportFileKind::Json, source, None)
            .await
            .expect("preview");
        assert_eq!(preview.total, 2);
        assert_eq!(preview.new, 2);
        assert_eq!(preview.items[0].outcome, RowOutcome::New);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn csv_preview_needs_a_mapping() {
        let (pool, path) = migrated_pool("import_file_csv_no_mapping.db").await;
        let service = ImportFileService::new(pool.clone());

        let error = service
            .preview(ImportFileKind::Csv, csv_source(), None)
            .await
            .expect_err("no mapping");
        assert!(error.to_string().contains("column mapping"));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn csv_commit_imports_all_new_rows_by_default() {
        let (pool, path) = migrated_pool("import_file_csv_commit.db").await;
        let service = ImportFileService::new(pool.clone());

        let report = service
            .commit(
                ImportFileKind::Csv,
                csv_source(),
                Some(&csv_mapping()),
                None,
            )
            .await
            .expect("commit");
        assert_eq!(report.committed, 2);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.failed, 0);

        let titles =
            sqlx::query_as::<_, (String,)>("SELECT title_main FROM media ORDER BY title_main")
                .fetch_all(&pool)
                .await
                .expect("titles")
                .into_iter()
                .map(|(title,)| title)
                .collect::<Vec<_>>();
        assert_eq!(titles, vec!["Berserk", "Sword of the Dawn"]);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn json_commit_dedups_existing_rows_as_skipped() {
        let (pool, path) = migrated_pool("import_file_json_commit.db").await;
        let service = ImportFileService::new(pool.clone());

        let source = r#"[{"title":"Sword of the Dawn","content_type":"novel","pages":320}]"#;
        let first = service
            .commit(ImportFileKind::Json, source, None, None)
            .await
            .expect("first import");
        assert_eq!(first.committed, 1);

        let second = service
            .commit(ImportFileKind::Json, source, None, None)
            .await
            .expect("second import");
        assert_eq!(second.committed, 0, "already in library → skipped");
        assert_eq!(second.skipped, 1);
        assert_eq!(second.items[0].status, RowStatus::Skipped);
        assert_eq!(second.items[0].media_id, first.items[0].media_id);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn headers_reads_the_first_row() {
        let (pool, path) = migrated_pool("import_file_headers.db").await;
        let service = ImportFileService::new(pool.clone());
        assert_eq!(
            service.headers(csv_source(), ",").expect("headers"),
            vec!["Title", "Author", "Genres", "Pages"]
        );
        assert_eq!(
            service.headers(csv_source(), "\\t").expect("headers"),
            vec!["Title,Author,Genres,Pages"],
            "wrong delimiter merges the row into one column"
        );

        pool.close().await;
        cleanup_files(&path);
    }
}
