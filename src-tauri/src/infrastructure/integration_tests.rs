//! MISSION-020: DB integration tests over the real schema + repositories.
//!
//! Where the per-module repository tests prove one aggregate in isolation,
//! these tests exercise cross-aggregate flows on a fully-migrated database:
//! lifecycle + cascades, manual transactions, repo-internal atomicity, FTS
//! consistency across related writes, and FK rejection at every layer.

use sqlx::sqlite::SqlitePool;

use crate::error::AppError;
use crate::infrastructure::repositories::{activity, collection, media, node, review, tracking};
use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

fn sample_media(id: &str, title: &str) -> media::MediaRecord {
    media::MediaRecord {
        id: id.to_string(),
        content_type: "novel".to_string(),
        format: Some("light_novel".to_string()),
        title_main: title.to_string(),
        title_original: None,
        synopsis: Some("A test synopsis".to_string()),
        pub_status: "ongoing".to_string(),
        start_date: Some("2025-01-01".to_string()),
        end_date: None,
        release_year: Some(2025),
        language: Some("ja".to_string()),
        country: None,
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: None,
        ch_count: Some(120),
        cover_asset_id: None,
        banner_asset_id: None,
        provider: Some("anilist".to_string()),
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
    }
}

async fn ensure_person(pool: &SqlitePool) {
    sqlx::query("INSERT INTO person (id, name, role) VALUES ('p-1', 'Test Author', 'author')")
        .execute(pool)
        .await
        .expect("seed person");
}

fn sample_node(id: &str, media_id: &str, parent_id: Option<&str>, kind: &str) -> node::NodeRecord {
    node::NodeRecord {
        id: id.to_string(),
        media_id: media_id.to_string(),
        parent_id: parent_id.map(str::to_string),
        kind: kind.to_string(),
        position: 1,
        number: None,
        title: None,
        release_date: None,
        duration_min: None,
        page_count: None,
        synopsis: None,
        external_id: None,
        is_special: false,
        created_at: "2026-01-01".to_string(),
    }
}

async fn count_rows(pool: &SqlitePool, sql: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(sql).fetch_one(pool).await.expect("count");
    n
}

