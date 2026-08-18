//! Import pipeline core (MISSION-067, ARCHITECTURE §6 / REQ-IMPORT-002).
//!
//! Pure and side-effect-free: the stages an imported row walks through are
//! implemented here — **parser** (the `ImportParser` seam; the format parsers
//! themselves land in MISSION-068) → **validator** (`validate`) → **normalizer**
//! (`normalize`) → **deduplicator** (`dedup`, on top of `identity`) → **preview**
//! (`preview`). The transaction + report stages live in the application service
//! (`application::import_pipeline`) because they touch the database.
//!
//! A row flows as `ParsedItem` (raw, stringly-typed — what any file parser or
//! column mapping produces) → `ImportRow` (the validated canonical domain row
//! that dedup and the transaction consume). Validation policy:
//!
//!   - **Error** (row skipped): blank title, missing/unrecognized content type,
//!     a count/year that is not a non-negative integer. These are structural —
//!     silently importing them would create corrupt rows.
//!   - **Warning** (row imported, field degraded): unrecognized pub status
//!     (defaults to `unknown`), malformed dates or language (dropped), unknown
//!     person roles or blank/duplicate external ids (dropped). Enrichment data
//!     degrades gracefully so real-world files (Goodreads/StoryGraph exports)
//!     import instead of failing.

use std::collections::HashSet;
use std::str::FromStr;

use crate::domain::enums::{ContentType, CoreStatus, MediaStatus, PersonRole};
use crate::domain::identity::{self, IdentityCandidate, IdentityKind};
use crate::domain::value_objects::{
    DateOnly, ExternalId, LanguageCode, MediaId, ProviderId, Title,
};

/// How severe an issue is. Errors make a row invalid; warnings are carried
/// through and surfaced on the preview/report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// A field-level problem found on a parsed row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub field: String,
    pub message: String,
}

/// A whole-file parse failure (as opposed to per-row issues).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("unsupported import source: {0}")]
    Unsupported(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// The parser seam. A parser turns a source document into raw items; a
/// malformed document is a whole-file `ImportError`, while per-item problems
/// surface as `Issue`s during validation.
pub trait ImportParser: Send + Sync {
    fn parse(&self, source: &str) -> Result<Vec<ParsedItem>, ImportError>;
}

/// A raw row as produced by a parser (or by a CSV column mapping, MISSION-068).
/// All fields are stringly-typed on purpose: numbers/dates/enums are parsed
/// once, by the validator/normalizer.
#[derive(Debug, Clone)]
pub struct ParsedItem {
    /// 1-based source record number (line for CSV, index + 1 for JSON).
    pub row: usize,
    pub title: String,
    pub title_original: Option<String>,
    pub alt_titles: Vec<String>,
    pub content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<String>,
    pub duration_min: Option<String>,
    pub ep_count: Option<String>,
    pub ch_count: Option<String>,
    pub synopsis: Option<String>,
    /// `(role, name)` pairs, roles as `PersonRole::as_str()` values.
    pub people: Vec<(String, String)>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    /// `(provider, ext_id, url)` triples.
    pub external_ids: Vec<(String, String, Option<String>)>,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    /// User list state (MISSION-072 provider imports). Optional on purpose —
    /// generic CSV/JSON imports carry no user data and these stay `None`.
    pub my_status: Option<String>,
    pub my_rating: Option<String>,
    pub my_review: Option<String>,
    pub progress: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub repeat_count: Option<String>,
}

/// The validated canonical row: everything dedup and the transaction consume.
/// Only produced by `normalize`/`prepare` after validation passed.
#[derive(Debug, Clone)]
pub struct ImportRow {
    pub title: Title,
    pub content_type: ContentType,
    pub format: Option<String>,
    pub pub_status: MediaStatus,
    /// Validated ISO `YYYY-MM-DD` strings.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<i64>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<i64>,
    pub duration_min: Option<i64>,
    pub ep_count: Option<i64>,
    pub ch_count: Option<i64>,
    pub synopsis: Option<String>,
    pub people: Vec<(PersonRole, String)>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExternalId>,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    /// User list state (MISSION-072) — validated/normalized from the parsed
    /// row. `my_status` is a `CoreStatus`; a non-zero `repeat_count` forces the
    /// status to `Repeat` (the domain invariant, MISSION-022). Ratings are
    /// normalized to a 0–10 integer scale.
    pub my_status: Option<CoreStatus>,
    pub my_rating: Option<i64>,
    pub my_review: Option<String>,
    pub progress: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub repeat_count: Option<i64>,
}

/// The result of the validator+normalizer pair for one row.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PreparedRow {
    Valid {
        source_row: usize,
        row: ImportRow,
        warnings: Vec<Issue>,
    },
    Invalid {
        source_row: usize,
        errors: Vec<Issue>,
    },
}

