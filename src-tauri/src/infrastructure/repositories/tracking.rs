//! Tracking repository (MISSION-019).
//!
//! User state: the per-media `tracking` row and per-node `node_progress`
//! rows. Aggregate progress is derived, never stored. Status transitions and
//! progress math live in the domain layer (MISSION-023/024); this module only
//! persists records.

use std::collections::HashMap;

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;
use crate::infrastructure::repositories::media::{row_to_summary, MediaSummary};

/// The per-media user tracking state.
#[derive(Debug, Clone)]
pub struct TrackingRecord {
    pub media_id: String,
    pub core_status: String,
    pub custom_status_id: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub repeat_count: i64,
    pub current_node_id: Option<String>,
    pub current_position: Option<i64>,
    /// 1 = Normal (autoTrack) mode, 0 = Manual (MISSION-052).
    pub auto_track: i64,
    pub updated_at: String,
}

/// Per-node progress (part of the tracking aggregate).
#[derive(Debug, Clone)]
pub struct NodeProgress {
    pub node_id: String,
    pub state: String,
    pub read_at: Option<String>,
    pub note: Option<String>,
    pub rating: Option<i64>,
    pub updated_at: String,
}

/// Insert or update the tracking row for a media.
pub async fn upsert_tracking(pool: &SqlitePool, t: &TrackingRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO tracking
           (media_id, core_status, custom_status_id, started_at, finished_at,
            repeat_count, current_node_id, current_position, auto_track, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           core_status = excluded.core_status,
           custom_status_id = excluded.custom_status_id,
           started_at = excluded.started_at,
           finished_at = excluded.finished_at,
           repeat_count = excluded.repeat_count,
           current_node_id = excluded.current_node_id,
           current_position = excluded.current_position,
           auto_track = excluded.auto_track,
           updated_at = excluded.updated_at",
    )
    .bind(&t.media_id)
    .bind(&t.core_status)
    .bind(&t.custom_status_id)
    .bind(&t.started_at)
    .bind(&t.finished_at)
    .bind(t.repeat_count)
    .bind(&t.current_node_id)
    .bind(t.current_position)
    .bind(t.auto_track)
    .bind(&t.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch the tracking row for a media (or `None`).
pub async fn get_tracking(
    pool: &SqlitePool,
    media_id: &str,
) -> Result<Option<TrackingRecord>, AppError> {
    let row = sqlx::query(
        "SELECT media_id, core_status, custom_status_id, started_at, finished_at, \
         repeat_count, current_node_id, current_position, auto_track, updated_at \
         FROM tracking WHERE media_id = ?",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_tracking))
}

/// Remove the tracking row for a media (leaves node_progress untouched).
pub async fn delete_tracking(pool: &SqlitePool, media_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM tracking WHERE media_id = ?")
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Insert or update progress for one node.
pub async fn set_progress(pool: &SqlitePool, p: &NodeProgress) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO node_progress (node_id, state, read_at, note, rating, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(node_id) DO UPDATE SET
           state = excluded.state,
           read_at = excluded.read_at,
           note = excluded.note,
           rating = excluded.rating,
           updated_at = excluded.updated_at",
    )
    .bind(&p.node_id)
    .bind(&p.state)
    .bind(&p.read_at)
    .bind(&p.note)
    .bind(p.rating)
    .bind(&p.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch progress for one node (or `None`).
pub async fn get_progress(
    pool: &SqlitePool,
    node_id: &str,
) -> Result<Option<NodeProgress>, AppError> {
    let row = sqlx::query(
        "SELECT node_id, state, read_at, note, rating, updated_at \
         FROM node_progress WHERE node_id = ?",
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_progress))
}

/// Upsert progress rows for many nodes in one transaction (MISSION-047 range
/// marks). Resolves with the ids actually written.
pub async fn set_progress_many(
    pool: &SqlitePool,
    rows: &[NodeProgress],
) -> Result<Vec<String>, AppError> {
    let mut tx = pool.begin().await?;
    let mut written = Vec::with_capacity(rows.len());
    for progress in rows {
        sqlx::query(
            "INSERT INTO node_progress (node_id, state, read_at, note, rating, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(node_id) DO UPDATE SET
               state = excluded.state,
               read_at = excluded.read_at,
               note = excluded.note,
               rating = excluded.rating,
               updated_at = excluded.updated_at",
        )
        .bind(&progress.node_id)
        .bind(&progress.state)
        .bind(&progress.read_at)
        .bind(&progress.note)
        .bind(progress.rating)
        .bind(&progress.updated_at)
        .execute(&mut *tx)
        .await?;
        written.push(progress.node_id.clone());
    }
    tx.commit().await?;
    Ok(written)
}

/// All progress rows for a media's nodes.
pub async fn progress_for_media(
    pool: &SqlitePool,
    media_id: &str,
) -> Result<Vec<NodeProgress>, AppError> {
    let rows = sqlx::query(
        "SELECT np.node_id, np.state, np.read_at, np.note, np.rating, np.updated_at
         FROM node_progress np
         JOIN content_node cn ON cn.id = np.node_id
         WHERE cn.media_id = ?
         ORDER BY cn.position, cn.id",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_progress).collect())
}

/// Number of a media's nodes in the given state (e.g. "read").
pub async fn count_nodes_in_state(
    pool: &SqlitePool,
    media_id: &str,
    state: &str,
) -> Result<i64, AppError> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM node_progress np
         JOIN content_node cn ON cn.id = np.node_id
         WHERE cn.media_id = ? AND np.state = ?",
    )
    .bind(media_id)
    .bind(state)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Every node of a media with its progress state (unmarked nodes report
/// `unread`). This is the full tick set the domain progress engine folds —
/// unmarked nodes must be present for the auto-status suggestion to reason
/// about completion (MISSION-048).
#[derive(Debug, Clone)]
pub struct NodeTickRow {
    pub node_id: String,
    pub kind: String,
    pub page_count: Option<i64>,
    pub duration_min: Option<i64>,
    pub state: String,
}

/// All node ticks for a media, in display order (position, then id).
pub async fn node_ticks_for_media(
    pool: &SqlitePool,
    media_id: &str,
) -> Result<Vec<NodeTickRow>, AppError> {
    let rows = sqlx::query(
        "SELECT cn.id, cn.kind, cn.page_count, cn.duration_min, \
         COALESCE(np.state, 'unread') \
         FROM content_node cn \
         LEFT JOIN node_progress np ON np.node_id = cn.id \
         WHERE cn.media_id = ? \
         ORDER BY cn.position, cn.id",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| NodeTickRow {
            node_id: row.get(0),
            kind: row.get(1),
            page_count: row.get(2),
            duration_min: row.get(3),
            state: row.get(4),
        })
        .collect())
}

/// The countable node kinds across every progress template (episodes,
/// chapters, generic nodes) — the universe `progress_summaries`/`next_unread_unit`
/// reason about.
const UNIT_KINDS: &str = "'episode','chapter','node'";

/// A media's progress summary for library lists (MISSION-049). Weights are
/// unit counts — pages for books, else one per node — matching the domain
/// progress engine, plus the next not-yet-consumed countable node in display
/// order (drives the in-grid "mark next" control).
#[derive(Debug, Clone)]
pub struct MediaProgressSummary {
    pub media_id: String,
    pub content_type: String,
    pub total_weight: i64,
    pub completed_weight: i64,
    pub next_node_id: Option<String>,
    pub next_kind: Option<String>,
    pub next_number: Option<String>,
    pub next_position: Option<i64>,
}

/// The next countable node of a media that is not yet consumed.
#[derive(Debug, Clone)]
pub struct NextUnitRow {
    pub node_id: String,
    pub kind: String,
    pub number: Option<String>,
    pub position: i64,
}

/// Weighted progress summaries for a set of media ids, computed in two batched
/// queries — one aggregate (weights per media) and one windowed "first unread
/// node" — so a whole library list resolves in a constant number of queries
/// regardless of how many media match. Media without countable nodes (or
/// unknown ids) simply produce no row.
pub async fn progress_summaries(
    pool: &SqlitePool,
    media_ids: &[String],
) -> Result<Vec<MediaProgressSummary>, AppError> {
    if media_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut summaries: HashMap<String, MediaProgressSummary> = HashMap::new();
    for chunk in media_ids.chunks(500) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

        let aggregate_sql = format!(
            "SELECT cn.media_id, m.content_type, \
                COALESCE(SUM(CASE WHEN m.content_type = 'book' \
                   THEN COALESCE(cn.page_count, 1) ELSE 1 END), 0) AS total_weight, \
                COALESCE(SUM(CASE WHEN COALESCE(np.state, 'unread') IN ('read', 'watched') \
                   THEN CASE WHEN m.content_type = 'book' \
                     THEN COALESCE(cn.page_count, 1) ELSE 1 END ELSE 0 END), 0) \
                   AS completed_weight \
             FROM content_node cn \
             JOIN media m ON m.id = cn.media_id \
             LEFT JOIN node_progress np ON np.node_id = cn.id \
             WHERE cn.media_id IN ({placeholders}) \
               AND cn.kind IN ({UNIT_KINDS}) \
             GROUP BY cn.media_id, m.content_type"
        );
        let mut query = sqlx::query(&aggregate_sql);
        for id in chunk {
            query = query.bind(id.as_str());
        }
        let rows = query.fetch_all(pool).await?;
        for row in rows {
            summaries.insert(
                row.get(0),
                MediaProgressSummary {
                    media_id: row.get(0),
                    content_type: row.get(1),
                    total_weight: row.get(2),
                    completed_weight: row.get(3),
                    next_node_id: None,
                    next_kind: None,
                    next_number: None,
                    next_position: None,
                },
            );
        }

        let next_sql = format!(
            "WITH ranked AS ( \
               SELECT cn.id, cn.kind, cn.number, cn.position, cn.media_id, \
                      ROW_NUMBER() OVER (PARTITION BY cn.media_id \
                        ORDER BY cn.position, cn.id) AS rn \
               FROM content_node cn \
               JOIN media m ON m.id = cn.media_id \
               LEFT JOIN node_progress np ON np.node_id = cn.id \
               WHERE cn.media_id IN ({placeholders}) \
                 AND cn.kind IN ({UNIT_KINDS}) \
                 AND COALESCE(np.state, 'unread') NOT IN ('read', 'watched') \
             ) \
             SELECT media_id, id, kind, number, position FROM ranked WHERE rn = 1"
        );
        let mut query = sqlx::query(&next_sql);
        for id in chunk {
            query = query.bind(id.as_str());
        }
        let rows = query.fetch_all(pool).await?;
        for row in rows {
            if let Some(entry) = summaries.get_mut(&row.get::<String, _>(0)) {
                entry.next_node_id = row.get(1);
                entry.next_kind = row.get(2);
                entry.next_number = row.get(3);
                entry.next_position = row.get(4);
            }
        }
    }
    Ok(summaries.into_values().collect())
}

/// The first countable node of a media that is not yet consumed, in display
/// order (position, then id), or `None` when everything is consumed or the
/// media has no countable nodes (MISSION-049 mark-next).
pub async fn next_unread_unit(
    pool: &SqlitePool,
    media_id: &str,
) -> Result<Option<NextUnitRow>, AppError> {
    let row = sqlx::query(
        "SELECT cn.id, cn.kind, cn.number, cn.position \
         FROM content_node cn \
         LEFT JOIN node_progress np ON np.node_id = cn.id \
         WHERE cn.media_id = ? \
           AND cn.kind IN ('episode', 'chapter', 'node') \
           AND COALESCE(np.state, 'unread') NOT IN ('read', 'watched') \
         ORDER BY cn.position, cn.id \
         LIMIT 1",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| NextUnitRow {
        node_id: row.get(0),
        kind: row.get(1),
        number: row.get(2),
        position: row.get(3),
    }))
}

