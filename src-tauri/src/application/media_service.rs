//! Media application service (MISSION-038).
//!
//! Use-cases that orchestrate domain validation and persistence. Commands stay
//! thin; this module owns the flow: parse input → build the `Media` aggregate →
//! validate invariants → persist via the media repository.
//!
//! Timestamps and ids are minted here (repositories are clock-free by design):
//! ids are UUID-style, timestamps ISO 8601 UTC.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::application::progress_service::{summary_dto, ProgressSummary};
use crate::domain::enums::{ContentType, MediaStatus};
use crate::domain::media::{Media, MediaRuntime};
use crate::domain::value_objects::{LanguageCode, MediaId};
use crate::error::AppError;
use crate::infrastructure::repositories::media::{
    facets as media_facets, list as list_rows, AltTitle, ExternalId, MediaFacets, MediaFilter,
    MediaRecord, MediaRelation, MediaSort, MediaSummary,
};
use crate::infrastructure::repositories::tracking;

/// Command input for a manual add (MISSION-038).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddMediaInput {
    pub title: String,
    pub content_type: String,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub synopsis: Option<String>,
    pub release_year: Option<i64>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub pages: Option<i64>,
    pub duration_min: Option<i64>,
    pub ep_count: Option<i64>,
    pub ch_count: Option<i64>,
    pub genres: Vec<String>,
}

/// Media use-cases.
pub struct MediaService {
    pool: SqlitePool,
}

/// Listing input (MISSION-041). All filters optional; empty input returns the
/// whole library ordered by title ascending.
#[derive(Debug, Default, Clone)]
pub struct MediaListInput {
    pub content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub genre: Option<String>,
    pub tag: Option<String>,
    pub year: Option<i64>,
    pub favorite: Option<bool>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub ascending: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// A serializable library row for the grid/list views.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MediaListItem {
    pub id: String,
    pub content_type: String,
    pub title: String,
    pub pub_status: String,
    pub release_year: Option<i64>,
    pub cover_asset_id: Option<String>,
    pub updated_at: String,
    /// Per-media progress summary driving the in-grid quick controls (MISSION-049).
    pub progress: ProgressSummary,
}

impl MediaService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a media entry from manual input; resolves with the new media id.
    pub async fn add_media(&self, input: AddMediaInput) -> Result<MediaId, AppError> {
        let now = Utc::now().to_rfc3339();
        let id = MediaId::new(format!("m-{}", Uuid::new_v4()))?;
        let content_type = ContentType::from_str(&input.content_type)?;
        let status = MediaStatus::from_str(input.pub_status.as_deref().unwrap_or("unknown"))?;
        let language = input
            .language
            .as_deref()
            .map(LanguageCode::new)
            .transpose()?;
        let release_year = input
            .release_year
            .map(u16::try_from)
            .transpose()
            .map_err(|_| AppError::validation("release year out of range"))?;

        let genres: Vec<String> = {
            let mut seen = HashSet::new();
            input
                .genres
                .into_iter()
                .map(|genre| genre.trim().to_string())
                .filter(|genre| !genre.is_empty() && seen.insert(genre.clone()))
                .collect()
        };

        let media = Media {
            id,
            content_type,
            format: input.format,
            title: crate::domain::value_objects::Title::new(input.title, None, Vec::new())?,
            synopsis: input.synopsis,
            status,
            start_date: None,
            end_date: None,
            release_year,
            language,
            country: input.country,
            content_rating: None,
            runtime: MediaRuntime {
                pages: to_u32(input.pages)?,
                duration_min: to_u32(input.duration_min)?,
                ep_count: to_u32(input.ep_count)?,
                ch_count: to_u32(input.ch_count)?,
            },
            people: Vec::new(),
            genres,
            tags: Vec::new(),
            external_ids: Vec::new(),
            relations: Vec::new(),
            provider: None,
            provider_url: None,
            metadata_refreshed_at: None,
            created_at: now.clone(),
            updated_at: now,
        };

        media.validate()?;
        let record = to_record(&media);
        crate::infrastructure::repositories::media::create(&self.pool, &record).await?;
        Ok(media.id)
    }

    /// List library entries with optional filters; title-ascending by default.
    pub async fn list_media(&self, input: MediaListInput) -> Result<Vec<MediaListItem>, AppError> {
        let sort = match input.sort.as_deref().unwrap_or("title") {
            "title" => MediaSort::Title,
            "created_at" => MediaSort::CreatedAt,
            "updated_at" => MediaSort::UpdatedAt,
            "release_year" => MediaSort::ReleaseYear,
            other => return Err(AppError::validation(format!("unknown sort: {other}"))),
        };
        let ascending = match (input.sort.as_deref(), input.ascending) {
            (None, _) => true,
            (_, Some(ascending)) => ascending,
            (Some(_), None) => false,
        };
        let filter = MediaFilter {
            content_type: input.content_type,
            format: input.format,
            pub_status: input.pub_status,
            genre: input.genre,
            tag: input.tag,
            year: input.year,
            favorite: input.favorite,
            search: input.search,
            sort,
            ascending,
            limit: input.limit,
            offset: input.offset,
        };

        let rows = list_rows(&self.pool, &filter).await?;
        self.to_list_items(rows).await
    }

    /// Distinct filter values present in the library (MISSION-041).
    pub async fn list_facets(&self) -> Result<MediaFacets, AppError> {
        media_facets(&self.pool).await
    }

    /// Read the full aggregate for one media (MISSION-042).
    pub async fn get_media(&self, id: &str) -> Result<Option<MediaRecord>, AppError> {
        crate::infrastructure::repositories::media::get(&self.pool, id).await
    }

    /// Local full-text search over titles/alt titles/people/genres/tags etc.
    /// (MISSION-043). Empty/whitespace queries resolve to no results.
    pub async fn search_media(&self, query: &str) -> Result<Vec<MediaListItem>, AppError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let rows = crate::infrastructure::repositories::media::search(&self.pool, query).await?;
        self.to_list_items(rows).await
    }

    /// Map repo summary rows onto list items, attaching each media's progress
    /// summary in one batched pass (no per-row queries).
    async fn to_list_items(&self, rows: Vec<MediaSummary>) -> Result<Vec<MediaListItem>, AppError> {
        let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
        let summaries: HashMap<String, ProgressSummary> =
            tracking::progress_summaries(&self.pool, &ids)
                .await?
                .into_iter()
                .map(|row| (row.media_id.clone(), summary_dto(&row)))
                .collect();
        Ok(rows
            .into_iter()
            .map(|row| {
                let summary = summaries.get(&row.id);
                media_list_item(row, summary)
            })
            .collect())
    }
}