/// Per-item outcome of the dedup + validation stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOutcome {
    /// No identity overlap — safe to import as a new media.
    New,
    /// The same (provider, external id) is already on file.
    InLibrary,
    /// A title/other candidate strongly overlaps an existing media.
    Duplicate,
    /// The row failed validation and cannot be imported.
    Invalid,
}

/// The result of matching one canonical row against the library.
#[derive(Debug, Clone, PartialEq)]
pub struct DedupMatch {
    pub outcome: RowOutcome,
    pub media_id: Option<MediaId>,
    pub kind: IdentityKind,
    /// 1.0 (exact id) / 0.95 (exact title) / fuzzy similarity.
    pub score: Option<f64>,
    pub title_similarity: f64,
}

/// One preview row, per-item outcome plus the issues the user should see.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewItem {
    pub source_row: usize,
    pub title: Option<String>,
    pub outcome: RowOutcome,
    pub matched_media_id: Option<String>,
    pub match_kind: Option<String>,
    pub match_score: Option<f64>,
    pub issues: Vec<Issue>,
    /// The canonical row, present only for `New` items (the commit target).
    #[serde(skip)]
    pub row: Option<ImportRow>,
}

/// The stage-5 preview: totals plus per-item outcomes (REQ-IMPORT-003).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportPreview {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub new: usize,
    pub in_library: usize,
    pub duplicates: usize,
    pub items: Vec<PreviewItem>,
}

/// Which source rows to import on commit. Only rows whose preview outcome is
/// `New` are honored; everything else is reported as skipped.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImportPlan {
    pub rows: Vec<usize>,
}

impl ImportPlan {
    /// A plan that imports nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// A plan that imports every `New` row of a preview.
    pub fn all_new(preview: &ImportPreview) -> Self {
        Self {
            rows: preview
                .items
                .iter()
                .filter(|item| item.outcome == RowOutcome::New)
                .map(|item| item.source_row)
                .collect(),
        }
    }
}

/// Per-row result status of the commit transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    Committed,
    Skipped,
    Failed,
}

/// One report row.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReportItem {
    pub source_row: usize,
    pub title: String,
    pub status: RowStatus,
    pub media_id: Option<String>,
    pub message: Option<String>,
}

/// The stage-7 result report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportReport {
    pub total: usize,
    pub committed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub items: Vec<ReportItem>,
}

/// Stage 2 — validator. Runs the field checks on a parsed row and returns every
/// issue (errors and warnings). A row with any `Error` is invalid and skipped.
pub fn validate(item: &ParsedItem) -> Vec<Issue> {
    let (_raw, issues) = parse_raw(item);
    issues
}

/// Stage 3 — normalizer. Maps a validated row onto the canonical `ImportRow`.
/// `Err` carries the error issues (the row is invalid); `Ok` carries the row
/// plus the warning issues the preview surfaces.
pub fn normalize(item: &ParsedItem) -> Result<(ImportRow, Vec<Issue>), Vec<Issue>> {
    let (raw, issues) = parse_raw(item);
    if issues.iter().any(|issue| issue.severity == Severity::Error) {
        Err(issues)
    } else {
        let warnings: Vec<Issue> = issues
            .into_iter()
            .filter(|issue| issue.severity == Severity::Warning)
            .collect();
        Ok((build_row(raw), warnings))
    }
}

/// validator + normalizer in one step — what `preview` actually runs.
pub fn prepare(item: &ParsedItem) -> PreparedRow {
    let (raw, issues) = parse_raw(item);
    let errors: Vec<Issue> = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .cloned()
        .collect();
    if !errors.is_empty() {
        return PreparedRow::Invalid {
            source_row: item.row,
            errors,
        };
    }
    let warnings: Vec<Issue> = issues
        .into_iter()
        .filter(|issue| issue.severity == Severity::Warning)
        .collect();
    PreparedRow::Valid {
        source_row: item.row,
        row: build_row(raw),
        warnings,
    }
}

/// Stage 4 — deduplicator. Matches a canonical row against the library's
/// identity candidates (MISSION-026): the same (provider, external id) is
/// `InLibrary`, a fold-equal title is `Duplicate`, everything else is `New`.
pub fn dedup(row: &ImportRow, candidates: &[IdentityCandidate]) -> DedupMatch {
    match identity::best_match(&row.title, &row.external_ids, candidates) {
        Some(m) => {
            let outcome = match m.kind {
                IdentityKind::Exact => RowOutcome::InLibrary,
                IdentityKind::TitleExact | IdentityKind::Fuzzy => RowOutcome::Duplicate,
                IdentityKind::None => RowOutcome::New,
            };
            DedupMatch {
                outcome,
                media_id: Some(m.media_id),
                kind: m.kind,
                score: Some(m.score),
                title_similarity: m.title_similarity,
            }
        }
        None => DedupMatch {
            outcome: RowOutcome::New,
            media_id: None,
            kind: IdentityKind::None,
            score: None,
            title_similarity: 0.0,
        },
    }
}

