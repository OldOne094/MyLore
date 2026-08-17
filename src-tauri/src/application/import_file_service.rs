//! File-import use-cases (MISSION-068): the **MyLore JSON format** and
//! **CSV with a column mapping**, both on top of the MISSION-067 pipeline.
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
use crate::infrastructure::parsers::{csv_headers, CsvMapping, CsvParser, JsonParser};

/// Which file format to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFileKind {
    Json,
    Csv,
}

impl ImportFileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
        }
    }

    /// Sniff a file's format from its first meaningful byte.
    pub fn detect(source: &str) -> Self {
        let trimmed = source.trim_start();
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            Self::Json
        } else {
            Self::Csv
        }
    }
}

impl std::str::FromStr for ImportFileKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
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
    async fn detect_format_sniffs_json_vs_csv() {
        assert_eq!(
            ImportFileKind::detect("[\n { \"title\": \"Sword\" } ]"),
            ImportFileKind::Json
        );
        assert_eq!(
            ImportFileKind::detect("  {\"title\":\"Sword\"}"),
            ImportFileKind::Json
        );
        assert_eq!(
            ImportFileKind::detect("Title,Author\nSword,Jane"),
            ImportFileKind::Csv
        );
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
