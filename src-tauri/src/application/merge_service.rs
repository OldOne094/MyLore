//! Merge application service (MISSION-089).
//!
//! Wires MISSION-028's merge policy to the database. Two phases, both
//! surfaced over IPC:
//!   - **plan** — loads both aggregates and produces a preview: the field
//!     conflicts (different non-empty values), what will move (nodes,
//!     review/tracking when the survivor lacks one, collection memberships)
//!     and the merged title.
//!   - **apply** — snapshots the duplicate into `trash` (kind `merge`) with
//!     everything the undo needs — the full record, its node ids, whether
//!     review/tracking were moved and which collections it belonged to — then
//!     updates the survivor, re-keys the duplicate's rows onto it and deletes
//!     the duplicate row (leftover cascades). Restoring that trash entry
//!     (handled by `TrashService`) reverses the whole merge.
//!
//! The policy mirrors `domain::merge::plan_merge` but operates on repository
//! `MediaRecord`s directly, so no lossy domain↔record conversion sits in the
//! path.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::domain::normalize::fold_title;
use crate::error::AppError;
use crate::infrastructure::repositories::media::{self, AltTitle, MediaRecord};

/// A field where survivor and duplicate carry different non-empty values.
#[derive(Debug, Clone, Serialize)]
pub struct MergeConflict {
    pub field: String,
    pub survivor: String,
    pub duplicate: String,
}

/// What apply would change — the dialog's preview.
#[derive(Debug, Clone, Serialize)]
pub struct MergePreview {
    pub survivor_id: String,
    pub duplicate_id: String,
    pub survivor_title: String,
    pub duplicate_title: String,
    pub merged_title: String,
    pub conflicts: Vec<MergeConflict>,
    pub nodes_to_move: u32,
    pub move_review: bool,
    pub move_tracking: bool,
    pub collections_to_move: u32,
}

/// What a finished merge reports.
#[derive(Debug, Clone, Serialize)]
pub struct MergeResult {
    /// Trash entry holding the undo image.
    pub trash_id: String,
}

/// Everything needed to undo a merge, stored as the trash payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeTrashPayload {
    pub survivor_id: String,
    pub media: MediaRecord,
    pub node_ids: Vec<String>,
    pub moved_review: bool,
    pub moved_tracking: bool,
    pub collection_ids: Vec<String>,
}

/// Merge use-cases.
pub struct MergeService {
    pool: SqlitePool,
}