/// Map a repository summary row onto the serializable DTO.
fn media_list_item(row: MediaSummary, summary: Option<&ProgressSummary>) -> MediaListItem {
    MediaListItem {
        id: row.id,
        content_type: row.content_type,
        title: row.title_main,
        pub_status: row.pub_status,
        release_year: row.release_year,
        cover_asset_id: row.cover_asset_id,
        updated_at: row.updated_at,
        progress: summary.cloned().unwrap_or(ProgressSummary {
            percent: None,
            completed: 0,
            total: 0,
            next_label: None,
            next_node_id: None,
        }),
    }
}

fn to_u32(value: Option<i64>) -> Result<Option<u32>, AppError> {
    match value {
        None => Ok(None),
        Some(value) => u32::try_from(value)
            .map(Some)
            .map_err(|_| AppError::validation(format!("runtime value out of range: {value}"))),
    }
}

/// Map the validated domain aggregate onto the persistence record.
fn to_record(media: &Media) -> MediaRecord {
    MediaRecord {
        id: media.id.as_str().to_string(),
        content_type: media.content_type.as_str().to_string(),
        format: media.format.clone(),
        title_main: media.title.main().to_string(),
        title_original: media.title.original().map(str::to_string),
        synopsis: media.synopsis.clone(),
        pub_status: media.status.as_str().to_string(),
        start_date: media
            .start_date
            .as_ref()
            .map(|date| date.as_str().to_string()),
        end_date: media
            .end_date
            .as_ref()
            .map(|date| date.as_str().to_string()),
        release_year: media.release_year.map(Into::into),
        language: media
            .language
            .as_ref()
            .map(|lang| lang.as_str().to_string()),
        country: media.country.clone(),
        content_rating: media.content_rating.clone(),
        pages: media.runtime.pages.map(Into::into),
        duration_min: media.runtime.duration_min.map(Into::into),
        ep_count: media.runtime.ep_count.map(Into::into),
        ch_count: media.runtime.ch_count.map(Into::into),
        cover_asset_id: None,
        banner_asset_id: None,
        provider: media.provider.as_ref().map(|p| p.as_str().to_string()),
        provider_url: media.provider_url.clone(),
        metadata_refreshed_at: media.metadata_refreshed_at.clone(),
        created_at: media.created_at.clone(),
        updated_at: media.updated_at.clone(),
        alt_titles: media
            .title
            .alternatives()
            .iter()
            .map(|title| AltTitle {
                lang: String::new(),
                title: title.clone(),
            })
            .collect(),
        people: media
            .people
            .iter()
            .map(|person| person.name.clone())
            .collect(),
        genres: media.genres.clone(),
        tags: media.tags.clone(),
        external_ids: media
            .external_ids
            .iter()
            .map(|id| ExternalId {
                provider: id.provider().as_str().to_string(),
                ext_id: id.value().to_string(),
                url: id.url().map(str::to_string),
            })
            .collect(),
        relations: media
            .relations
            .iter()
            .map(|relation| MediaRelation {
                to_id: relation.to_id.as_str().to_string(),
                relation: relation.kind.as_str().to_string(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::media;
    use crate::infrastructure::test_support::migrated_pool;

    fn input() -> AddMediaInput {
        AddMediaInput {
            title: "Sword of the Dawn".into(),
            content_type: "novel".into(),
            format: Some("light_novel".into()),
            pub_status: Some("ongoing".into()),
            synopsis: Some("A blade that learns to dream.".into()),
            release_year: Some(2026),
            language: Some("ja".into()),
            country: Some("JP".into()),
            pages: Some(320),
            duration_min: None,
            ep_count: None,
            ch_count: None,
            genres: vec!["fantasy".into(), "fantasy".into()],
        }
    }

    #[tokio::test]
    async fn add_media_persists_and_returns_id() {
        let (pool, _path) = migrated_pool("media_service_add.db").await;
        let service = MediaService::new(pool.clone());

        let id = service.add_media(input()).await.expect("add media");
        assert!(id.as_str().starts_with("m-"));

        let stored = media::get(&pool, id.as_str())
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(stored.title_main, "Sword of the Dawn");
        assert_eq!(stored.content_type, "novel");
        assert_eq!(stored.pub_status, "ongoing");
        assert_eq!(stored.release_year, Some(2026));
        assert_eq!(stored.pages, Some(320));
        assert_eq!(stored.genres, vec!["fantasy".to_string()]);
    }

    #[tokio::test]
    async fn add_media_rejects_blank_title() {
        let (pool, _path) = migrated_pool("media_service_invalid.db").await;
        let service = MediaService::new(pool.clone());

        let mut bad = input();
        bad.title = "  ".into();
        let err = service
            .add_media(bad)
            .await
            .expect_err("blank title rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_media_rejects_unknown_content_type() {
        let (pool, _path) = migrated_pool("media_service_type.db").await;
        let service = MediaService::new(pool.clone());

        let mut bad = input();
        bad.content_type = "podcast".into();
        let err = service
            .add_media(bad)
            .await
            .expect_err("unknown type rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_media_defaults_publication_status() {
        let (pool, _path) = migrated_pool("media_service_status.db").await;
        let service = MediaService::new(pool.clone());

        let mut minimal = input();
        minimal.pub_status = None;
        let id = service.add_media(minimal).await.expect("add media");
        let stored = media::get(&pool, id.as_str())
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(stored.pub_status, "unknown");
    }

    #[tokio::test]
    async fn add_media_rejects_negative_runtime() {
        let (pool, _path) = migrated_pool("media_service_runtime.db").await;
        let service = MediaService::new(pool.clone());

        let mut bad = input();
        bad.pages = Some(-3);
        let err = service
            .add_media(bad)
            .await
            .expect_err("negative pages rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn list_media_returns_all_entries_title_ascending() {
        let (pool, _path) = migrated_pool("media_service_list.db").await;
        let service = MediaService::new(pool.clone());

        let mut first = input();
        first.title = "Anzu and the Paper Moon".into();
        let mut second = input();
        second.title = "Beneath the Iron Sky".into();
        service.add_media(first).await.expect("add first");
        service.add_media(second).await.expect("add second");

        let items = service
            .list_media(MediaListInput::default())
            .await
            .expect("list");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Anzu and the Paper Moon");
        assert_eq!(items[1].title, "Beneath the Iron Sky");
        assert!(items[0].id.starts_with("m-"));
        assert_eq!(items[0].content_type, "novel");
        assert_eq!(items[0].pub_status, "ongoing");
        assert_eq!(items[0].release_year, Some(2026));
    }

    #[tokio::test]
    async fn list_media_filters_by_content_type() {
        let (pool, _path) = migrated_pool("media_service_list_filter.db").await;
        let service = MediaService::new(pool.clone());

        let mut novel = input();
        novel.title = "The Clockwork Archive".into();
        let mut anime = input();
        anime.title = "Skyline Echoes".into();
        anime.content_type = "anime".into();
        service.add_media(novel).await.expect("add novel");
        service.add_media(anime).await.expect("add anime");

        let items = service
            .list_media(MediaListInput {
                content_type: Some("anime".into()),
                ..MediaListInput::default()
            })
            .await
            .expect("list anime");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Skyline Echoes");
    }

    #[tokio::test]
    async fn list_media_filters_by_format_and_year() {
        let (pool, _path) = migrated_pool("media_service_list_format_year.db").await;
        let service = MediaService::new(pool.clone());

        let mut light_novel = input();
        light_novel.title = "The Silent Witching Hour".into();
        light_novel.format = Some("light_novel".into());
        light_novel.release_year = Some(2025);
        let mut webtoon = input();
        webtoon.title = "Crimson Web".into();
        webtoon.format = Some("webtoon".into());
        webtoon.release_year = Some(2026);
        service.add_media(light_novel).await.expect("add first");
        service.add_media(webtoon).await.expect("add second");

        let by_format = service
            .list_media(MediaListInput {
                format: Some("webtoon".into()),
                ..MediaListInput::default()
            })
            .await
            .expect("list by format");
        assert_eq!(by_format.len(), 1);
        assert_eq!(by_format[0].title, "Crimson Web");

        let by_year = service
            .list_media(MediaListInput {
                year: Some(2025),
                ..MediaListInput::default()
            })
            .await
            .expect("list by year");
        assert_eq!(by_year.len(), 1);
        assert_eq!(by_year[0].title, "The Silent Witching Hour");
    }

    #[tokio::test]
    async fn get_media_returns_full_aggregate() {
        let (pool, _path) = migrated_pool("media_service_get.db").await;
        let service = MediaService::new(pool.clone());

        let mut media_input = input();
        media_input.genres = vec!["fantasy".into(), "science_fiction".into()];
        let id = service.add_media(media_input).await.expect("add media");

        let detail = service
            .get_media(id.as_str())
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(detail.title_main, "Sword of the Dawn");
        assert_eq!(detail.title_original, None);
        assert_eq!(detail.content_type, "novel");
        assert_eq!(detail.format, Some("light_novel".to_string()));
        assert_eq!(detail.pub_status, "ongoing");
        assert_eq!(detail.release_year, Some(2026));
        assert_eq!(detail.language, Some("ja".to_string()));
        assert_eq!(detail.country, Some("JP".to_string()));
        assert_eq!(detail.pages, Some(320));
        assert_eq!(
            detail.synopsis.as_deref(),
            Some("A blade that learns to dream.")
        );
        assert_eq!(
            detail.genres,
            vec!["fantasy".to_string(), "science_fiction".to_string()]
        );
        assert!(detail.created_at <= detail.updated_at);
    }

    #[tokio::test]
    async fn get_media_returns_none_for_unknown_id() {
        let (pool, _path) = migrated_pool("media_service_get_missing.db").await;
        let service = MediaService::new(pool.clone());
        assert!(service
            .get_media("m-does-not-exist")
            .await
            .expect("get")
            .is_none());
    }

    #[tokio::test]
    async fn search_media_returns_matching_summaries() {
        let (pool, _path) = migrated_pool("media_service_search.db").await;
        let service = MediaService::new(pool.clone());

        let mut one = input();
        one.title = "Sword of the Dawn".into();
        service.add_media(one).await.expect("add one");

        let mut two = input();
        two.title = "Beneath the Iron Sky".into();
        service.add_media(two).await.expect("add two");

        let hits = service.search_media("sword").await.expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Sword of the Dawn");
    }

    #[tokio::test]
    async fn search_media_blank_query_resolves_empty() {
        let (pool, _path) = migrated_pool("media_service_search_blank.db").await;
        let service = MediaService::new(pool.clone());
        service.add_media(input()).await.expect("add media");

        assert!(service
            .search_media("   ")
            .await
            .expect("search")
            .is_empty());
    }

    #[tokio::test]
    async fn list_facets_returns_distinct_present_values() {
        let (pool, _path) = migrated_pool("media_service_facets.db").await;
        let service = MediaService::new(pool.clone());

        let mut one = input();
        one.title = "First Light".into();
        one.format = Some("light_novel".into());
        one.release_year = Some(2025);
        let mut two = input();
        two.title = "Second Wave".into();
        two.format = Some("webtoon".into());
        two.release_year = Some(2026);
        service.add_media(one).await.expect("add one");
        service.add_media(two).await.expect("add two");

        let facets = service.list_facets().await.expect("facets");
        assert_eq!(
            facets.formats,
            vec!["light_novel".to_string(), "webtoon".to_string()]
        );
        assert_eq!(facets.years, vec![2026, 2025]);
        assert!(!facets.genres.is_empty(), "seeded genres present");
        assert!(!facets.tags.is_empty(), "seeded domain tags present");
    }

    #[tokio::test]
    async fn list_media_rejects_unknown_sort() {
        let (pool, _path) = migrated_pool("media_service_list_sort.db").await;
        let service = MediaService::new(pool.clone());

        let err = service
            .list_media(MediaListInput {
                sort: Some("popularity".into()),
                ..MediaListInput::default()
            })
            .await
            .expect_err("unknown sort rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }
}
