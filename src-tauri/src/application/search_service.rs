//! External search application service (MISSION-059, ARCHITECTURE §5).
//!
//! The combined search use-case: local full-text hits plus provider hits, with
//! every external hit passed through the identity service so the UI can flag it
//! as **already in library** (definitive external-id match), **duplicate
//! candidate** (title-exact or fuzzy) or **new**. Hits are grouped by provider
//! in registration order; per-provider failures are surfaced separately so one
//! down provider never masks the rest.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::application::media_service::{MediaListItem, MediaService};
use crate::application::providers::coordinator::ProviderCoordinator;
use crate::domain::enums::ContentType;
use crate::domain::identity::{self, IdentityCandidate, IdentityKind};
use crate::domain::provider::types::ProviderCandidate;
use crate::domain::value_objects::{ExternalId, ProviderId, Title};
use crate::error::AppError;
use crate::infrastructure::repositories::media as media_repo;

/// Combined result of a discover search (ARCHITECTURE §5 `{ local, external }`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExternalSearchView {
    /// Local full-text hits for the same query.
    pub local: Vec<MediaListItem>,
    /// External hits grouped by provider, in registration order.
    pub groups: Vec<ProviderGroup>,
    /// Providers that errored during the fan-out (partial results shown).
    pub failures: Vec<ProviderFailureView>,
}

/// One provider's contribution to the external results.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderGroup {
    /// Provider id (e.g. `anilist`).
    pub provider: String,
    /// Human-readable provider name for the group header.
    pub name: String,
    pub hits: Vec<ExternalHit>,
}

/// One normalized provider hit, with its identity flag.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExternalHit {
    pub provider: String,
    pub provider_id: String,
    pub title: String,
    pub content_type: String,
    pub release_year: Option<i64>,
    pub cover_url: Option<String>,
    pub synopsis: Option<String>,
    pub url: Option<String>,
    pub identity: IdentityFlagView,
}

/// How an external hit relates to the library (MISSION-059 flags).
#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityFlagView {
    /// `in_library` | `duplicate` | `new`.
    pub kind: String,
    /// The matched library media, when `in_library` or `duplicate`.
    pub media_id: Option<String>,
    /// The identity match score, when matched (1.0 exact / 0.95 title-exact).
    pub score: Option<f64>,
}

/// A provider that failed during the external fan-out.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderFailureView {
    pub provider: String,
    pub message: String,
}

/// External search use-cases.
pub struct SearchService {
    pool: SqlitePool,
    coordinator: Arc<ProviderCoordinator>,
}

impl SearchService {
    pub fn new(pool: SqlitePool, coordinator: Arc<ProviderCoordinator>) -> Self {
        Self { pool, coordinator }
    }