/// Ordering for `recent_media_by_status` (MISSION-050 dashboard widgets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentOrder {
    /// Most recently updated tracking row first — drives "Continue".
    UpdatedAt,
    /// Most recently finished first, unfinished rows last — drives "Completed".
    FinishedAt,
}

/// Media joined with a tracking row in one of `statuses` (MISSION-050).
/// Ordered per `order` and capped by `limit`; rows carry the `MediaSummary`
/// shape so callers map them onto list items with the batched progress attach.
pub async fn recent_media_by_status(
    pool: &SqlitePool,
    statuses: &[&str],
    order: RecentOrder,
    limit: i64,
) -> Result<Vec<MediaSummary>, AppError> {
    if statuses.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = String::from(
        "SELECT m.id, m.content_type, m.title_main, m.pub_status, m.release_year, \
         m.cover_asset_id, COALESCE(r.favorite, 0) AS favorite, m.updated_at \
         FROM media m \
         JOIN tracking t ON t.media_id = m.id \
         LEFT JOIN review r ON r.media_id = m.id \
         WHERE t.core_status IN (",
    );
    query.push_str(&vec!["?"; statuses.len()].join(", "));
    query.push(')');
    match order {
        RecentOrder::UpdatedAt => {
            query.push_str(" ORDER BY t.updated_at DESC, m.title_main ASC LIMIT ?")
        }
        RecentOrder::FinishedAt => query.push_str(
            " ORDER BY t.finished_at IS NULL ASC, t.finished_at DESC, m.title_main ASC LIMIT ?",
        ),
    }
    let mut qb = sqlx::query(&query);
    for status in statuses {
        qb = qb.bind(status);
    }
    qb = qb.bind(limit);
    let rows = qb.fetch_all(pool).await?;
    Ok(rows.into_iter().map(row_to_summary).collect())
}

