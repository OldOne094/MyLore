//! Enrich metadata application service (MISSION-061, ARCHITECTURE §5).
//!
//! The enrich use-case refreshes the **provider-owned** metadata fields of a
//! library media from the provider it was imported from, and reports exactly
//! what changed. Per ADR-007 user data is a separate aggregate: tracking,
//! review, collections, personal tags and asset ids (`cover_asset_id`,
//! `banner_asset_id`) are never touched — enrichment writes only the `media`
//! row plus its metadata link sets, stamped with `metadata_refreshed_at`.

use std::sync::Arc;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::application::providers::coordinator::ProviderCoordinator;
use crate::domain::provider::types::ProviderMedia;
use crate::domain::value_objects::ExternalId as DomainExternalId;
use crate::domain::value_objects::ProviderId;
use crate::error::AppError;
use crate::infrastructure::repositories::media as media_repo;
use crate::infrastructure::repositories::media::{AltTitle, MediaRecord};

/// One changed provider-owned field, before → after (MISSION-061 diff).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichChange {
    /// Stable field key: `title_main`, `synopsis`, `genres`, `people`, … —
    /// the frontend maps these onto display labels.
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Result of an enrichment (MISSION-061).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichView {
    pub media_id: String,
    /// The provider the refresh consulted (e.g. `anilist`).
    pub provider: String,
    /// When the provider was last consulted (`metadata_refreshed_at`).
    pub refreshed_at: String,
    /// Whether any provider-owned field actually changed.
    pub changed: bool,
    /// Per-field before/after diffs (`changed` implies at least one entry).
    pub changes: Vec<EnrichChange>,
}

/// Enrich metadata use-cases.
pub struct EnrichService {
    pool: SqlitePool,
    coordinator: Arc<ProviderCoordinator>,
}

impl EnrichService {
    pub fn new(pool: SqlitePool, coordinator: Arc<ProviderCoordinator>) -> Self {
        Self { pool, coordinator }
    }

    /// Refresh one media's provider-owned metadata from its provider and report
    /// the diff. The media must have been imported from a provider (its own
    /// `(provider, provider_id)` external id is the identity consulted).
    ///
    /// When nothing changed, only `metadata_refreshed_at` is stamped (the
    /// provider was consulted) — `updated_at` is left alone so a no-op refresh
    /// never churns "recently updated" ordering.
    pub async fn enrich_from_provider(&self, media_id: &str) -> Result<EnrichView, AppError> {
        let stored = media_repo::get(&self.pool, media_id)
            .await?
            .ok_or_else(|| AppError::validation("media not found"))?;

        let provider = stored
            .provider
            .clone()
            .ok_or_else(|| AppError::validation("media is not linked to a provider"))?;
        let provider_id = stored
            .external_ids
            .iter()
            .find(|id| id.provider == provider)
            .map(|id| id.ext_id.clone())
            .ok_or_else(|| AppError::validation("media has no provider external id"))?;

        let token = self.coordinator.token();
        let details = self
            .coordinator
            .get_details(&provider, &provider_id, &token)
            .await?;

        let changes = self.diff(&stored, &provider, &details).await?;

        let refreshed_at = Utc::now().to_rfc3339();
        if changes.is_empty() {
            media_repo::stamp_metadata_refreshed(&self.pool, media_id, &refreshed_at).await?;
        } else {
            let fresh = self
                .build_fresh(&stored, &provider, &provider_id, &details, &refreshed_at)
                .await?;
            media_repo::update(&self.pool, &fresh).await?;
        }

        Ok(EnrichView {
            media_id: media_id.to_string(),
            provider,
            refreshed_at,
            changed: !changes.is_empty(),
            changes,
        })
    }

