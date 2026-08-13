//! Tracking repository (MISSION-019).
//!
//! User state: the per-media `tracking` row and per-node `node_progress`
//! rows. Aggregate progress is derived, never stored. Status transitions and
//! progress math live in the domain layer (MISSION-023/024); this module only
//! persists records.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

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
            repeat_count, current_node_id, current_position, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           core_status = excluded.core_status,
           custom_status_id = excluded.custom_status_id,
           started_at = excluded.started_at,
           finished_at = excluded.finished_at,
           repeat_count = excluded.repeat_count,
           current_node_id = excluded.current_node_id,
           current_position = excluded.current_position,
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
         repeat_count, current_node_id, current_position, updated_at \
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
        updated_at: get(8).expect("updated_at"),
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

        let mut t = tracking("m-1", "completed");
        t.repeat_count = 1;
        t.updated_at = "2026-02-01".into();
        upsert_tracking(&pool, &t).await.expect("re-upsert");

        let got = get_tracking(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(got.core_status, "completed");
        assert_eq!(got.repeat_count, 1);
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
}