fn row_to_tracking(row: SqliteRow) -> TrackingRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    TrackingRecord {
        media_id: get(0).expect("media_id"),
        core_status: get(1).expect("core_status"),
        custom_status_id: get(2),
        started_at: get(3),
        finished_at: get(4),
        repeat_count: row.get(5),
        current_node_id: get(6),
        current_position: row.get(7),
        auto_track: row.get(8),
        updated_at: get(9).expect("updated_at"),
    }
}

fn row_to_progress(row: SqliteRow) -> NodeProgress {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    NodeProgress {
        node_id: get(0).expect("node_id"),
        state: get(1).expect("state"),
        read_at: get(2),
        note: get(3),
        rating: row.get(4),
        updated_at: get(5).expect("updated_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::node;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn tracking(media_id: &str, status: &str) -> TrackingRecord {
        TrackingRecord {
            media_id: media_id.to_string(),
            core_status: status.to_string(),
            custom_status_id: None,
            started_at: Some("2026-01-01".into()),
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: Some(12),
            auto_track: 1,
            updated_at: "2026-01-01".to_string(),
        }
    }

    async fn seed_node(pool: &SqlitePool, id: &str, media_id: &str) {
        node::create(
            pool,
            &node::NodeRecord {
                id: id.to_string(),
                media_id: media_id.to_string(),
                parent_id: None,
                kind: "chapter".into(),
                position: 1,
                number: None,
                title: None,
                release_date: None,
                duration_min: None,
                page_count: None,
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed node");
    }

    #[tokio::test]
    async fn upsert_tracking_roundtrips_and_updates() {
        let (pool, path) = migrated_pool("tracking_repo.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");

        upsert_tracking(&pool, &tracking("m-1", "in_progress"))
            .await
            .expect("upsert");
        let got = get_tracking(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(got.core_status, "in_progress");
        assert_eq!(got.current_position, Some(12));
        assert_eq!(got.auto_track, 1, "auto_track defaults to Normal");

        let mut t = tracking("m-1", "completed");
        t.repeat_count = 1;
        t.auto_track = 0;
        t.updated_at = "2026-02-01".into();
        upsert_tracking(&pool, &t).await.expect("re-upsert");

        let got = get_tracking(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(got.core_status, "completed");
        assert_eq!(got.repeat_count, 1);
        assert_eq!(got.auto_track, 0, "upsert roundtrips the mode");
        assert_eq!(got.updated_at, "2026-02-01");

        assert!(get_tracking(&pool, "nope").await.expect("get").is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn node_progress_upsert_and_aggregate() {
        let (pool, path) = migrated_pool("tracking_progress.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node(&pool, "n-1", "m-1").await;
        seed_node(&pool, "n-2", "m-1").await;

        set_progress(
            &pool,
            &NodeProgress {
                node_id: "n-1".into(),
                state: "read".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: Some(8),
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("set progress");
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "n-1".into(),
                state: "read".into(),
                read_at: Some("2026-01-02".into()),
                note: Some("loved it".into()),
                rating: Some(9),
                updated_at: "2026-01-03".into(),
            },
        )
        .await
        .expect("update progress");

        let got = get_progress(&pool, "n-1").await.expect("get").unwrap();
        assert_eq!(got.rating, Some(9));
        assert_eq!(got.note.as_deref(), Some("loved it"));

        let all = progress_for_media(&pool, "m-1").await.expect("all");
        assert_eq!(all.len(), 1);

        let read = count_nodes_in_state(&pool, "m-1", "read")
            .await
            .expect("count");
        assert_eq!(read, 1);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn node_ticks_include_unmarked_nodes_as_unread() {
        let (pool, path) = migrated_pool("tracking_ticks.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'manga', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node(&pool, "n-1", "m-1").await;
        seed_node(&pool, "n-2", "m-1").await;
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "n-1".into(),
                state: "read".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("mark read");

        let ticks = node_ticks_for_media(&pool, "m-1").await.expect("ticks");
        assert_eq!(ticks.len(), 2, "unmarked nodes still yield a tick");
        assert_eq!(ticks[0].node_id, "n-1");
        assert_eq!(ticks[0].state, "read");
        assert_eq!(ticks[1].node_id, "n-2");
        assert_eq!(ticks[1].state, "unread");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn set_progress_many_writes_all_in_one_transaction() {
        let (pool, path) = migrated_pool("tracking_progress_many.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node(&pool, "n-1", "m-1").await;
        seed_node(&pool, "n-2", "m-1").await;

        let written = set_progress_many(
            &pool,
            &[
                NodeProgress {
                    node_id: "n-1".into(),
                    state: "read".into(),
                    read_at: Some("2026-01-02".into()),
                    note: None,
                    rating: None,
                    updated_at: "2026-01-02".into(),
                },
                NodeProgress {
                    node_id: "n-2".into(),
                    state: "read".into(),
                    read_at: Some("2026-01-02".into()),
                    note: None,
                    rating: None,
                    updated_at: "2026-01-02".into(),
                },
            ],
        )
        .await
        .expect("set many");

        assert_eq!(written, vec!["n-1".to_string(), "n-2".to_string()]);
        let all = progress_for_media(&pool, "m-1").await.expect("all");
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|p| p.state == "read"));
        pool.close().await;
        cleanup_files(&path);
    }

    async fn seed_node_with_kind(
        pool: &SqlitePool,
        id: &str,
        media_id: &str,
        kind: &str,
        position: i64,
    ) {
        node::create(
            pool,
            &node::NodeRecord {
                id: id.to_string(),
                media_id: media_id.to_string(),
                parent_id: None,
                kind: kind.into(),
                position,
                number: Some(format!("n{position}")),
                title: None,
                release_date: None,
                duration_min: Some(24),
                page_count: Some(30),
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed node");
    }

    #[tokio::test]
    async fn progress_summaries_computes_weighted_percent_and_next_unit() {
        let (pool, path) = migrated_pool("tracking_summaries.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'anime', 'Anime', '2026-01-01', '2026-01-01'),
                    ('m-2', 'book', 'Book', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node_with_kind(&pool, "e1", "m-1", "episode", 1).await;
        seed_node_with_kind(&pool, "e2", "m-1", "episode", 2).await;
        seed_node_with_kind(&pool, "e3", "m-1", "episode", 3).await;
        seed_node_with_kind(&pool, "s1", "m-1", "season", 1).await;
        seed_node_with_kind(&pool, "c1", "m-2", "chapter", 1).await;
        seed_node_with_kind(&pool, "c2", "m-2", "chapter", 2).await;

        set_progress(
            &pool,
            &NodeProgress {
                node_id: "e1".into(),
                state: "watched".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("watch e1");
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "c1".into(),
                state: "read".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("read c1");

        let mut summaries = progress_summaries(&pool, &["m-1".into(), "m-2".into()])
            .await
            .expect("summaries");
        summaries.sort_by(|a, b| a.media_id.cmp(&b.media_id));

        let anime = &summaries[0];
        assert_eq!(anime.media_id, "m-1");
        assert_eq!(anime.total_weight, 3, "seasons are not countable units");
        assert_eq!(anime.completed_weight, 1);
        assert_eq!(anime.next_node_id.as_deref(), Some("e2"));
        assert_eq!(anime.next_kind.as_deref(), Some("episode"));
        assert_eq!(anime.next_number.as_deref(), Some("n2"));

        let book = &summaries[1];
        assert_eq!(book.total_weight, 60, "books weigh chapters by pages");
        assert_eq!(book.completed_weight, 30);
        assert_eq!(book.next_node_id.as_deref(), Some("c2"));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn progress_summaries_skips_unknown_and_reports_consumed_media() {
        let (pool, path) = migrated_pool("tracking_summaries_missing.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'movie', 'Movie', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node_with_kind(&pool, "e1", "m-1", "episode", 1).await;
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "e1".into(),
                state: "watched".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("watch the movie");

        let summaries = progress_summaries(&pool, &["m-1".into(), "m-nope".into()])
            .await
            .expect("summaries");
        assert_eq!(summaries.len(), 1, "unknown ids produce no row");
        let movie = &summaries[0];
        assert_eq!(movie.total_weight, 1);
        assert_eq!(movie.completed_weight, 1);
        assert!(
            movie.next_node_id.is_none(),
            "fully consumed media has no next unit"
        );

        assert!(progress_summaries(&pool, &[])
            .await
            .expect("empty")
            .is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn next_unread_unit_tracks_display_order_and_skips() {
        let (pool, path) = migrated_pool("tracking_next_unit.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'anime', 'Anime', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node_with_kind(&pool, "e1", "m-1", "episode", 1).await;
        seed_node_with_kind(&pool, "e2", "m-1", "episode", 2).await;
        seed_node_with_kind(&pool, "e3", "m-1", "episode", 3).await;

        set_progress(
            &pool,
            &NodeProgress {
                node_id: "e1".into(),
                state: "watched".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("watch e1");
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "e2".into(),
                state: "skipped".into(),
                read_at: None,
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("skip e2");

        let next = next_unread_unit(&pool, "m-1")
            .await
            .expect("next")
            .expect("has next");
        assert_eq!(next.node_id, "e2", "skipped nodes are still 'next'");
        assert_eq!(next.kind, "episode");
        assert_eq!(next.number.as_deref(), Some("n2"));

        let next = next_unread_unit(&pool, "m-nope").await.expect("next");
        assert!(next.is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn next_unread_unit_is_none_when_everything_is_consumed() {
        let (pool, path) = migrated_pool("tracking_next_unit_done.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'anime', 'Anime', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        seed_node_with_kind(&pool, "e1", "m-1", "episode", 1).await;
        seed_node_with_kind(&pool, "s1", "m-1", "season", 1).await;
        set_progress(
            &pool,
            &NodeProgress {
                node_id: "e1".into(),
                state: "watched".into(),
                read_at: Some("2026-01-02".into()),
                note: None,
                rating: None,
                updated_at: "2026-01-02".into(),
            },
        )
        .await
        .expect("watch e1");

        assert!(
            next_unread_unit(&pool, "m-1")
                .await
                .expect("next")
                .is_none(),
            "an unread season is not a countable unit"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    async fn seed_media_row(
        pool: &SqlitePool,
        id: &str,
        content_type: &str,
        title: &str,
        created_at: &str,
    ) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(content_type)
        .bind(title)
        .bind(created_at)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("seed media row");
    }

    #[tokio::test]
    async fn recent_media_by_status_filters_and_orders_by_updated_at() {
        let (pool, path) = migrated_pool("tracking_dashboard_continue.db").await;
        seed_media_row(&pool, "m-1", "anime", "A", "2026-01-01").await;
        seed_media_row(&pool, "m-2", "novel", "B", "2026-01-01").await;
        seed_media_row(&pool, "m-3", "manga", "C", "2026-01-01").await;
        let mut row = tracking("m-1", "in_progress");
        row.updated_at = "2026-01-05".into();
        upsert_tracking(&pool, &row).await.expect("track m-1");
        let mut row = tracking("m-2", "repeat");
        row.updated_at = "2026-01-06".into();
        upsert_tracking(&pool, &row).await.expect("track m-2");
        upsert_tracking(&pool, &tracking("m-3", "completed"))
            .await
            .expect("track m-3");

        let got = recent_media_by_status(
            &pool,
            &["in_progress", "repeat"],
            RecentOrder::UpdatedAt,
            10,
        )
        .await
        .expect("continue");
        let ids: Vec<&str> = got.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["m-2", "m-1"], "most recently updated first");
        assert_eq!(got[0].content_type, "novel");
        assert_eq!(got[0].title_main, "B");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn recent_media_by_status_orders_completed_nulls_last_and_limits() {
        let (pool, path) = migrated_pool("tracking_dashboard_completed.db").await;
        seed_media_row(&pool, "m-1", "anime", "A", "2026-01-01").await;
        seed_media_row(&pool, "m-2", "novel", "B", "2026-01-01").await;
        seed_media_row(&pool, "m-3", "manga", "C", "2026-01-01").await;
        seed_media_row(&pool, "m-4", "movie", "D", "2026-01-01").await;
        let mut finished_old = tracking("m-1", "completed");
        finished_old.finished_at = Some("2026-01-02".into());
        finished_old.updated_at = "2026-01-09".into();
        upsert_tracking(&pool, &finished_old).await.expect("old");
        let mut finished_new = tracking("m-2", "completed");
        finished_new.finished_at = Some("2026-01-08".into());
        finished_new.updated_at = "2026-01-10".into();
        upsert_tracking(&pool, &finished_new).await.expect("new");
        upsert_tracking(&pool, &tracking("m-3", "completed"))
            .await
            .expect("no finish");
        upsert_tracking(&pool, &tracking("m-4", "in_progress"))
            .await
            .expect("active");

        let got = recent_media_by_status(&pool, &["completed"], RecentOrder::FinishedAt, 2)
            .await
            .expect("completed");
        let ids: Vec<&str> = got.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["m-2", "m-1"],
            "finished newest first, capped at limit"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn recent_media_by_status_skips_untracked_and_empty_statuses() {
        let (pool, path) = migrated_pool("tracking_dashboard_empty.db").await;
        seed_media_row(&pool, "m-1", "anime", "A", "2026-01-01").await;
        seed_media_row(&pool, "m-2", "novel", "B", "2026-01-01").await;

        let untracked = recent_media_by_status(&pool, &["in_progress"], RecentOrder::UpdatedAt, 10)
            .await
            .expect("untracked");
        assert!(untracked.is_empty(), "no tracking rows → no widget rows");

        let none = recent_media_by_status(&pool, &[], RecentOrder::UpdatedAt, 10)
            .await
            .expect("empty statuses");
        assert!(
            none.is_empty(),
            "empty status list resolves without SQL error"
        );
        pool.close().await;
        cleanup_files(&path);
    }
}