impl MergeService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Preview what merging `duplicate_id` into `survivor_id` would change.
    pub async fn plan(
        &self,
        survivor_id: &str,
        duplicate_id: &str,
    ) -> Result<MergePreview, AppError> {
        if survivor_id == duplicate_id {
            return Err(AppError::validation("cannot merge a title into itself"));
        }
        let survivor = get_record(&self.pool, survivor_id).await?;
        let duplicate = get_record(&self.pool, duplicate_id).await?;

        let node_ids = node_ids_of(&self.pool, duplicate_id).await?;
        let move_review = !exists(&self.pool, "review", survivor_id).await?
            && exists(&self.pool, "review", duplicate_id).await?;
        let move_tracking = !exists(&self.pool, "tracking", survivor_id).await?
            && exists(&self.pool, "tracking", duplicate_id).await?;
        let collection_ids = collection_ids_of(&self.pool, duplicate_id).await?;

        let (merged, conflicts) = merge_records(&survivor, &duplicate);

        Ok(MergePreview {
            survivor_id: survivor.id.clone(),
            duplicate_id: duplicate.id.clone(),
            survivor_title: survivor.title_main.clone(),
            duplicate_title: duplicate.title_main.clone(),
            merged_title: merged.title_main,
            conflicts,
            nodes_to_move: node_ids.len() as u32,
            move_review,
            move_tracking,
            collections_to_move: collection_ids.len() as u32,
        })
    }

    /// Apply a merge: snapshot the duplicate into trash, fold its data into
    /// the survivor and delete it. Resolves with the trash id for undo.
    pub async fn apply(
        &self,
        survivor_id: &str,
        duplicate_id: &str,
    ) -> Result<MergeResult, AppError> {
        if survivor_id == duplicate_id {
            return Err(AppError::validation("cannot merge a title into itself"));
        }
        let survivor = get_record(&self.pool, survivor_id).await?;
        let duplicate = get_record(&self.pool, duplicate_id).await?;

        let node_ids = node_ids_of(&self.pool, duplicate_id).await?;
        let moved_review = !exists(&self.pool, "review", survivor_id).await?
            && exists(&self.pool, "review", duplicate_id).await?;
        let moved_tracking = !exists(&self.pool, "tracking", survivor_id).await?
            && exists(&self.pool, "tracking", duplicate_id).await?;
        let collection_ids = collection_ids_of(&self.pool, duplicate_id).await?;

        // 1. Undo image first: from here on every failure is recoverable by
        //    restoring this trash entry.
        let payload = MergeTrashPayload {
            survivor_id: survivor.id.clone(),
            media: duplicate.clone(),
            node_ids: node_ids.clone(),
            moved_review,
            moved_tracking,
            collection_ids: collection_ids.clone(),
        };
        let trash_id = format!("t-{}", Uuid::new_v4());
        crate::infrastructure::repositories::trash::insert(
            &self.pool,
            &trash_id,
            "merge",
            &serde_json::to_string(&payload)?,
            &Utc::now().to_rfc3339(),
        )
        .await?;

        // 2. Fold the duplicate into the survivor.
        let (merged, _) = merge_records(&survivor, &duplicate);
        media::update(&self.pool, &merged).await?;

        // 3. Re-key the duplicate's rows onto the survivor.
        sqlx::query("UPDATE content_node SET media_id = ? WHERE media_id = ?")
            .bind(survivor_id)
            .bind(duplicate_id)
            .execute(&self.pool)
            .await?;
        if moved_review {
            sqlx::query("UPDATE review SET media_id = ? WHERE media_id = ?")
                .bind(survivor_id)
                .bind(duplicate_id)
                .execute(&self.pool)
                .await?;
        }
        if moved_tracking {
            sqlx::query("UPDATE tracking SET media_id = ? WHERE media_id = ?")
                .bind(survivor_id)
                .bind(duplicate_id)
                .execute(&self.pool)
                .await?;
        }
        for collection_id in &collection_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO collection_member (collection_id, media_id, added_at) \
                 VALUES (?, ?, ?)",
            )
            .bind(collection_id)
            .bind(survivor_id)
            .bind(&survivor.created_at)
            .execute(&self.pool)
            .await?;
        }

        // 4. Delete the duplicate; whatever it still owns cascades away.
        media::delete(&self.pool, duplicate_id).await?;

        Ok(MergeResult { trash_id })
    }
}

async fn get_record(pool: &SqlitePool, id: &str) -> Result<MediaRecord, AppError> {
    media::get(pool, id)
        .await?
        .ok_or_else(|| AppError::validation(format!("media not found: {id}")))
}

