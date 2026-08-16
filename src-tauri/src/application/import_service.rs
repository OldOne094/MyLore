//! Import-from-provider application service (MISSION-060, ARCHITECTURE §5).
//!
//! The import use-case: a user picks an external hit on the discover page and
//! the app runs **search → details → identity check → add**. The identity check
//! is re-run against the *full* details (not just the search row) so a title
//! that is already in the library — or a strong duplicate candidate — resolves
//! to the existing media instead of creating a second copy. When the title is
//! new, the media aggregate (plus its people/genre/tag/external-id links) is
//! persisted, and the provider's content-node tree (volumes→chapters,
//! seasons→episodes) is imported so tracking/progress work immediately.

use std::sync::Arc;

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::warn;
use uuid::Uuid;

use crate::application::providers::coordinator::{CancellationToken, ProviderCoordinator};
use crate::domain::identity::{self, IdentityKind};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderMedia, ProviderNode};
use crate::domain::value_objects::{ExternalId, MediaId, ProviderId, Title};
use crate::error::AppError;
use crate::infrastructure::repositories::asset as asset_repo;
use crate::infrastructure::repositories::asset::AssetRecord;
use crate::infrastructure::repositories::media as media_repo;
use crate::infrastructure::repositories::media::{AltTitle, MediaRecord};
use crate::infrastructure::repositories::node as node_repo;
use crate::infrastructure::repositories::node::NodeRecord;

/// Result of an import-from-provider call (MISSION-060).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderImportView {
    pub media_id: String,
    /// Whether a new media was created; `false` when the identity check resolved
    /// the title to an already-stored media.
    pub created: bool,
    /// `in_library` | `duplicate` | `new` — mirrors the discover identity flags.
    pub identity_kind: String,
    /// The imported (or matched) title, for display.
    pub title: String,
    pub content_type: String,
}

/// Import-from-provider use-cases.
pub struct ImportService {
    pool: SqlitePool,
    coordinator: Arc<ProviderCoordinator>,
}

impl ImportService {
    pub fn new(pool: SqlitePool, coordinator: Arc<ProviderCoordinator>) -> Self {
        Self { pool, coordinator }
    }