    /// Build the refreshed record: provider-owned fields from `details`,
    /// user-owned fields preserved from `stored` (assets, personal tags,
    /// relations, created_at).
    async fn build_fresh(
        &self,
        stored: &MediaRecord,
        provider: &str,
        provider_id: &str,
        details: &ProviderMedia,
        refreshed_at: &str,
    ) -> Result<MediaRecord, AppError> {
        let mut people = Vec::new();
        for person in &details.people {
            people.push(
                media_repo::ensure_person(&self.pool, &person.name, person.role.as_str()).await?,
            );
        }

        let mut genres = Vec::new();
        for genre in &details.genres {
            genres.push(media_repo::ensure_genre(&self.pool, genre).await?);
        }

        // Domain tags are provider-owned and refreshed; personal tags are
        // user-owned and must survive the wholesale link rewrite.
        let mut tags = media_repo::personal_tag_ids(&self.pool, &stored.id).await?;
        for tag in &details.tags {
            tags.push(media_repo::ensure_domain_tag(&self.pool, tag).await?);
        }

        let mut external_ids = Vec::new();
        if let Ok(pid) = ProviderId::new(provider) {
            if let Ok(own) =
                DomainExternalId::new(pid, provider_id.to_string(), details.url.clone())
            {
                external_ids.push(media_repo::ExternalId {
                    provider: own.provider().as_str().to_string(),
                    ext_id: own.value().to_string(),
                    url: own.url().map(str::to_string),
                });
            }
        }
        for ext in &details.external_ids {
            if ext.provider().as_str() == provider {
                continue;
            }
            external_ids.push(media_repo::ExternalId {
                provider: ext.provider().as_str().to_string(),
                ext_id: ext.value().to_string(),
                url: ext.url().map(str::to_string),
            });
        }

        Ok(MediaRecord {
            id: stored.id.clone(),
            content_type: details.content_type.as_str().to_string(),
            format: details.format.clone(),
            title_main: details.title_main.clone(),
            title_original: details.title_original.clone(),
            synopsis: details.synopsis.clone(),
            pub_status: details.pub_status.as_str().to_string(),
            start_date: details.start_date.clone(),
            end_date: details.end_date.clone(),
            release_year: details.release_year.map(Into::into),
            language: details.language.clone(),
            country: details.country.clone(),
            content_rating: details.content_rating.clone(),
            pages: details.pages.map(Into::into),
            duration_min: details.duration_min.map(Into::into),
            ep_count: details.ep_count.map(Into::into),
            ch_count: details.ch_count.map(Into::into),
            cover_asset_id: stored.cover_asset_id.clone(),
            banner_asset_id: stored.banner_asset_id.clone(),
            provider: Some(provider.to_string()),
            provider_url: details.url.clone(),
            metadata_refreshed_at: Some(refreshed_at.to_string()),
            created_at: stored.created_at.clone(),
            updated_at: refreshed_at.to_string(),
            alt_titles: details
                .alt_titles
                .iter()
                .map(|title| AltTitle {
                    lang: String::new(),
                    title: title.clone(),
                })
                .collect(),
            people,
            genres,
            tags,
            external_ids,
            relations: stored.relations.clone(),
        })
    }