async fn node_ids_of(pool: &SqlitePool, media_id: &str) -> Result<Vec<String>, AppError> {
    let ids: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM content_node WHERE media_id = ? ORDER BY id")
            .bind(media_id)
            .fetch_all(pool)
            .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

async fn exists(pool: &SqlitePool, table: &str, media_id: &str) -> Result<bool, AppError> {
    // Table names come from the two call sites above, never from user input.
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE media_id = ?");
    let (count,): (i64,) = sqlx::query_as(&sql).bind(media_id).fetch_one(pool).await?;
    Ok(count > 0)
}

async fn collection_ids_of(pool: &SqlitePool, media_id: &str) -> Result<Vec<String>, AppError> {
    let ids: Vec<(String,)> = sqlx::query_as(
        "SELECT collection_id FROM collection_member WHERE media_id = ? ORDER BY collection_id",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

/// The MISSION-028 policy at record level: survivor identity kept, scalars
/// prefer the survivor and fall back to the duplicate, sets are unioned, the
/// duplicate's main title becomes an alternative when it differs, external-id
/// providers stay unique with the survivor winning. Conflicts are reported
/// for every different non-empty pair.
fn merge_records(
    survivor: &MediaRecord,
    duplicate: &MediaRecord,
) -> (MediaRecord, Vec<MergeConflict>) {
    let mut conflicts: Vec<MergeConflict> = Vec::new();
    let mut merged = survivor.clone();
    merged.updated_at = Utc::now().to_rfc3339();

    if survivor.content_type != duplicate.content_type {
        conflicts.push(conflict(
            "content_type",
            &survivor.content_type,
            &duplicate.content_type,
        ));
    }
    if survivor.pub_status != duplicate.pub_status {
        conflicts.push(conflict(
            "pub_status",
            &survivor.pub_status,
            &duplicate.pub_status,
        ));
    }

    // Optional scalars: conflict when both differ, fallback when absent.
    merge_opt(
        &mut conflicts,
        "format",
        &survivor.format,
        &duplicate.format,
        &mut merged.format,
    );
    merge_opt(
        &mut conflicts,
        "synopsis",
        &survivor.synopsis,
        &duplicate.synopsis,
        &mut merged.synopsis,
    );
    merge_opt(
        &mut conflicts,
        "title_original",
        &survivor.title_original,
        &duplicate.title_original,
        &mut merged.title_original,
    );
    merge_opt(
        &mut conflicts,
        "start_date",
        &survivor.start_date,
        &duplicate.start_date,
        &mut merged.start_date,
    );
    merge_opt(
        &mut conflicts,
        "end_date",
        &survivor.end_date,
        &duplicate.end_date,
        &mut merged.end_date,
    );
    merge_opt(
        &mut conflicts,
        "release_year",
        &survivor.release_year,
        &duplicate.release_year,
        &mut merged.release_year,
    );
    merge_opt(
        &mut conflicts,
        "language",
        &survivor.language,
        &duplicate.language,
        &mut merged.language,
    );
    merge_opt(
        &mut conflicts,
        "country",
        &survivor.country,
        &duplicate.country,
        &mut merged.country,
    );
    merge_opt(
        &mut conflicts,
        "content_rating",
        &survivor.content_rating,
        &duplicate.content_rating,
        &mut merged.content_rating,
    );
    merge_opt(
        &mut conflicts,
        "pages",
        &survivor.pages,
        &duplicate.pages,
        &mut merged.pages,
    );
    merge_opt(
        &mut conflicts,
        "duration_min",
        &survivor.duration_min,
        &duplicate.duration_min,
        &mut merged.duration_min,
    );
    merge_opt(
        &mut conflicts,
        "ep_count",
        &survivor.ep_count,
        &duplicate.ep_count,
        &mut merged.ep_count,
    );
    merge_opt(
        &mut conflicts,
        "ch_count",
        &survivor.ch_count,
        &duplicate.ch_count,
        &mut merged.ch_count,
    );

    // Titles: the survivor's main wins; a different duplicate main title
    // survives as an alternative.
    if fold_title(&survivor.title_main) != fold_title(&duplicate.title_main) {
        conflicts.push(conflict(
            "title",
            &survivor.title_main,
            &duplicate.title_main,
        ));
        let already_listed = merged
            .alt_titles
            .iter()
            .any(|alt| fold_title(&alt.title) == fold_title(&duplicate.title_main));
        if !already_listed {
            merged.alt_titles.push(AltTitle {
                lang: duplicate.language.clone().unwrap_or_else(|| "und".into()),
                title: duplicate.title_main.clone(),
            });
        }
    }
    for alt in &duplicate.alt_titles {
        let exists = merged
            .alt_titles
            .iter()
            .any(|a| fold_title(&a.title) == fold_title(&alt.title))
            || fold_title(&merged.title_main) == fold_title(&alt.title);
        if !exists {
            merged.alt_titles.push(alt.clone());
        }
    }

    merged.genres = union(&survivor.genres, &duplicate.genres);
    merged.tags = union(&survivor.tags, &duplicate.tags);
    merged.people = union(&survivor.people, &duplicate.people);

    // Relations: unioned by target+kind; anything pointing at either merged
    // id is dropped (it would be a self-relation on the survivor).
    for relation in &duplicate.relations {
        if relation.to_id == survivor.id || relation.to_id == duplicate.id {
            continue;
        }
        if !survivor
            .relations
            .iter()
            .any(|r| r.to_id == relation.to_id && r.relation == relation.relation)
        {
            merged.relations.push(relation.clone());
        }
    }
    merged
        .relations
        .retain(|r| r.to_id != survivor.id && r.to_id != duplicate.id);

    // External ids stay provider-unique; the survivor's value wins.
    for ext in &duplicate.external_ids {
        match merged
            .external_ids
            .iter_mut()
            .find(|e| e.provider == ext.provider)
        {
            Some(existing) => {
                if existing.ext_id != ext.ext_id {
                    conflicts.push(MergeConflict {
                        field: format!("external_id.{}", ext.provider),
                        survivor: existing.ext_id.clone(),
                        duplicate: ext.ext_id.clone(),
                    });
                }
            }
            None => merged.external_ids.push(ext.clone()),
        }
    }

    (merged, conflicts)
}

fn conflict(field: &str, survivor: &str, duplicate: &str) -> MergeConflict {
    MergeConflict {
        field: field.into(),
        survivor: survivor.into(),
        duplicate: duplicate.into(),
    }
}

fn merge_opt<T: PartialEq + Clone + ToString>(
    conflicts: &mut Vec<MergeConflict>,
    field: &str,
    survivor: &Option<T>,
    duplicate: &Option<T>,
    target: &mut Option<T>,
) {
    if let (Some(s), Some(d)) = (survivor, duplicate) {
        if s != d {
            conflicts.push(MergeConflict {
                field: field.into(),
                survivor: s.to_string(),
                duplicate: d.to_string(),
            });
        }
    }
    if target.is_none() {
        *target = duplicate.clone();
    }
}

fn union(first: &[String], second: &[String]) -> Vec<String> {
    let mut out = first.to_vec();
    for value in second {
        if !out.iter().any(|v| v == value) {
            out.push(value.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::media_service::{AddMediaInput, MediaService};
    use crate::application::trash_service::TrashService;
    use crate::infrastructure::repositories::media::ExternalId;
    use crate::infrastructure::repositories::trash;
    use crate::infrastructure::test_support::migrated_pool;

    fn record(id: &str, title: &str) -> MediaRecord {
        MediaRecord {
            id: id.into(),
            content_type: "novel".into(),
            format: None,
            title_main: title.into(),
            title_original: None,
            synopsis: None,
            pub_status: "ongoing".into(),
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
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
            alt_titles: Vec::new(),
            people: Vec::new(),
            genres: Vec::new(),
            tags: Vec::new(),
            external_ids: Vec::new(),
            relations: Vec::new(),
        }
    }

    #[test]
    fn merge_records_follows_the_survivor_preferred_policy() {
        let mut survivor = record("m-s", "Fairy Tail");
        survivor.synopsis = Some("survivor synopsis".into());
        survivor.release_year = Some(2009);
        survivor.genres = vec!["fantasy".into()];
        survivor.external_ids = vec![ExternalId {
            provider: "anilist".into(),
            ext_id: "1".into(),
            url: None,
        }];

        let mut duplicate = record("m-d", "Fairy Tail (Manga)");
        duplicate.synopsis = Some("duplicate synopsis".into());
        duplicate.release_year = Some(2014);
        duplicate.genres = vec!["action".into()];
        duplicate.external_ids = vec![ExternalId {
            provider: "anilist".into(),
            ext_id: "2".into(),
            url: None,
        }];

        let (merged, conflicts) = merge_records(&survivor, &duplicate);

        // Survivor identity + values win; the duplicate main title becomes an
        // alternative; the survivor's year stays (fallback only when absent);
        // the genre set unions.
        assert_eq!(merged.id, "m-s");
        assert_eq!(merged.title_main, "Fairy Tail");
        assert_eq!(merged.release_year, Some(2009));
        assert_eq!(merged.genres, vec!["fantasy", "action"]);
        assert!(merged
            .alt_titles
            .iter()
            .any(|alt| alt.title == "Fairy Tail (Manga)"));

        let fields: Vec<&str> = conflicts.iter().map(|c| c.field.as_str()).collect();
        assert!(fields.contains(&"synopsis"));
        assert!(fields.contains(&"release_year"));
        assert!(fields.contains(&"title"));
        assert!(conflicts
            .iter()
            .any(|c| c.field == "external_id.anilist" && c.survivor == "1" && c.duplicate == "2"));
    }

    // Local stand-in mirroring the repo's ExternalId shape for brevity in the
    // policy test above.
    struct ExternalIdLike {
        provider: String,
        ext_id: String,
        url: Option<String>,
    }

    impl From<ExternalIdLike> for crate::infrastructure::repositories::media::ExternalId {
        fn from(value: ExternalIdLike) -> Self {
            Self {
                provider: value.provider,
                ext_id: value.ext_id,
                url: value.url,
            }
        }
    }

    async fn seed_pair(pool: &sqlx::SqlitePool) -> (String, String) {
        let media_service = MediaService::new(pool.clone());
        let survivor = media_service
            .add_media(AddMediaInput {
                title: "Fairy Tail".into(),
                content_type: "novel".into(),
                format: Some("light_novel".into()),
                pub_status: Some("ongoing".into()),
                synopsis: None,
                release_year: Some(2009),
                language: None,
                country: None,
                pages: None,
                duration_min: None,
                ep_count: None,
                ch_count: None,
                genres: vec!["fantasy".into()],
            })
            .await
            .expect("add survivor")
            .to_string();
        let duplicate = media_service
            .add_media(AddMediaInput {
                title: "Fairy Tail (Duplicate)".into(),
                content_type: "novel".into(),
                format: None,
                pub_status: Some("ongoing".into()),
                synopsis: Some("duplicate synopsis".into()),
                release_year: None,
                language: None,
                country: None,
                pages: Some(42),
                duration_min: None,
                ep_count: None,
                ch_count: None,
                genres: vec!["action".into()],
            })
            .await
            .expect("add duplicate")
            .to_string();

        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, created_at) \
             VALUES ('n-1', ?, 'chapter', 1, '2026-01-01')",
        )
        .bind(&duplicate)
        .execute(pool)
        .await
        .expect("seed node");
        sqlx::query("INSERT INTO review (media_id, rating, created_at, updated_at) VALUES (?, 8, '2026-01-01', '2026-01-01')")
            .bind(&duplicate)
            .execute(pool)
            .await
            .expect("seed review");
        sqlx::query(
            "INSERT INTO collection (id, name, created_at) VALUES ('c-1', 'Reading', '2026-01-01')",
        )
        .execute(pool)
        .await
        .expect("seed collection");
        sqlx::query("INSERT INTO collection_member (collection_id, media_id, added_at) VALUES ('c-1', ?, '2026-01-01')")
            .bind(&duplicate)
            .execute(pool)
            .await
            .expect("seed membership");

        (survivor, duplicate)
    }

    #[tokio::test]
    async fn plan_reports_conflicts_and_movements() {
        let (pool, _path) = migrated_pool("merge_plan.db").await;
        let service = MergeService::new(pool.clone());
        let (survivor, duplicate) = seed_pair(&pool).await;

        let preview = service.plan(&survivor, &duplicate).await.expect("plan");
        assert_eq!(preview.merged_title, "Fairy Tail");
        assert_eq!(preview.nodes_to_move, 1);
        assert!(preview.move_review, "the survivor has no review yet");
        assert!(!preview.move_tracking);
        assert_eq!(preview.collections_to_move, 1);
        // The differing main titles surface as a conflict; the duplicate's
        // synopsis fills the survivor's gap without conflicting.
        assert!(preview.conflicts.iter().any(|c| c.field == "title"));
        assert!(!preview.conflicts.iter().any(|c| c.field == "synopsis"));

        pool.close().await;
    }

    #[tokio::test]
    async fn apply_folds_the_duplicate_and_trash_can_undo_it() {
        let (pool, _path) = migrated_pool("merge_apply.db").await;
        let service = MergeService::new(pool.clone());
        let trash_service = TrashService::new(pool.clone());
        let (survivor, duplicate) = seed_pair(&pool).await;

        let result = service.apply(&survivor, &duplicate).await.expect("apply");

        // The survivor absorbed everything; the duplicate row is gone.
        let merged = crate::infrastructure::repositories::media::get(&pool, &survivor)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(merged.pages, Some(42), "fallback from the duplicate");
        assert_eq!(merged.genres.len(), 2, "genres unioned");
        let (nodes,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM content_node WHERE media_id = ?")
                .bind(&survivor)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(nodes, 1, "the duplicate's node was re-parented");
        let (reviews,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM review WHERE media_id = ?")
            .bind(&survivor)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(reviews, 1, "the duplicate's review moved");
        assert!(
            crate::infrastructure::repositories::media::get(&pool, &duplicate)
                .await
                .expect("get")
                .is_none()
        );

        // Undo through the trash layer restores the pre-merge world.
        trash_service
            .restore_media(&result.trash_id)
            .await
            .expect("undo");
        let restored = crate::infrastructure::repositories::media::get(&pool, &duplicate)
            .await
            .expect("get")
            .expect("duplicate re-created");
        assert_eq!(restored.title_main, "Fairy Tail (Duplicate)");
        let (back,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM content_node WHERE media_id = ?")
                .bind(&duplicate)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(back, 1, "the node went home");
        let (review_back,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM review WHERE media_id = ?")
                .bind(&duplicate)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(review_back, 1, "the review went home");
        let (memberships,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM collection_member WHERE media_id = ?")
                .bind(&duplicate)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(memberships, 1, "collection membership re-added");

        pool.close().await;
    }

    #[tokio::test]
    async fn apply_rejects_self_merge_and_missing_media() {
        let (pool, _path) = migrated_pool("merge_validate.db").await;
        let service = MergeService::new(pool.clone());
        let (survivor, _duplicate) = seed_pair(&pool).await;

        assert!(service.apply(&survivor, &survivor).await.is_err());
        assert!(service.plan(&survivor, "m-missing").await.is_err());
        let entries = trash::list(&pool).await.expect("trash untouched");
        assert!(entries.is_empty());

        pool.close().await;
    }
}