    /// Run a discover search: local FTS + provider fan-out + identity flags.
    /// Blank queries resolve to an empty view without hitting any provider.
    pub async fn search_external(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<ExternalSearchView, AppError> {
        if query.trim().is_empty() {
            return Ok(ExternalSearchView {
                local: Vec::new(),
                groups: Vec::new(),
                failures: Vec::new(),
            });
        }

        let local = MediaService::new(self.pool.clone())
            .search_media(query)
            .await?;

        let library = media_repo::identity_candidates(&self.pool).await?;

        let token = self.coordinator.token();
        let outcome = self
            .coordinator
            .search_all(query, content_type, &token)
            .await;

        // Names + registration order for stable group ordering.
        let order: Vec<(String, String)> = self
            .coordinator
            .providers()
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();

        let mut by_provider: HashMap<String, Vec<ExternalHit>> = HashMap::new();
        for hit in outcome.hits {
            let flag = tag_identity(&hit.provider, &hit.candidate, &library);
            by_provider
                .entry(hit.provider.clone())
                .or_default()
                .push(external_hit(hit.provider, hit.candidate, flag));
        }

        let groups = order
            .into_iter()
            .filter_map(|(provider, name)| {
                by_provider.remove(&provider).map(|hits| ProviderGroup {
                    provider,
                    name,
                    hits,
                })
            })
            .collect();

        let failures = outcome
            .failures
            .into_iter()
            .map(|f| ProviderFailureView {
                provider: f.provider,
                message: f.error.to_string(),
            })
            .collect();

        Ok(ExternalSearchView {
            local,
            groups,
            failures,
        })
    }
}

/// Map one provider hit onto its serializable DTO.
fn external_hit(
    provider: String,
    candidate: ProviderCandidate,
    identity: IdentityFlagView,
) -> ExternalHit {
    ExternalHit {
        provider,
        provider_id: candidate.provider_id,
        title: candidate.title,
        content_type: candidate.content_type.as_str().to_string(),
        release_year: candidate.release_year.map(Into::into),
        cover_url: candidate.cover_url,
        synopsis: candidate.synopsis,
        url: candidate.url,
        identity,
    }
}

/// Run the identity service against a provider hit. The hit's own
/// (provider, provider_id) pair is treated as an external id (that's what an
/// import would store), so a title that was already imported from the same
/// provider resolves to an **exact** in-library flag.
fn tag_identity(
    provider: &str,
    candidate: &ProviderCandidate,
    library: &[IdentityCandidate],
) -> IdentityFlagView {
    let Ok(titles) = Title::new(candidate.title.clone(), None, Vec::new()) else {
        return IdentityFlagView {
            kind: "new".to_string(),
            media_id: None,
            score: None,
        };
    };
    let mut external_ids = candidate.external_ids.clone();
    // The hit's own (provider, provider_id) pair is an external id an import
    // would store, so a previously-imported title matches exactly. Skip when
    // the provider id fails validation (can't happen for registered adapters).
    if let Ok(own) = ProviderId::new(provider)
        .and_then(|pid| ExternalId::new(pid, candidate.provider_id.clone(), candidate.url.clone()))
    {
        external_ids.push(own);
    }

    match identity::best_match(&titles, &external_ids, library) {
        Some(m) => {
            let kind = match m.kind {
                IdentityKind::Exact => "in_library",
                IdentityKind::TitleExact | IdentityKind::Fuzzy => "duplicate",
                IdentityKind::None => "new",
            };
            IdentityFlagView {
                kind: kind.to_string(),
                media_id: Some(m.media_id.as_str().to_string()),
                score: Some(m.score),
            }
        }
        None => IdentityFlagView {
            kind: "new".to_string(),
            media_id: None,
            score: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::providers::config::ProviderConfig;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::domain::enums::ContentType;
    use crate::domain::provider::capabilities::ProviderCapabilities;
    use crate::domain::provider::error::ProviderError;
    use crate::domain::provider::{Provider, ProviderCandidate};
    use crate::infrastructure::repositories::media::{
        self, ExternalId as RepoExternalId, MediaRecord,
    };
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn fake_media(id: &str, title: &str, ext: Option<(&str, &str)>) -> MediaRecord {
        let mut record = MediaRecord {
            id: id.to_string(),
            content_type: "novel".to_string(),
            format: Some("light_novel".to_string()),
            title_main: title.to_string(),
            title_original: None,
            synopsis: None,
            pub_status: "unknown".to_string(),
            start_date: None,
            end_date: None,
            release_year: Some(2025),
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
        if let Some((provider, ext_id)) = ext {
            record.external_ids.push(RepoExternalId {
                provider: provider.to_string(),
                ext_id: ext_id.to_string(),
                url: None,
            });
        }
        record
    }

    fn candidate(provider_id: &str, title: &str, ext: &[(&str, &str)]) -> ProviderCandidate {
        ProviderCandidate {
            provider: "anilist".to_string(),
            provider_id: provider_id.to_string(),
            title: title.to_string(),
            content_type: ContentType::Anime,
            release_year: Some(2011),
            cover_url: Some("https://s4.anilist.co/cover.png".to_string()),
            synopsis: Some("A synopsis".to_string()),
            external_ids: ext
                .iter()
                .map(|(provider, value)| {
                    crate::domain::value_objects::ExternalId::new(
                        crate::domain::value_objects::ProviderId::new(*provider).unwrap(),
                        *value,
                        None,
                    )
                    .unwrap()
                })
                .collect(),
            url: Some("https://anilist.co/anime/21".to_string()),
        }
    }

    #[derive(Clone)]
    enum Behavior {
        Ok(Vec<ProviderCandidate>),
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
                details: false,
                nodes: false,
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
            match self.behavior.lock().unwrap().clone() {
                Behavior::Ok(hits) => Ok(hits),
                Behavior::Fail(error) => Err(error),
            }
        }
        async fn get_details(
            &self,
            _id: &str,
        ) -> Result<crate::domain::provider::ProviderMedia, ProviderError> {
            Err(ProviderError::Unsupported {
                provider: self.id.clone(),
                operation: "details".into(),
            })
        }
    }

    fn coord(pairs: Vec<(ProviderConfig, Arc<dyn Provider>)>) -> Arc<ProviderCoordinator> {
        Arc::new(ProviderCoordinator::new(pairs).unwrap())
    }

    fn anilist_config() -> ProviderConfig {
        ProviderConfig::new("anilist").with_requests_per_sec(0.0)
    }

    #[tokio::test]
    async fn blank_query_returns_empty_view_without_providers() {
        let (pool, path) = migrated_pool("search_service_blank.db").await;
        let service = SearchService::new(pool.clone(), coord(vec![]));
        let view = service
            .search_external("   ", None)
            .await
            .expect("blank resolves");
        assert!(view.local.is_empty());
        assert!(view.groups.is_empty());
        assert!(view.failures.is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn exact_external_id_flags_in_library() {
        let (pool, path) = migrated_pool("search_service_exact.db").await;
        media::create(
            &pool,
            &fake_media("m-1", "Hunter x Hunter", Some(("anilist", "21"))),
        )
        .await
        .expect("seed library");

        let fake = Arc::new(FakeProvider {
            id: "anilist".into(),
            behavior: Mutex::new(Behavior::Ok(vec![candidate("21", "Hunter x Hunter", &[])])),
        });
        let service = SearchService::new(pool.clone(), coord(vec![(anilist_config(), fake)]));

        let view = service
            .search_external("hunter", None)
            .await
            .expect("search");
        assert_eq!(view.groups.len(), 1);
        assert_eq!(view.groups[0].provider, "anilist");
        let hit = &view.groups[0].hits[0];
        assert_eq!(hit.identity.kind, "in_library");
        assert_eq!(hit.identity.media_id.as_deref(), Some("m-1"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn title_match_flags_duplicate() {
        let (pool, path) = migrated_pool("search_service_duplicate.db").await;
        media::create(&pool, &fake_media("m-1", "Attack on Titan", None))
            .await
            .expect("seed library");

        let fake = Arc::new(FakeProvider {
            id: "anilist".into(),
            behavior: Mutex::new(Behavior::Ok(vec![candidate(
                "16498",
                "Attack on Titan",
                &[],
            )])),
        });
        let service = SearchService::new(pool.clone(), coord(vec![(anilist_config(), fake)]));

        let view = service
            .search_external("attack", None)
            .await
            .expect("search");
        let hit = &view.groups[0].hits[0];
        assert_eq!(hit.identity.kind, "duplicate");
        assert_eq!(hit.identity.media_id.as_deref(), Some("m-1"));
        assert!(hit.identity.score.unwrap() > 0.5);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn unmatched_hit_is_new() {
        let (pool, path) = migrated_pool("search_service_new.db").await;
        let fake = Arc::new(FakeProvider {
            id: "anilist".into(),
            behavior: Mutex::new(Behavior::Ok(vec![candidate("999", "Something Else", &[])])),
        });
        let service = SearchService::new(pool.clone(), coord(vec![(anilist_config(), fake)]));

        let view = service
            .search_external("something", None)
            .await
            .expect("search");
        let hit = &view.groups[0].hits[0];
        assert_eq!(hit.identity.kind, "new");
        assert!(hit.identity.media_id.is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn failures_are_surfaced_alongside_hits() {
        let (pool, path) = migrated_pool("search_service_failures.db").await;
        let ok = Arc::new(FakeProvider {
            id: "anilist".into(),
            behavior: Mutex::new(Behavior::Ok(vec![candidate("21", "Hunter x Hunter", &[])])),
        });
        let failing = Arc::new(FakeProvider {
            id: "tmdb".into(),
            behavior: Mutex::new(Behavior::Fail(ProviderError::ProviderDown {
                provider: "tmdb".into(),
                status: Some(503),
            })),
        });
        let service = SearchService::new(
            pool.clone(),
            coord(vec![
                (anilist_config(), ok),
                (
                    ProviderConfig::new("tmdb").with_requests_per_sec(0.0),
                    failing,
                ),
            ]),
        );

        let view = service
            .search_external("hunter", None)
            .await
            .expect("search");
        assert_eq!(view.groups.len(), 1);
        assert_eq!(view.failures.len(), 1);
        assert_eq!(view.failures[0].provider, "tmdb");
        assert!(!view.failures[0].message.is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn local_hits_and_provider_groups_coexist() {
        let (pool, path) = migrated_pool("search_service_local.db").await;
        media::create(&pool, &fake_media("m-1", "Sword of the Dawn", None))
            .await
            .expect("seed library");
        let fake = Arc::new(FakeProvider {
            id: "anilist".into(),
            behavior: Mutex::new(Behavior::Ok(vec![candidate("21", "Hunter x Hunter", &[])])),
        });
        let service = SearchService::new(pool.clone(), coord(vec![(anilist_config(), fake)]));

        let view = service
            .search_external("sword", None)
            .await
            .expect("search");
        assert_eq!(view.local.len(), 1);
        assert_eq!(view.local[0].title, "Sword of the Dawn");
        assert_eq!(view.groups.len(), 1);
        pool.close().await;
        cleanup_files(&path);
    }
}