    /// Import one provider title: details → identity check → add media + nodes.
    ///
    /// Returns the media that ended up owning the title — a newly created one
    /// (`created: true`) or the existing library row the identity check
    /// matched (`created: false`).
    pub async fn import_from_provider(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<ProviderImportView, AppError> {
        let token = self.coordinator.token();
        let details = self
            .coordinator
            .get_details(provider, provider_id, &token)
            .await?;

        let titles = Title::new(
            details.title_main.clone(),
            details.title_original.clone(),
            details.alt_titles.clone(),
        )?;

        let mut external_ids = details.external_ids.clone();
        // The provider's own (provider, provider_id) pair is the identity an
        // import would store; add it so a previously-imported title matches
        // exactly, and drop any same-provider duplicate the provider surfaced.
        if let Ok(pid) = ProviderId::new(provider) {
            if let Ok(own) = ExternalId::new(pid, provider_id, details.url.clone()) {
                external_ids.retain(|id| id.provider() != own.provider());
                external_ids.push(own);
            }
        }

        let library = media_repo::identity_candidates(&self.pool).await?;
        if let Some(m) = identity::best_match(&titles, &external_ids, &library) {
            let kind = match m.kind {
                IdentityKind::Exact => "in_library",
                IdentityKind::TitleExact | IdentityKind::Fuzzy => "duplicate",
                IdentityKind::None => "new",
            };
            return Ok(ProviderImportView {
                media_id: m.media_id.as_str().to_string(),
                created: false,
                identity_kind: kind.to_string(),
                title: details.title_main.clone(),
                content_type: details.content_type.as_str().to_string(),
            });
        }

        let media_id = self
            .persist_media(provider, &details, &external_ids)
            .await?;

        if let Err(error) = self
            .persist_nodes(provider, provider_id, media_id.as_str(), &token)
            .await
        {
            // Best-effort enrichment: the media is already imported, so a node
            // tree failure must not fail the import. Log and surface the title.
            warn!(
                provider,
                provider_id,
                media = %media_id.as_str(),
                %error,
                "content-node tree import failed; media was still added"
            );
        }

        Ok(ProviderImportView {
            media_id: media_id.as_str().to_string(),
            created: true,
            identity_kind: "new".to_string(),
            title: details.title_main,
            content_type: details.content_type.as_str().to_string(),
        })
    }

    /// Persist the media aggregate from provider details (MISSION-060).
    async fn persist_media(
        &self,
        provider: &str,
        details: &ProviderMedia,
        external_ids: &[ExternalId],
    ) -> Result<MediaId, AppError> {
        let now = Utc::now().to_rfc3339();
        let id = MediaId::new(format!("m-{}", Uuid::new_v4()))?;

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

        let mut tags = Vec::new();
        for tag in &details.tags {
            tags.push(media_repo::ensure_domain_tag(&self.pool, tag).await?);
        }

        // Register provider cover/banner URLs as `remote` assets so the image
        // pipeline (MISSION-062) can download/cache them lazily on resolve.
        let cover_asset_id = self
            .create_asset("cover", details.cover_url.as_deref(), &now)
            .await?;
        let banner_asset_id = self
            .create_asset("banner", details.banner_url.as_deref(), &now)
            .await?;

        let own_ext_ids = external_ids
            .iter()
            .map(|id| media_repo::ExternalId {
                provider: id.provider().as_str().to_string(),
                ext_id: id.value().to_string(),
                url: id.url().map(str::to_string),
            })
            .collect();

        let record = MediaRecord {
            id: id.as_str().to_string(),
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
            cover_asset_id,
            banner_asset_id,
            provider: Some(provider.to_string()),
            provider_url: details.url.clone(),
            metadata_refreshed_at: Some(now.clone()),
            created_at: now.clone(),
            updated_at: now,
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
            external_ids: own_ext_ids,
            relations: Vec::new(),
        };

        media_repo::create(&self.pool, &record).await?;
        Ok(id)
    }

    /// Register a provider cover/banner URL as a `remote` asset row, resolving
    /// with its id. `None` URLs register nothing (MISSION-062).
    async fn create_asset(
        &self,
        kind: &str,
        remote_url: Option<&str>,
        now: &str,
    ) -> Result<Option<String>, AppError> {
        let Some(url) = remote_url else {
            return Ok(None);
        };
        let id = format!("a-{}", Uuid::new_v4());
        asset_repo::insert(
            &self.pool,
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

    /// Import the provider's content-node tree (volumes→chapters, seasons→
    /// episodes). Unsupported providers are a no-op; other errors propagate so
    /// the caller can log them as best-effort.
    async fn persist_nodes(
        &self,
        provider: &str,
        provider_id: &str,
        media_id: &str,
        token: &CancellationToken,
    ) -> Result<(), AppError> {
        let nodes = match self
            .coordinator
            .get_nodes(provider, provider_id, token)
            .await
        {
            Ok(nodes) => nodes,
            Err(ProviderError::Unsupported { .. }) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        self.insert_node_tree(&nodes, media_id, None).await
    }

    /// Insert a provider node tree, parents first (iterative pre-order so no
    /// async recursion / boxing is needed). Node ids are minted here; the
    /// provider's own node id is kept as `external_id`.
    async fn insert_node_tree(
        &self,
        nodes: &[ProviderNode],
        media_id: &str,
        parent_id: Option<&str>,
    ) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        let mut stack: Vec<(&ProviderNode, Option<String>)> = Vec::new();
        for node in nodes.iter().rev() {
            stack.push((node, parent_id.map(str::to_string)));
        }
        while let Some((node, parent)) = stack.pop() {
            let id = format!("n-{}", Uuid::new_v4());
            let record = NodeRecord {
                id: id.clone(),
                media_id: media_id.to_string(),
                parent_id: parent.clone(),
                kind: node.kind.as_str().to_string(),
                position: node.position,
                number: node.number.clone(),
                title: node.title.clone(),
                release_date: node.release_date.clone(),
                duration_min: node.duration_min,
                page_count: node.page_count,
                synopsis: node.synopsis.clone(),
                external_id: Some(node.id.clone()),
                is_special: node.is_special,
                created_at: now.clone(),
            };
            node_repo::create(&self.pool, &record).await?;
            for child in node.children.iter().rev() {
                stack.push((child, Some(id.clone())));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::providers::config::ProviderConfig;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::domain::enums::{ContentType, MediaStatus, NodeKind, PersonRole};
    use crate::domain::provider::capabilities::ProviderCapabilities;
    use crate::domain::provider::{Provider, ProviderCandidate};
    use crate::infrastructure::repositories::media as media_repo;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    #[derive(Clone)]
    enum Behavior {
        Ok(ProviderMedia, Vec<ProviderNode>),
        DetailsFail(ProviderError),
        NodesFail(ProviderError),
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
        async fn get_details(
            &self,
            _provider_id: &str,
        ) -> Result<ProviderMedia, ProviderError> {
            match self.behavior.lock().unwrap().clone() {
                Behavior::Ok(details, _) => Ok(details),
                Behavior::DetailsFail(error) => Err(error),
                Behavior::NodesFail(_) => Err(ProviderError::Unsupported {
                    provider: self.id.clone(),
                    operation: "details".into(),
                }),
            }
        }
        async fn get_nodes(
            &self,
            _provider_id: &str,
        ) -> Result<Vec<ProviderNode>, ProviderError> {
            match self.behavior.lock().unwrap().clone() {
                Behavior::Ok(_, nodes) => Ok(nodes),
                Behavior::DetailsFail(_) => Err(ProviderError::Unsupported {
                    provider: self.id.clone(),
                    operation: "nodes".into(),
                }),
                Behavior::NodesFail(error) => Err(error),
            }
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

    fn media(provider_id: &str, title: &str) -> ProviderMedia {
        ProviderMedia {
            provider: "fake".to_string(),
            provider_id: provider_id.to_string(),
            title_main: title.to_string(),
            title_original: None,
            alt_titles: vec!["Alt".to_string()],
            content_type: ContentType::Novel,
            format: Some("light_novel".to_string()),
            pub_status: MediaStatus::Ongoing,
            synopsis: Some("A synopsis.".to_string()),
            start_date: Some("2020-01-01".to_string()),
            end_date: None,
            release_year: Some(2020),
            language: Some("ja".to_string()),
            country: Some("JP".to_string()),
            content_rating: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count: Some(120),
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

    fn nodes() -> Vec<ProviderNode> {
        vec![ProviderNode {
            id: "vol-1".to_string(),
            kind: NodeKind::Volume,
            position: 1,
            number: Some("1".to_string()),
            title: Some("Volume 1".to_string()),
            release_date: None,
            duration_min: None,
            page_count: None,
            synopsis: None,
            is_special: false,
            children: vec![ProviderNode {
                id: "ch-1".to_string(),
                kind: NodeKind::Chapter,
                position: 1,
                number: Some("1".to_string()),
                title: Some("Chapter 1".to_string()),
                release_date: None,
                duration_min: None,
                page_count: Some(20),
                synopsis: None,
                is_special: false,
                children: Vec::new(),
            }],
        }]
    }

    #[tokio::test]
    async fn import_creates_media_with_links_and_node_tree() {
        let (pool, path) = migrated_pool("import_service_new.db").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(media("x1", "Sword of the Dawn"), nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(provider));

        let view = service
            .import_from_provider("fake", "x1")
            .await
            .expect("import");
        assert!(view.created);
        assert_eq!(view.identity_kind, "new");
        assert_eq!(view.title, "Sword of the Dawn");

        let stored = media_repo::get(&pool, &view.media_id)
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(stored.provider.as_deref(), Some("fake"));
        assert_eq!(stored.ch_count, Some(120));
        assert_eq!(stored.genres, vec!["fantasy".to_string()]);
        assert_eq!(stored.tags, vec!["isekai".to_string()]);
        assert_eq!(stored.alt_titles.len(), 1);
        assert_eq!(
            stored.external_ids[0],
            media_repo::ExternalId {
                provider: "fake".to_string(),
                ext_id: "x1".to_string(),
                url: Some("https://fake.test/series/x".to_string()),
            }
        );

        let people = sqlx::query_as::<_, (String, String, String)>(
            "SELECT p.id, p.name, p.role FROM person p JOIN media_person mp \
             ON mp.person_id = p.id WHERE mp.media_id = ?",
        )
        .bind(&view.media_id)
        .fetch_all(&pool)
        .await
        .expect("people");
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].1, "Test Author");
        assert_eq!(people[0].2, "author");

        let tree = node_repo::list_by_media(&pool, &view.media_id)
            .await
            .expect("nodes");
        assert_eq!(tree.len(), 2);
        let volume = tree.iter().find(|n| n.kind == "volume").expect("volume");
        assert_eq!(volume.number.as_deref(), Some("1"));
        assert_eq!(volume.external_id.as_deref(), Some("vol-1"));
        let chapter = tree.iter().find(|n| n.kind == "chapter").expect("chapter");
        assert_eq!(chapter.parent_id.as_deref(), Some(volume.id.as_str()));
        assert_eq!(chapter.page_count, Some(20));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn reimport_of_same_provider_id_resolves_to_existing() {
        let (pool, path) = migrated_pool("import_service_reimport.db").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(media("x1", "Sword of the Dawn"), nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(provider));

        let first = service
            .import_from_provider("fake", "x1")
            .await
            .expect("first import");
        assert!(first.created);

        let second = service
            .import_from_provider("fake", "x1")
            .await
            .expect("second import");
        assert!(!second.created);
        assert_eq!(second.identity_kind, "in_library");
        assert_eq!(second.media_id, first.media_id);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn title_duplicate_without_external_id_flags_duplicate() {
        let (pool, path) = migrated_pool("import_service_duplicate.db").await;
        // A library row with the same title but no fake external id.
        let library: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(media("zzz", "Sword of the Dawn"), nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(library));
        let first = service
            .import_from_provider("fake", "zzz")
            .await
            .expect("first import");

        // Incoming title matches, but the provider id differs.
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(media("x9", "Sword of the Dawn"), nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(provider));
        let second = service
            .import_from_provider("fake", "x9")
            .await
            .expect("second import");
        assert!(!second.created);
        assert_eq!(second.identity_kind, "duplicate");
        assert_eq!(second.media_id, first.media_id);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn details_failure_propagates() {
        let (pool, path) = migrated_pool("import_service_details_fail.db").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::DetailsFail(ProviderError::NotFound {
                provider: "fake".into(),
            })),
        });
        let service = ImportService::new(pool.clone(), coord(provider));
        let err = service
            .import_from_provider("fake", "missing")
            .await
            .expect_err("should fail");
        assert!(err.to_string().contains("not found on fake"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn import_registers_cover_and_banner_assets() {
        let (pool, path) = migrated_pool("import_service_assets.db").await;
        let mut m = media("x1", "Sword of the Dawn");
        m.cover_url = Some("https://cdn.example/cover.jpg".to_string());
        m.banner_url = Some("https://cdn.example/banner.jpg".to_string());
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(m, nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(provider));

        let view = service
            .import_from_provider("fake", "x1")
            .await
            .expect("import");
        assert!(view.created);

        let stored = media_repo::get(&pool, &view.media_id)
            .await
            .expect("get")
            .expect("stored");
        let cover = stored.cover_asset_id.expect("cover asset linked");
        let banner = stored.banner_asset_id.expect("banner asset linked");
        assert_ne!(cover, banner);

        let cover_row = asset_repo::get(&pool, &cover).await.expect("get").unwrap();
        assert_eq!(cover_row.kind, "cover");
        assert_eq!(
            cover_row.remote_url.as_deref(),
            Some("https://cdn.example/cover.jpg")
        );
        assert_eq!(cover_row.status, "remote", "assets await lazy resolve");

        let banner_row = asset_repo::get(&pool, &banner).await.expect("get").unwrap();
        assert_eq!(banner_row.kind, "banner");
        assert_eq!(
            banner_row.remote_url.as_deref(),
            Some("https://cdn.example/banner.jpg")
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn import_without_urls_links_no_assets() {
        let (pool, path) = migrated_pool("import_service_no_assets.db").await;
        let provider: Arc<dyn Provider> = Arc::new(FakeProvider {
            id: "fake".into(),
            behavior: Mutex::new(Behavior::Ok(media("x1", "Plain Title"), nodes())),
        });
        let service = ImportService::new(pool.clone(), coord(provider));
        let view = service
            .import_from_provider("fake", "x1")
            .await
            .expect("import");
        let stored = media_repo::get(&pool, &view.media_id)
            .await
            .expect("get")
            .expect("stored");
        assert!(stored.cover_asset_id.is_none());
        assert!(stored.banner_asset_id.is_none());
        pool.close().await;
        cleanup_files(&path);
    }
}