/// Stage 5 — preview. Validates + normalizes + dedups every row and aggregates
/// the outcomes. Read-only; the transaction stage is the application service.
pub fn preview(items: &[ParsedItem], candidates: &[IdentityCandidate]) -> ImportPreview {
    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut new = 0usize;
    let mut in_library = 0usize;
    let mut duplicates = 0usize;

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match prepare(item) {
            PreparedRow::Invalid { source_row, errors } => {
                invalid += 1;
                out.push(PreviewItem {
                    source_row,
                    title: clean_opt(Some(&item.title)),
                    outcome: RowOutcome::Invalid,
                    matched_media_id: None,
                    match_kind: None,
                    match_score: None,
                    issues: errors,
                    row: None,
                });
            }
            PreparedRow::Valid {
                source_row,
                row,
                warnings,
            } => {
                valid += 1;
                let m = dedup(&row, candidates);
                match m.outcome {
                    RowOutcome::New => new += 1,
                    RowOutcome::InLibrary => in_library += 1,
                    RowOutcome::Duplicate => duplicates += 1,
                    RowOutcome::Invalid => unreachable!("validated rows never dedup to Invalid"),
                }
                out.push(PreviewItem {
                    source_row,
                    title: Some(row.title.main().to_string()),
                    outcome: m.outcome,
                    matched_media_id: m.media_id.map(|id| id.as_str().to_string()),
                    match_kind: match m.kind {
                        IdentityKind::Exact => Some("in_library".to_string()),
                        IdentityKind::TitleExact | IdentityKind::Fuzzy => {
                            Some("duplicate".to_string())
                        }
                        IdentityKind::None => None,
                    },
                    match_score: m.score,
                    issues: warnings,
                    row: Some(row),
                });
            }
        }
    }

    ImportPreview {
        total: items.len(),
        valid,
        invalid,
        new,
        in_library,
        duplicates,
        items: out,
    }
}

/// Everything `validate` and `normalize` read off a raw item, parsed once.
struct RawFields {
    title: Option<Title>,
    content_type: Option<ContentType>,
    format: Option<String>,
    pub_status: MediaStatus,
    start_date: Option<String>,
    end_date: Option<String>,
    release_year: Option<i64>,
    language: Option<String>,
    country: Option<String>,
    content_rating: Option<String>,
    pages: Option<i64>,
    duration_min: Option<i64>,
    ep_count: Option<i64>,
    ch_count: Option<i64>,
    synopsis: Option<String>,
    people: Vec<(PersonRole, String)>,
    genres: Vec<String>,
    tags: Vec<String>,
    external_ids: Vec<ExternalId>,
    cover_url: Option<String>,
    banner_url: Option<String>,
    my_status: Option<CoreStatus>,
    my_rating: Option<i64>,
    my_review: Option<String>,
    progress: Option<i64>,
    started_at: Option<String>,
    completed_at: Option<String>,
    repeat_count: Option<i64>,
}