/// Create every aggregate around one media, then delete the media and verify
/// the whole graph cascades while unrelated rows (a second media and its
/// collection) survive.
#[tokio::test]
async fn full_lifecycle_across_aggregates_cascades_on_delete() {
    let (pool, path) = migrated_pool("integration_lifecycle.db").await;
    ensure_person(&pool).await;

    // A second, unrelated media to prove we only delete the target graph.
    media::create(&pool, &sample_media("m-2", "Unrelated"))
        .await
        .expect("create m-2");

    let mut m1 = sample_media("m-1", "Sword of the Dawn");
    m1.alt_titles.push(media::AltTitle {
        lang: "ja".into(),
        title: "??????".into(),
    });
    m1.people.push("p-1".to_string());
    m1.genres.push("fantasy".to_string());
    m1.tags.push("isekai".to_string());
    m1.external_ids.push(media::ExternalId {
        provider: "anilist".into(),
        ext_id: "42".into(),
        url: Some("https://anilist.co/anime/42".into()),
    });
    m1.relations.push(media::MediaRelation {
        to_id: "m-2".into(),
        relation: "sequel".into(),
    });
    media::create(&pool, &m1).await.expect("create m-1");

    // Node tree with a descendant.
    node::create(&pool, &sample_node("v-1", "m-1", None, "volume"))
        .await
        .expect("create volume");
    node::create(&pool, &sample_node("c-1", "m-1", Some("v-1"), "chapter"))
        .await
        .expect("create chapter");

    tracking::upsert_tracking(
        &pool,
        &tracking::TrackingRecord {
            media_id: "m-1".into(),
            core_status: "in_progress".into(),
            custom_status_id: None,
            started_at: Some("2026-01-01".into()),
            finished_at: None,
            repeat_count: 0,
            current_node_id: Some("c-1".into()),
            current_position: Some(12),
            auto_track: 1,
            updated_at: "2026-01-01".into(),
        },
    )
    .await
    .expect("tracking");

    tracking::set_progress(
        &pool,
        &tracking::NodeProgress {
            node_id: "c-1".into(),
            state: "read".into(),
            read_at: Some("2026-01-02".into()),
            note: None,
            rating: Some(9),
            updated_at: "2026-01-02".into(),
        },
    )
    .await
    .expect("node progress");

    review::upsert(
        &pool,
        &review::ReviewRecord {
            media_id: "m-1".into(),
            rating: Some(9),
            review: Some("A sweeping epic".into()),
            short_review: None,
            notes: Some("re-read in 2027".into()),
            favorite: true,
            is_spoiler: false,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        },
    )
    .await
    .expect("review");

    collection::create(
        &pool,
        &collection::CollectionRecord {
            id: "col-1".into(),
            name: "Favorites".into(),
            is_smart: false,
            filter_def: None,
            sort_order: 0,
            created_at: "2026-01-01".into(),
        },
    )
    .await
    .expect("collection");
    collection::add_member(&pool, "col-1", "m-1", 0, "2026-01-01")
        .await
        .expect("add member");

    activity::log(
        &pool,
        &activity::ActivityRecord {
            id: "act-1".into(),
            media_id: Some("m-1".into()),
            node_id: Some("c-1".into()),
            kind: "progress".into(),
            meta: Some("in_progress".into()),
            created_at: "2026-01-02".into(),
        },
    )
    .await
    .expect("activity");

    // Everything is reachable before the delete, including through FTS.
    assert_eq!(
        media::search(&pool, "sweeping")
            .await
            .expect("search")
            .len(),
        1
    );
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM review").await, 1);
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM content_node").await,
        2
    );
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM collection_member").await,
        1
    );

    media::delete(&pool, "m-1").await.expect("delete m-1");

    // The target aggregate is gone in every table.
    assert!(media::get(&pool, "m-1").await.expect("get m-1").is_none());
    assert!(media::search(&pool, "sweeping")
        .await
        .expect("search")
        .is_empty());
    assert!(media::search(&pool, "sword")
        .await
        .expect("search")
        .is_empty());
    for (sql, name) in [
        ("SELECT COUNT(*) FROM media_alt_title", "alt titles"),
        ("SELECT COUNT(*) FROM media_person", "media_person"),
        ("SELECT COUNT(*) FROM media_genre", "media_genre"),
        ("SELECT COUNT(*) FROM media_tag", "media_tag"),
        ("SELECT COUNT(*) FROM media_external_id", "external ids"),
        ("SELECT COUNT(*) FROM media_relation", "media_relation"),
        ("SELECT COUNT(*) FROM content_node", "content_node"),
        ("SELECT COUNT(*) FROM node_progress", "node_progress"),
        ("SELECT COUNT(*) FROM tracking", "tracking"),
        ("SELECT COUNT(*) FROM review", "review"),
        (
            "SELECT COUNT(*) FROM collection_member",
            "collection_member",
        ),
        ("SELECT COUNT(*) FROM activity", "activity"),
    ] {
        assert_eq!(
            count_rows(&pool, sql).await,
            0,
            "{name} should cascade with media"
        );
    }

    // Unrelated rows survive.
    assert!(media::get(&pool, "m-2").await.expect("get m-2").is_some());
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM collection").await,
        1,
        "collection survives"
    );
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM person").await,
        1,
        "person survives"
    );

    pool.close().await;
    cleanup_files(&path);
}