    /// Compute the before/after diff between the stored record and fresh
    /// provider details. Only provider-owned fields are compared; personal
    /// tags and asset ids never appear here.
    async fn diff(
        &self,
        stored: &MediaRecord,
        provider: &str,
        details: &ProviderMedia,
    ) -> Result<Vec<EnrichChange>, AppError> {
        let mut changes = Vec::new();

        push_scalar(
            &mut changes,
            "content_type",
            Some(stored.content_type.clone()),
            Some(details.content_type.as_str().to_string()),
        );
        push_scalar(
            &mut changes,
            "format",
            stored.format.clone(),
            details.format.clone(),
        );
        push_scalar(
            &mut changes,
            "title_main",
            Some(stored.title_main.clone()),
            Some(details.title_main.clone()),
        );
        push_scalar(
            &mut changes,
            "title_original",
            stored.title_original.clone(),
            details.title_original.clone(),
        );
        push_scalar(
            &mut changes,
            "synopsis",
            stored.synopsis.clone(),
            details.synopsis.clone(),
        );
        push_scalar(
            &mut changes,
            "pub_status",
            Some(stored.pub_status.clone()),
            Some(details.pub_status.as_str().to_string()),
        );
        push_scalar(
            &mut changes,
            "start_date",
            stored.start_date.clone(),
            details.start_date.clone(),
        );
        push_scalar(
            &mut changes,
            "end_date",
            stored.end_date.clone(),
            details.end_date.clone(),
        );
        push_num(
            &mut changes,
            "release_year",
            stored.release_year,
            details.release_year.map(Into::into),
        );
        push_scalar(
            &mut changes,
            "language",
            stored.language.clone(),
            details.language.clone(),
        );
        push_scalar(
            &mut changes,
            "country",
            stored.country.clone(),
            details.country.clone(),
        );
        push_scalar(
            &mut changes,
            "content_rating",
            stored.content_rating.clone(),
            details.content_rating.clone(),
        );
        push_num(
            &mut changes,
            "pages",
            stored.pages,
            details.pages.map(Into::into),
        );
        push_num(
            &mut changes,
            "duration_min",
            stored.duration_min,
            details.duration_min.map(Into::into),
        );
        push_num(
            &mut changes,
            "ep_count",
            stored.ep_count,
            details.ep_count.map(Into::into),
        );
        push_num(
            &mut changes,
            "ch_count",
            stored.ch_count,
            details.ch_count.map(Into::into),
        );
        push_scalar(
            &mut changes,
            "provider_url",
            stored.provider_url.clone(),
            details.url.clone(),
        );

        let before_alt: Vec<String> = stored.alt_titles.iter().map(|a| a.title.clone()).collect();
        push_set(
            &mut changes,
            "alt_titles",
            before_alt,
            details.alt_titles.clone(),
        );

        let before_people = self.people_labels(&stored.people).await?;
        let after_people: Vec<String> = details
            .people
            .iter()
            .map(|p| format!("{} ({})", p.name, p.role.as_str()))
            .collect();
        push_set(&mut changes, "people", before_people, after_people);

        let before_genres = self.genre_labels(&stored.genres).await?;
        push_set(
            &mut changes,
            "genres",
            before_genres,
            details.genres.clone(),
        );

        let before_tags = self.domain_tag_labels(&stored.tags).await?;
        push_set(&mut changes, "tags", before_tags, details.tags.clone());

        let before_ext: Vec<String> = stored
            .external_ids
            .iter()
            .map(|e| format!("{}:{}", e.provider, e.ext_id))
            .collect();
        let after_ext = external_id_labels(provider, details);
        push_set(&mut changes, "external_ids", before_ext, after_ext);

        Ok(changes)
    }

    /// Resolve stored person ids to "Name (role)" labels.
    async fn people_labels(&self, ids: &[String]) -> Result<Vec<String>, AppError> {
        let info = media_repo::person_info(&self.pool, ids).await?;
        Ok(ids
            .iter()
            .filter_map(|id| info.get(id).map(|(name, role)| format!("{name} ({role})")))
            .collect())
    }

    /// Resolve stored genre ids to names.
    async fn genre_labels(&self, ids: &[String]) -> Result<Vec<String>, AppError> {
        let names = media_repo::genre_names(&self.pool, ids).await?;
        Ok(ids.iter().filter_map(|id| names.get(id).cloned()).collect())
    }

    /// Resolve stored tag ids to domain-tag names (personal tags are user data
    /// and excluded from the provider diff).
    async fn domain_tag_labels(&self, ids: &[String]) -> Result<Vec<String>, AppError> {
        let info = media_repo::tag_info(&self.pool, ids).await?;
        Ok(ids
            .iter()
            .filter_map(|id| {
                let (name, scope) = info.get(id)?;
                (scope == "domain").then(|| name.clone())
            })
            .collect())
    }
}

/// The external-id set a refresh would store: the own `(provider, provider_id)`
/// pair plus the provider-surfaced ids (own provider skipped to avoid dupes).
fn external_id_labels(provider: &str, details: &ProviderMedia) -> Vec<String> {
    let mut labels = vec![format!("{provider}:{}", details.provider_id)];
    for ext in &details.external_ids {
        if ext.provider().as_str() != provider {
            labels.push(format!("{}:{}", ext.provider().as_str(), ext.value()));
        }
    }
    labels
}