fn parse_raw(item: &ParsedItem) -> (RawFields, Vec<Issue>) {
    let mut issues: Vec<Issue> = Vec::new();

    let title = match parse_title(item, &mut issues) {
        Ok(title) => Some(title),
        Err(message) => {
            issues.push(err("title", message));
            None
        }
    };

    let content_type = match clean_opt(item.content_type.as_deref()) {
        None => {
            issues.push(err("content_type", "content type is required"));
            None
        }
        Some(value) => match ContentType::from_str(&value) {
            Ok(content_type) => Some(content_type),
            Err(_) => {
                issues.push(err(
                    "content_type",
                    format!("unrecognized content type: {value:?}"),
                ));
                None
            }
        },
    };

    let pub_status = match clean_opt(item.pub_status.as_deref()) {
        None => MediaStatus::Unknown,
        Some(value) => match MediaStatus::from_str(&value) {
            Ok(status) => status,
            Err(_) => {
                issues.push(warn(
                    "pub_status",
                    format!("unrecognized status {value:?}; using \"unknown\""),
                ));
                MediaStatus::Unknown
            }
        },
    };

    let start_date = parse_date("start_date", item.start_date.as_deref(), &mut issues);
    let end_date = parse_date("end_date", item.end_date.as_deref(), &mut issues);
    let end_date = match (&start_date, &end_date) {
        (Some(start), Some(end)) if end < start => {
            issues.push(warn(
                "end_date",
                "end date is before start date; end date dropped",
            ));
            None
        }
        _ => end_date,
    };

    let release_year = parse_count("release_year", item.release_year.as_deref(), &mut issues);
    let language = parse_language(item.language.as_deref(), &mut issues);

    let pages = parse_count("pages", item.pages.as_deref(), &mut issues);
    let duration_min = parse_count("duration_min", item.duration_min.as_deref(), &mut issues);
    let ep_count = parse_count("ep_count", item.ep_count.as_deref(), &mut issues);
    let ch_count = parse_count("ch_count", item.ch_count.as_deref(), &mut issues);

    let people = parse_people(&item.people, &mut issues);
    let genres = clean_labels(&item.genres);
    let tags = clean_labels(&item.tags);
    let external_ids = parse_external_ids(&item.external_ids, &mut issues);

    let my_status = parse_core_status(item.my_status.as_deref(), &mut issues);
    let my_rating = parse_rating(item.my_rating.as_deref(), &mut issues);
    let progress = parse_opt_count("progress", item.progress.as_deref(), &mut issues);
    let repeat_count = parse_opt_count("repeat_count", item.repeat_count.as_deref(), &mut issues);
    let my_status = match (my_status, repeat_count) {
        // A non-zero repeat forces the `Repeat` status (tracking invariant).
        (Some(CoreStatus::Repeat), _) => my_status,
        (_, Some(n)) if n > 0 => {
            if my_status.is_some() {
                issues.push(warn(
                    "my_status",
                    "repeat count implies a re-read/re-watch; status set to \"repeat\"",
                ));
            }
            Some(CoreStatus::Repeat)
        }
        _ => my_status,
    };
    let started_at = parse_date("started_at", item.started_at.as_deref(), &mut issues);
    let completed_at = parse_date("completed_at", item.completed_at.as_deref(), &mut issues);
    let completed_at = match (&started_at, &completed_at) {
        (Some(start), Some(finish)) if finish < start => {
            issues.push(warn(
                "completed_at",
                "completed date is before started date; completed date dropped",
            ));
            None
        }
        _ => completed_at,
    };
    let completed_at = match (&my_status, &completed_at) {
        // A finish date implies a terminal bucket (tracking invariant).
        (Some(status), Some(_))
            if !matches!(status, CoreStatus::Completed | CoreStatus::Dropped) =>
        {
            issues.push(warn(
                "completed_at",
                "completed date dropped for a non-terminal status",
            ));
            None
        }
        _ => completed_at,
    };

    let fields = RawFields {
        title,
        content_type,
        format: clean_opt(item.format.as_deref()),
        pub_status,
        start_date,
        end_date,
        release_year,
        language,
        country: clean_opt(item.country.as_deref()),
        content_rating: clean_opt(item.content_rating.as_deref()),
        pages,
        duration_min,
        ep_count,
        ch_count,
        synopsis: clean_opt(item.synopsis.as_deref()),
        people,
        genres,
        tags,
        external_ids,
        cover_url: clean_opt(item.cover_url.as_deref()),
        banner_url: clean_opt(item.banner_url.as_deref()),
        my_status,
        my_rating,
        my_review: clean_opt(item.my_review.as_deref()),
        progress,
        started_at,
        completed_at,
        repeat_count,
    };
    (fields, issues)
}

fn build_row(raw: RawFields) -> ImportRow {
    ImportRow {
        title: raw.title.expect("validated title"),
        content_type: raw.content_type.expect("validated content type"),
        format: raw.format,
        pub_status: raw.pub_status,
        start_date: raw.start_date,
        end_date: raw.end_date,
        release_year: raw.release_year,
        language: raw.language,
        country: raw.country,
        content_rating: raw.content_rating,
        pages: raw.pages,
        duration_min: raw.duration_min,
        ep_count: raw.ep_count,
        ch_count: raw.ch_count,
        synopsis: raw.synopsis,
        people: raw.people,
        genres: raw.genres,
        tags: raw.tags,
        external_ids: raw.external_ids,
        cover_url: raw.cover_url,
        banner_url: raw.banner_url,
        my_status: raw.my_status,
        my_rating: raw.my_rating,
        my_review: raw.my_review,
        progress: raw.progress,
        started_at: raw.started_at,
        completed_at: raw.completed_at,
        repeat_count: raw.repeat_count,
    }
}