/// A manual transaction rolls back every statement inside it; a committed
/// transaction persists, and the FTS index follows the committed state.
#[tokio::test]
async fn manual_transaction_rolls_back_all_statements() {
    let (pool, path) = migrated_pool("integration_tx.db").await;

    {
        let mut tx = pool.begin().await.expect("begin");
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Rollback Me', '2026-01-01', '2026-01-01')",
        )
        .execute(&mut *tx)
        .await
        .expect("insert inside tx");

        // A statement inside the tx violates a FK; the tx is dropped without
        // commit, so the earlier insert must not persist.
        let result = sqlx::query(
            "INSERT INTO media_genre (media_id, genre_id) VALUES ('m-missing', 'fantasy')",
        )
        .execute(&mut *tx)
        .await;
        assert!(result.is_err(), "FK violation inside tx");
        drop(tx);
    }

    assert!(media::get(&pool, "m-1").await.expect("get").is_none());
    assert!(media::search(&pool, "rollback")
        .await
        .expect("search")
        .is_empty());

    // Committing the same sequence persists and lands in the FTS index.
    {
        let mut tx = pool.begin().await.expect("begin");
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Kept Forever', '2026-01-01', '2026-01-01')",
        )
        .execute(&mut *tx)
        .await
        .expect("insert inside tx");
        tx.commit().await.expect("commit");
    }

    assert!(media::get(&pool, "m-1").await.expect("get").is_some());
    let hits = media::search(&pool, "forever").await.expect("search");
    assert_eq!(hits.len(), 1, "committed rows are indexed");
    assert_eq!(hits[0].id, "m-1");

    pool.close().await;
    cleanup_files(&path);
}

/// `media::create` is one transaction: a failing link insert must roll back the
/// media row, its links, and the FTS entry.
#[tokio::test]
async fn media_create_is_atomic_when_a_link_fails() {
    let (pool, path) = migrated_pool("integration_create_atomic.db").await;

    let mut m = sample_media("m-1", "Doomed");
    m.alt_titles.push(media::AltTitle {
        lang: "ja".into(),
        title: "??????".into(),
    });
    m.people.push("p-missing".to_string());

    let result = media::create(&pool, &m).await;
    assert!(result.is_err(), "link to missing person must fail");
    assert!(matches!(result, Err(AppError::Database(_))));

    // Nothing persisted — not even the earlier inserts inside the tx.
    assert!(media::get(&pool, "m-1").await.expect("get").is_none());
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM media_alt_title").await,
        0
    );
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM media").await, 0);
    assert!(media::search(&pool, "doomed")
        .await
        .expect("search")
        .is_empty());

    pool.close().await;
    cleanup_files(&path);
}

/// `media::update` rewrites links in one transaction; a failure mid-way leaves
/// the previous aggregate untouched.
#[tokio::test]
async fn media_update_is_atomic_when_a_link_fails() {
    let (pool, path) = migrated_pool("integration_update_atomic.db").await;
    ensure_person(&pool).await;

    let mut m = sample_media("m-1", "Sword of the Dawn");
    m.alt_titles.push(media::AltTitle {
        lang: "ja".into(),
        title: "??????".into(),
    });
    m.people.push("p-1".to_string());
    media::create(&pool, &m).await.expect("create");

    // Attempt an update whose link set references a missing person.
    let mut bad = m.clone();
    bad.title_main = "Dawn of the Sword".into();
    bad.people.clear();
    bad.people.push("p-missing".to_string());
    bad.updated_at = "2026-02-01".into();
    let result = media::update(&pool, &bad).await;
    assert!(result.is_err(), "link to missing person must fail");

    // The previous aggregate (and its links) is untouched.
    let got = media::get(&pool, "m-1").await.expect("get").unwrap();
    assert_eq!(got.title_main, "Sword of the Dawn");
    assert_eq!(got.updated_at, "2026-01-01");
    assert_eq!(got.people, vec!["p-1".to_string()]);
    assert_eq!(got.alt_titles.len(), 1);

    pool.close().await;
    cleanup_files(&path);
}