fn push_scalar(
    changes: &mut Vec<EnrichChange>,
    field: &str,
    before: Option<String>,
    after: Option<String>,
) {
    if before != after {
        changes.push(EnrichChange {
            field: field.to_string(),
            before,
            after,
        });
    }
}

fn push_num(changes: &mut Vec<EnrichChange>, field: &str, before: Option<i64>, after: Option<i64>) {
    push_scalar(
        changes,
        field,
        before.map(|v| v.to_string()),
        after.map(|v| v.to_string()),
    );
}

fn push_set(changes: &mut Vec<EnrichChange>, field: &str, before: Vec<String>, after: Vec<String>) {
    let mut before = before;
    let mut after = after;
    before.sort();
    after.sort();
    push_scalar(
        changes,
        field,
        Some(before.join(", ")),
        Some(after.join(", ")),
    );
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::providers::config::ProviderConfig;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::domain::enums::{ContentType, MediaStatus, PersonRole};
    use crate::domain::provider::capabilities::ProviderCapabilities;
    use crate::domain::provider::error::ProviderError;
    use crate::domain::provider::{Provider, ProviderCandidate};
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    #[derive(Clone)]
    enum Behavior {
        Ok(Box<ProviderMedia>),
        Fail(ProviderError),
    }

    struct FakeProvider {
        id: String,
        behavior: Mutex<Behavior>,
    }

    #[async_trait::async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn name(&self) -> &str {
            "Fake"
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            static CAPS: ProviderCapabilities = ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: crate::domain::provider::AuthKind::None,
            };
            &CAPS
        }
        async fn search(
            &self,
            _query: &str,
            _content_type: Option<ContentType>,
        ) -> Result<Vec<ProviderCandidate>, ProviderError> {
            Ok(Vec::new())
        }
        async fn get_details(&self, _provider_id: &str) -> Result<ProviderMedia, ProviderError> {
            match self.behavior.lock().unwrap().clone() {
                Behavior::Ok(details) => Ok(*details),
                Behavior::Fail(error) => Err(error),
            }
        }
        async fn get_nodes(
            &self,
            _provider_id: &str,
        ) -> Result<Vec<crate::domain::provider::types::ProviderNode>, ProviderError> {
            Ok(Vec::new())
        }
    }

    fn coord(provider: Arc<dyn Provider>) -> Arc<ProviderCoordinator> {
        Arc::new(
            ProviderCoordinator::new(vec![(
                ProviderConfig::new("fake").with_requests_per_sec(0.0),
                provider,
            )])
            .unwrap(),
        )
    }

    fn details(
        provider_id: &str,
        title: &str,
        ch_count: Option<u32>,
        synopsis: Option<String>,
    ) -> ProviderMedia {
        ProviderMedia {
            provider: "fake".to_string(),
            provider_id: provider_id.to_string(),
            title_main: title.to_string(),
            title_original: None,
            alt_titles: vec!["Alt".to_string()],
            content_type: ContentType::Novel,
            format: Some("light_novel".to_string()),
            pub_status: MediaStatus::Ongoing,
            synopsis,
            start_date: Some("2020-01-01".to_string()),
            end_date: None,
            release_year: Some(2020),
            language: Some("ja".to_string()),
            country: Some("JP".to_string()),
            content_rating: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count,
            cover_url: None,
            banner_url: None,
            url: Some("https://fake.test/series/x".to_string()),
            people: vec![crate::domain::provider::types::ProviderPerson {
                role: PersonRole::Author,
                name: "Test Author".to_string(),
            }],
            genres: vec!["Fantasy".to_string()],
            tags: vec!["Isekai".to_string()],
            external_ids: Vec::new(),
        }
    }

    /// Seed a media as if imported from `fake` (MISSION-060 import shape).
    async fn seed_media(pool: &sqlx::SqlitePool, id: &str, title: &str) {
        let author = media_repo::ensure_person(pool, "Test Author", "author")
            .await
            .unwrap();
        let genre = media_repo::ensure_genre(pool, "Fantasy").await.unwrap();
        let tag = media_repo::ensure_domain_tag(pool, "Isekai").await.unwrap();
        media_repo::create(
            pool,
            &MediaRecord {
                id: id.to_string(),
                content_type: "novel".to_string(),
                format: Some("light_novel".to_string()),
                title_main: title.to_string(),
                title_original: None,
                synopsis: Some("Old synopsis.".to_string()),
                pub_status: "ongoing".to_string(),
                start_date: Some("2020-01-01".to_string()),
                end_date: None,
                release_year: Some(2020),
                language: Some("ja".to_string()),
                country: Some("JP".to_string()),
                content_rating: None,
                pages: None,
                duration_min: None,
                ep_count: None,
                ch_count: Some(100),
                cover_asset_id: None,
                banner_asset_id: None,
                provider: Some("fake".to_string()),
                provider_url: Some("https://fake.test/series/x".to_string()),
                metadata_refreshed_at: None,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
                alt_titles: vec![AltTitle {
                    lang: String::new(),
                    title: "Alt".to_string(),
                }],
                people: vec![author],
                genres: vec![genre],
                tags: vec![tag],
                external_ids: vec![media_repo::ExternalId {
                    provider: "fake".to_string(),
                    ext_id: "x1".to_string(),
                    url: Some("https://fake.test/series/x".to_string()),
                }],
                relations: Vec::new(),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn refresh_updates_provider_fields_and_reports_diff() {
        let (pool, path) = migrated_pool("enrich_changed.db").await;
        seed_media(&pool, "m-1", "Sword of the Dawn").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(Box::new(details(
                "x1",
                "Sword of the Dawn",
                Some(120),
                Some("A fresh synopsis.".to_string()),
            )))),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));

        let view = service.enrich_from_provider("m-1").await.expect("enrich");
        assert!(view.changed);
        assert_eq!(view.provider, "fake");
        let fields: Vec<&str> = view.changes.iter().map(|c| c.field.as_str()).collect();
        assert!(fields.contains(&"synopsis"), "synopsis diff: {fields:?}");
        assert!(fields.contains(&"ch_count"), "ch_count diff: {fields:?}");

        let stored = media_repo::get(&pool, "m-1").await.unwrap().unwrap();
        assert_eq!(stored.synopsis.as_deref(), Some("A fresh synopsis."));
        assert_eq!(stored.ch_count, Some(120));
        assert!(stored.metadata_refreshed_at.is_some());
        assert_eq!(
            stored.external_ids[0],
            media_repo::ExternalId {
                provider: "fake".to_string(),
                ext_id: "x1".to_string(),
                url: Some("https://fake.test/series/x".to_string()),
            },
            "own external id preserved"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn noop_refresh_stamps_only_metadata_refreshed_at() {
        let (pool, path) = migrated_pool("enrich_noop.db").await;
        seed_media(&pool, "m-1", "Sword of the Dawn").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(Box::new(details(
                "x1",
                "Sword of the Dawn",
                Some(100),
                Some("Old synopsis.".to_string()),
            )))),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));

        let view = service.enrich_from_provider("m-1").await.expect("enrich");
        assert!(!view.changed);
        assert!(view.changes.is_empty());

        let stored = media_repo::get(&pool, "m-1").await.unwrap().unwrap();
        assert!(stored.metadata_refreshed_at.is_some(), "stamped");
        assert_eq!(stored.updated_at, "2026-01-01", "updated_at untouched");
        assert_eq!(stored.ch_count, Some(100), "fields untouched");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn personal_tags_and_assets_survive_refresh() {
        let (pool, path) = migrated_pool("enrich_preserve.db").await;
        seed_media(&pool, "m-1", "Sword of the Dawn").await;
        media_repo::create_personal_tag(&pool, "pt-1", "Backlog")
            .await
            .unwrap();
        media_repo::add_tag_to_many(&pool, "pt-1", &["m-1".to_string()])
            .await
            .unwrap();
        sqlx::query("INSERT INTO asset (id, kind, status, created_at) VALUES ('a-9', 'cover', 'remote', '2026-01-01')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE media SET cover_asset_id = 'a-9' WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        // Provider now reports a changed synopsis (so an update happens) but the
        // same domain tag set.
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(Box::new(details(
                "x1",
                "Sword of the Dawn",
                Some(120),
                Some("A fresh synopsis.".to_string()),
            )))),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));
        let view = service.enrich_from_provider("m-1").await.expect("enrich");
        assert!(view.changed);

        let stored = media_repo::get(&pool, "m-1").await.unwrap().unwrap();
        assert_eq!(stored.cover_asset_id.as_deref(), Some("a-9"), "asset kept");
        assert!(
            stored.tags.contains(&"pt-1".to_string()),
            "personal tag kept"
        );
        assert!(
            stored.tags.contains(&"isekai".to_string()),
            "domain tag kept"
        );

        // User data aggregates are untouched by the media rewrite.
        for (sql, name) in [
            ("SELECT COUNT(*) FROM tracking", "tracking"),
            ("SELECT COUNT(*) FROM review", "review"),
            ("SELECT COUNT(*) FROM collection", "collection"),
            (
                "SELECT COUNT(*) FROM collection_member",
                "collection_member",
            ),
        ] {
            let (n,): (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.unwrap();
            assert_eq!(n, 0, "{name} untouched");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_without_provider_is_rejected() {
        let (pool, path) = migrated_pool("enrich_no_provider.db").await;
        // A manual media (no provider, no external ids).
        let record = MediaRecord {
            id: "m-1".to_string(),
            content_type: "novel".to_string(),
            format: None,
            title_main: "Hand Made".to_string(),
            title_original: None,
            synopsis: None,
            pub_status: "unknown".to_string(),
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
            provider: None,
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
        };
        media_repo::create(&pool, &record).await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(Box::new(details("x1", "X", None, None)))),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));

        let err = service
            .enrich_from_provider("m-1")
            .await
            .expect_err("no provider rejected");
        assert!(err.to_string().contains("not linked to a provider"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn provider_failure_propagates() {
        let (pool, path) = migrated_pool("enrich_fail.db").await;
        seed_media(&pool, "m-1", "Sword of the Dawn").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Fail(ProviderError::NotFound {
                provider: "fake".into(),
            })),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));

        let err = service
            .enrich_from_provider("m-1")
            .await
            .expect_err("should fail");
        assert!(err.to_string().contains("not found on fake"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn domain_tags_refresh_but_personal_tags_are_not_in_diff() {
        let (pool, path) = migrated_pool("enrich_tag_diff.db").await;
        seed_media(&pool, "m-1", "Sword of the Dawn").await;
        media_repo::create_personal_tag(&pool, "pt-1", "Backlog")
            .await
            .unwrap();
        media_repo::add_tag_to_many(&pool, "pt-1", &["m-1".to_string()])
            .await
            .unwrap();

        // Provider now reports an extra domain tag; personal tag must stay out
        // of the diff and be preserved.
        let mut updated = details("x1", "Sword of the Dawn", Some(100), None);
        updated.tags = vec!["Isekai".to_string(), "Rising Action".to_string()];
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(Box::new(updated))),
        });
        let service = EnrichService::new(pool.clone(), coord(provider));

        let view = service.enrich_from_provider("m-1").await.expect("enrich");
        assert!(view.changed);
        let tag_change = view
            .changes
            .iter()
            .find(|c| c.field == "tags")
            .expect("tags diff");
        assert!(
            !tag_change.before.as_deref().unwrap().contains("Backlog"),
            "personal tag excluded from before: {}",
            tag_change.before.as_deref().unwrap()
        );
        assert!(
            tag_change
                .after
                .as_deref()
                .unwrap()
                .contains("Rising Action"),
            "new domain tag reported: {}",
            tag_change.after.as_deref().unwrap()
        );

        let stored = media_repo::get(&pool, "m-1").await.unwrap().unwrap();
        assert!(stored.tags.contains(&"pt-1".to_string()), "personal kept");
        let rising = media_repo::ensure_domain_tag(&pool, "Rising Action")
            .await
            .unwrap();
        assert!(stored.tags.contains(&rising), "new domain tag persisted");

        pool.close().await;
        cleanup_files(&path);
    }
}