fn parse_title(item: &ParsedItem, issues: &mut Vec<Issue>) -> Result<Title, String> {
    let main = item.title.trim().to_string();
    if main.is_empty() {
        return Err("title must not be blank".to_string());
    }
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(main.to_lowercase());

    let mut original = clean_opt(item.title_original.as_deref());
    if let Some(value) = &original {
        if !seen.insert(value.to_lowercase()) {
            issues.push(warn(
                "title_original",
                "original title duplicates the main title; dropped",
            ));
            original = None;
        }
    }

    let mut alternatives = Vec::new();
    for alt in &item.alt_titles {
        let alt = alt.trim();
        if alt.is_empty() {
            continue;
        }
        if seen.insert(alt.to_lowercase()) {
            alternatives.push(alt.to_string());
        } else {
            issues.push(warn(
                "alt_titles",
                format!("title {alt:?} duplicates another title; dropped"),
            ));
        }
    }

    match Title::new(main, original, alternatives) {
        Ok(title) => Ok(title),
        Err(crate::domain::DomainError::Validation(message)) => Err(message),
    }
}

fn parse_date(field: &str, value: Option<&str>, issues: &mut Vec<Issue>) -> Option<String> {
    match clean_opt(value) {
        None => None,
        Some(value) => match DateOnly::new(&value) {
            Ok(date) => Some(date.as_str().to_string()),
            Err(_) => {
                issues.push(warn(field, format!("invalid ISO date {value:?} dropped")));
                None
            }
        },
    }
}

fn parse_language(value: Option<&str>, issues: &mut Vec<Issue>) -> Option<String> {
    match clean_opt(value) {
        None => None,
        Some(value) => match LanguageCode::new(&value) {
            Ok(code) => Some(code.as_str().to_string()),
            Err(_) => {
                issues.push(warn(
                    "language",
                    format!("invalid language code {value:?} dropped"),
                ));
                None
            }
        },
    }
}

fn parse_count(field: &str, value: Option<&str>, issues: &mut Vec<Issue>) -> Option<i64> {
    match clean_opt(value) {
        None => None,
        Some(value) => match value.parse::<i64>().ok().filter(|n| *n >= 0) {
            Some(count) => Some(count),
            None => {
                issues.push(err(
                    field,
                    format!("{field} must be a non-negative integer, got {value:?}"),
                ));
                None
            }
        },
    }
}

/// Like [`parse_count`] but degrades to a warning: user-list state is optional
/// and a stray cell should never invalidate an otherwise-importable row.
fn parse_opt_count(field: &str, value: Option<&str>, issues: &mut Vec<Issue>) -> Option<i64> {
    match clean_opt(value) {
        None => None,
        Some(value) => match value.parse::<i64>().ok().filter(|n| *n >= 0) {
            Some(count) => Some(count),
            None => {
                issues.push(warn(
                    field,
                    format!("{field} must be a non-negative integer; dropped"),
                ));
                None
            }
        },
    }
}

/// A user status (`CoreStatus::as_str` values). Unknown values degrade to a
/// warning and `None` — a provider's odd status label never blocks the row.
fn parse_core_status(value: Option<&str>, issues: &mut Vec<Issue>) -> Option<CoreStatus> {
    match clean_opt(value) {
        None => None,
        Some(value) => match CoreStatus::from_str(&value) {
            Ok(status) => Some(status),
            Err(_) => {
                issues.push(warn(
                    "my_status",
                    format!("unrecognized user status {value:?} dropped"),
                ));
                None
            }
        },
    }
}

/// A user rating normalized to the 0–10 integer scale. Non-integers and
/// out-of-range values degrade to a warning and `None`.
fn parse_rating(value: Option<&str>, issues: &mut Vec<Issue>) -> Option<i64> {
    match clean_opt(value) {
        None => None,
        Some(value) => match value.parse::<i64>().ok().filter(|n| (0..=10).contains(n)) {
            Some(rating) => Some(rating),
            None => {
                issues.push(warn(
                    "my_rating",
                    format!("rating {value:?} is not a 0–10 integer; dropped"),
                ));
                None
            }
        },
    }
}

fn parse_people(people: &[(String, String)], issues: &mut Vec<Issue>) -> Vec<(PersonRole, String)> {
    let mut out = Vec::new();
    for (role, name) in people {
        let name = name.trim();
        if name.is_empty() {
            issues.push(warn(
                "people",
                "person name must not be blank; person dropped",
            ));
            continue;
        }
        match PersonRole::from_str(role.trim()) {
            Ok(role) => out.push((role, name.to_string())),
            Err(_) => issues.push(warn(
                "people",
                format!("unknown person role {role:?}; person dropped"),
            )),
        }
    }
    out
}