/// Review content is indexed and the FTS index follows review edits.
#[tokio::test]
async fn fts_index_follows_review_content_changes() {
    let (pool, path) = migrated_pool("integration_fts_review.db").await;
    media::create(&pool, &sample_media("m-1", "Ghost Town"))
        .await
        .expect("create");

    review::upsert(
        &pool,
        &review::ReviewRecord {
            media_id: "m-1".into(),
            rating: None,
            review: Some("sweet sorrow symphony".into()),
            short_review: None,
            notes: None,
            favorite: false,
            is_spoiler: false,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        },
    )
    .await
    .expect("review");

    let hits = media::search(&pool, "sorrow").await.expect("search");
    assert_eq!(hits.len(), 1, "review body is searchable");
    assert_eq!(hits[0].id, "m-1");

    // Clear the review body: the old terms must leave the index.
    review::upsert(
        &pool,
        &review::ReviewRecord {
            media_id: "m-1".into(),
            rating: Some(8),
            review: None,
            short_review: Some("fine".into()),
            notes: None,
            favorite: false,
            is_spoiler: false,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-02".into(),
        },
    )
    .await
    .expect("review update");

    assert!(media::search(&pool, "sorrow")
        .await
        .expect("search")
        .is_empty());
    assert_eq!(
        media::search(&pool, "ghost").await.expect("search").len(),
        1
    );

    // Deleting the review must not drop the media document.
    review::delete(&pool, "m-1").await.expect("delete review");
    assert_eq!(
        media::search(&pool, "ghost").await.expect("search").len(),
        1
    );

    pool.close().await;
    cleanup_files(&path);
}

/// Orphan writes are rejected at the repository boundary everywhere.
#[tokio::test]
async fn fk_violations_are_rejected_across_aggregates() {
    let (pool, path) = migrated_pool("integration_fk.db").await;

    // A root node for a missing media: the FK on content_node.media_id fires.
    let result = node::create(&pool, &sample_node("n-1", "m-missing", None, "chapter")).await;
    assert!(result.is_err());

    let result = tracking::upsert_tracking(
        &pool,
        &tracking::TrackingRecord {
            media_id: "m-missing".into(),
            core_status: "planned".into(),
            custom_status_id: None,
            started_at: None,
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: None,
            auto_track: 1,
            updated_at: "2026-01-01".into(),
        },
    )
    .await;
    assert!(result.is_err(), "tracking for missing media rejected");

    let result = review::upsert(
        &pool,
        &review::ReviewRecord {
            media_id: "m-missing".into(),
            rating: None,
            review: None,
            short_review: None,
            notes: None,
            favorite: false,
            is_spoiler: false,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        },
    )
    .await;
    assert!(result.is_err(), "review for missing media rejected");

    collection::create(
        &pool,
        &collection::CollectionRecord {
            id: "col-1".into(),
            name: "Empty".into(),
            is_smart: false,
            filter_def: None,
            sort_order: 0,
            created_at: "2026-01-01".into(),
        },
    )
    .await
    .expect("collection");
    let result = collection::add_member(&pool, "col-1", "m-missing", 0, "2026-01-01").await;
    assert!(result.is_err(), "member for missing media rejected");

    let result = activity::log(
        &pool,
        &activity::ActivityRecord {
            id: "act-1".into(),
            media_id: Some("m-missing".into()),
            node_id: None,
            kind: "media.deleted".into(),
            meta: None,
            created_at: "2026-01-01".into(),
        },
    )
    .await;
    assert!(result.is_err(), "activity for missing media rejected");

    // Nothing was partially written by the rejected calls.
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM media").await, 0);
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM tracking").await, 0);
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM review").await, 0);
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM content_node").await,
        0
    );
    assert_eq!(
        count_rows(&pool, "SELECT COUNT(*) FROM collection_member").await,
        0
    );
    assert_eq!(count_rows(&pool, "SELECT COUNT(*) FROM activity").await, 0);

    pool.close().await;
    cleanup_files(&path);
}