fn parse_external_ids(
    ids: &[(String, String, Option<String>)],
    issues: &mut Vec<Issue>,
) -> Vec<ExternalId> {
    let mut out = Vec::new();
    let mut providers: HashSet<String> = HashSet::new();
    for (provider, value, url) in ids {
        let provider = provider.trim();
        let Ok(provider_id) = ProviderId::new(provider) else {
            issues.push(warn(
                "external_ids",
                format!("invalid provider id {provider:?}; external id dropped"),
            ));
            continue;
        };
        if value.trim().is_empty() {
            issues.push(warn(
                "external_ids",
                format!("external id for provider {provider:?} has a blank value; dropped"),
            ));
            continue;
        }
        if !providers.insert(provider_id.as_str().to_string()) {
            issues.push(warn(
                "external_ids",
                format!("duplicate external id for provider {provider:?}; dropped"),
            ));
            continue;
        }
        if let Ok(id) = ExternalId::new(provider_id, value.trim(), url.clone()) {
            out.push(id);
        }
    }
    out
}

fn clean_labels(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if seen.insert(value.to_lowercase()) {
            out.push(value.to_string());
        }
    }
    out
}

fn clean_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn err(field: &str, message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Error,
        field: field.to_string(),
        message: message.into(),
    }
}

fn warn(field: &str, message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Warning,
        field: field.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str) -> ParsedItem {
        ParsedItem {
            row: 1,
            title: title.to_string(),
            title_original: None,
            alt_titles: Vec::new(),
            content_type: Some("novel".to_string()),
            format: Some("light_novel".to_string()),
            pub_status: Some("ongoing".to_string()),
            start_date: None,
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
            genres: vec!["Fantasy".to_string(), "fantasy".to_string()],
            tags: vec!["isekai".to_string()],
            external_ids: Vec::new(),
            cover_url: None,
            banner_url: None,
            my_status: None,
            my_rating: None,
            my_review: None,
            progress: None,
            started_at: None,
            completed_at: None,
            repeat_count: None,
        }
    }

    fn candidate(id: &str, main: &str, ext: &[(&str, &str)]) -> IdentityCandidate {
        let external_ids = ext
            .iter()
            .map(|(provider, value)| {
                ExternalId::new(ProviderId::new(*provider).unwrap(), *value, None).unwrap()
            })
            .collect();
        IdentityCandidate {
            media_id: MediaId::new(id).unwrap(),
            titles: Title::new(main, None, Vec::new()).unwrap(),
            external_ids,
        }
    }

    #[test]
    fn normalize_builds_canonical_row_from_valid_item() {
        let (row, warnings) = normalize(&item("Sword of the Dawn")).expect("valid");
        assert!(warnings.is_empty());
        assert_eq!(row.title.main(), "Sword of the Dawn");
        assert_eq!(row.content_type, ContentType::Novel);
        assert_eq!(row.format.as_deref(), Some("light_novel"));
        assert_eq!(row.pub_status, MediaStatus::Ongoing);
        assert_eq!(row.release_year, Some(2025));
        assert_eq!(row.language.as_deref(), Some("ja"));
        assert_eq!(row.country.as_deref(), Some("JP"));
        assert_eq!(row.pages, Some(320));
        assert_eq!(
            row.people,
            vec![(PersonRole::Author, "Test Author".to_string())]
        );
        assert_eq!(
            row.genres,
            vec!["Fantasy".to_string()],
            "deduped, first casing kept"
        );
        assert_eq!(row.tags, vec!["isekai".to_string()]);
    }

    #[test]
    fn validate_rejects_structural_errors() {
        let mut blank = item("  ");
        blank.content_type = None;
        let issues = validate(&blank);
        assert!(issues
            .iter()
            .any(|i| i.severity == Severity::Error && i.field == "title"));

        let mut no_type = item("Sword");
        no_type.content_type = None;
        assert!(validate(&no_type)
            .iter()
            .any(|i| i.severity == Severity::Error && i.field == "content_type"));

        let mut bad_type = item("Sword");
        bad_type.content_type = Some("podcast".to_string());
        assert!(validate(&bad_type)
            .iter()
            .any(|i| i.severity == Severity::Error && i.field == "content_type"));

        let mut negative = item("Sword");
        negative.pages = Some("-5".to_string());
        assert!(validate(&negative)
            .iter()
            .any(|i| i.severity == Severity::Error && i.field == "pages"));

        let mut bad_year = item("Sword");
        bad_year.release_year = Some("n/a".to_string());
        assert!(validate(&bad_year)
            .iter()
            .any(|i| i.severity == Severity::Error && i.field == "release_year"));
    }

    #[test]
    fn validate_warns_and_normalize_degrades_optional_fields() {
        let mut degraded = item("Sword");
        degraded.pub_status = Some("watching".to_string());
        degraded.language = Some("EN".to_string());
        degraded.start_date = Some("May 1989".to_string());
        degraded.people = vec![("editor".to_string(), "Jane".to_string())];
        degraded.external_ids = vec![
            ("OpenLibrary".to_string(), "OL1".to_string(), None),
            ("anilist".to_string(), "".to_string(), None),
            ("anilist".to_string(), "42".to_string(), None),
        ];

        let issues = validate(&degraded);
        assert!(!issues.iter().any(|i| i.severity == Severity::Error));
        for field in [
            "pub_status",
            "language",
            "start_date",
            "people",
            "external_ids",
        ] {
            assert!(
                issues.iter().any(|i| i.field == field),
                "expected a warning for {field}"
            );
        }

        let (row, warnings) = normalize(&degraded).expect("valid after warnings");
        assert_eq!(warnings.len(), issues.len());
        assert_eq!(row.pub_status, MediaStatus::Unknown);
        assert_eq!(row.language, None);
        assert_eq!(row.start_date, None);
        assert!(row.people.is_empty());
        assert_eq!(
            row.external_ids,
            vec![ExternalId::new(ProviderId::new("anilist").unwrap(), "42", None).unwrap()],
            "invalid ids dropped, valid one kept"
        );
    }

    #[test]
    fn date_order_violation_drops_end_date() {
        let mut reversed = item("Sword");
        reversed.start_date = Some("2025-06-01".to_string());
        reversed.end_date = Some("2025-01-01".to_string());
        let (row, warnings) = normalize(&reversed).expect("valid");
        assert_eq!(row.start_date.as_deref(), Some("2025-06-01"));
        assert_eq!(row.end_date, None, "out-of-order end date dropped");
        assert!(warnings.iter().any(|i| i.field == "end_date"));
    }

    #[test]
    fn alt_titles_are_cleaned_and_deduplicated() {
        let mut with_alts = item("Sword of the Dawn");
        with_alts.title_original = Some("Dawn".to_string());
        with_alts.alt_titles = vec![
            "Sword of the Dawn".to_string(),
            "  ".to_string(),
            "Sword of Dawn".to_string(),
            "Sword of Dawn".to_string(),
        ];
        let (row, warnings) = normalize(&with_alts).expect("valid");
        assert_eq!(row.title.original(), Some("Dawn"));
        assert_eq!(row.title.alternatives(), &["Sword of Dawn".to_string()]);
        assert!(warnings.iter().any(|i| i.field == "alt_titles"));
    }

    #[test]
    fn prepare_invalid_carries_only_errors() {
        let mut invalid = item("  ");
        invalid.content_type = Some("podcast".to_string());
        match prepare(&invalid) {
            PreparedRow::Invalid { source_row, errors } => {
                assert_eq!(source_row, 1);
                assert!(errors.iter().all(|i| i.severity == Severity::Error));
            }
            PreparedRow::Valid { .. } => panic!("structurally invalid row must be invalid"),
        }
    }

    #[test]
    fn dedup_resolves_in_library_duplicate_and_new() {
        let candidates = vec![
            candidate("m-1", "Sword of the Dawn", &[("anilist", "42")]),
            candidate("m-2", "Sword of the Dawn", &[]),
        ];

        let mut with_id = item("Sword of the Dawn");
        with_id.external_ids = vec![("anilist".to_string(), "42".to_string(), None)];
        let (row, _) = normalize(&with_id).unwrap();
        let m = dedup(&row, &candidates);
        assert_eq!(m.outcome, RowOutcome::InLibrary);
        assert_eq!(m.media_id.unwrap().as_str(), "m-1");

        let mut other_id = item("Sword of the Dawn");
        other_id.external_ids = vec![("anilist".to_string(), "999".to_string(), None)];
        let (row, _) = normalize(&other_id).unwrap();
        assert_eq!(dedup(&row, &candidates).outcome, RowOutcome::Duplicate);

        let (row, _) = normalize(&item("Berserk")).unwrap();
        let m = dedup(&row, &candidates);
        assert_eq!(m.outcome, RowOutcome::New);
        assert!(m.media_id.is_none());
    }

    #[test]
    fn preview_counts_and_flags_outcomes() {
        let candidates = vec![candidate("m-1", "Sword of the Dawn", &[])];

        let mut duplicate = item("Sword of the Dawn");
        duplicate.row = 2;
        let mut invalid = item(" ");
        invalid.row = 3;

        let preview = preview(&[item("Berserk"), duplicate, invalid], &candidates);
        assert_eq!(preview.total, 3);
        assert_eq!(preview.valid, 2);
        assert_eq!(preview.invalid, 1);
        assert_eq!(preview.new, 1);
        assert_eq!(preview.in_library, 0);
        assert_eq!(preview.duplicates, 1);

        assert_eq!(preview.items[0].outcome, RowOutcome::New);
        assert_eq!(preview.items[0].title.as_deref(), Some("Berserk"));
        assert!(
            preview.items[0].row.is_some(),
            "New rows carry the commit target"
        );

        assert_eq!(preview.items[1].outcome, RowOutcome::Duplicate);
        assert_eq!(preview.items[1].matched_media_id.as_deref(), Some("m-1"));
        assert_eq!(preview.items[1].match_kind.as_deref(), Some("duplicate"));

        assert_eq!(preview.items[2].outcome, RowOutcome::Invalid);
        assert!(preview.items[2].row.is_none());
    }

    #[test]
    fn all_new_plan_selects_only_new_rows() {
        let candidates = vec![candidate("m-1", "Sword of the Dawn", &[])];
        let mut duplicate = item("Sword of the Dawn");
        duplicate.row = 2;
        let preview = preview(&[item("Berserk"), duplicate], &candidates);

        let plan = ImportPlan::all_new(&preview);
        assert_eq!(plan.rows, vec![1]);
        assert!(ImportPlan::none().rows.is_empty());
    }

    #[test]
    fn user_state_normalizes_status_rating_progress_and_dates() {
        let mut list = item("Sword of the Dawn");
        list.my_status = Some("in_progress".to_string());
        list.my_rating = Some("8".to_string());
        list.my_review = Some("Lovely.".to_string());
        list.progress = Some("12".to_string());
        list.started_at = Some("2026-01-01".to_string());
        list.completed_at = None;
        list.repeat_count = Some("0".to_string());

        let (row, warnings) = normalize(&list).expect("valid");
        assert!(warnings.is_empty());
        assert_eq!(row.my_status, Some(CoreStatus::InProgress));
        assert_eq!(row.my_rating, Some(8));
        assert_eq!(row.my_review.as_deref(), Some("Lovely."));
        assert_eq!(row.progress, Some(12));
        assert_eq!(row.started_at.as_deref(), Some("2026-01-01"));
        assert_eq!(row.completed_at, None);
        assert_eq!(row.repeat_count, Some(0));
    }

    #[test]
    fn user_state_degrades_bad_cells_with_warnings() {
        let mut list = item("Sword");
        list.my_status = Some("reading".to_string());
        list.my_rating = Some("7.5".to_string());
        list.progress = Some("n/a".to_string());
        list.repeat_count = Some("-1".to_string());

        let (row, warnings) = normalize(&list).expect("valid");
        assert_eq!(row.my_status, None);
        assert_eq!(row.my_rating, None);
        assert_eq!(row.progress, None);
        assert_eq!(row.repeat_count, None);
        for field in ["my_status", "my_rating", "progress", "repeat_count"] {
            assert!(
                warnings.iter().any(|i| i.field == field),
                "expected a warning for {field}"
            );
        }
    }

    #[test]
    fn non_zero_repeat_forces_repeat_status() {
        let mut list = item("Sword");
        list.my_status = Some("completed".to_string());
        list.repeat_count = Some("2".to_string());
        let (row, warnings) = normalize(&list).expect("valid");
        assert_eq!(row.my_status, Some(CoreStatus::Repeat));
        assert_eq!(row.repeat_count, Some(2));
        assert!(warnings.iter().any(|i| i.field == "my_status"));

        let mut alone = item("Sword");
        alone.repeat_count = Some("1".to_string());
        let (row, _) = normalize(&alone).expect("valid");
        assert_eq!(row.my_status, Some(CoreStatus::Repeat));
    }

    #[test]
    fn completed_date_is_dropped_for_non_terminal_status() {
        let mut list = item("Sword");
        list.my_status = Some("in_progress".to_string());
        list.completed_at = Some("2026-06-01".to_string());
        let (row, warnings) = normalize(&list).expect("valid");
        assert_eq!(row.completed_at, None);
        assert!(warnings.iter().any(|i| i.field == "completed_at"));

        let mut finished = item("Sword");
        finished.my_status = Some("completed".to_string());
        finished.started_at = Some("2026-06-01".to_string());
        finished.completed_at = Some("2026-05-01".to_string());
        let (row, warnings) = normalize(&finished).expect("valid");
        assert_eq!(row.completed_at, None, "finish before start dropped");
        assert!(warnings.iter().any(|i| i.field == "completed_at"));
    }
}
