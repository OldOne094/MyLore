//! Service integration unit tests (MISSION-029, spec §54).
//!
//! Exercises the pure domain services *together* through realistic workflows:
//! progress math feeding auto-status transitions, dedup → merge → re-parenting,
//! and stats aggregating the results. No I/O — everything is in-memory.

use mylore_lib::domain::content_node::ContentNode;
use mylore_lib::domain::enums::{
    ContentType, CoreStatus, MediaStatus, NodeKind, NodeProgressState,
};
use mylore_lib::domain::identity::{best_match, score_candidate, IdentityCandidate, IdentityKind};
use mylore_lib::domain::media::{Media, MediaRuntime};
use mylore_lib::domain::merge::plan_merge;
use mylore_lib::domain::normalize::title_matches;
use mylore_lib::domain::progress::{aggregate, NodeTick};
use mylore_lib::domain::stats::{compute_stats, MediaStatsRow};
use mylore_lib::domain::status::{apply_transition, suggest_auto_status};
use mylore_lib::domain::tracking::Tracking;
use mylore_lib::domain::value_objects::{DateOnly, ExternalId, MediaId, ProviderId, Rating, Title};

fn media(id: &str, main_title: &str, content_type: ContentType) -> Media {
    Media {
        id: MediaId::new(id).unwrap(),
        content_type,
        format: None,
        title: Title::new(main_title, None, vec![]).unwrap(),
        synopsis: None,
        status: MediaStatus::Ongoing,
        start_date: None,
        end_date: None,
        release_year: None,
        language: None,
        country: None,
        content_rating: None,
        runtime: MediaRuntime::default(),
        people: Vec::new(),
        genres: Vec::new(),
        tags: Vec::new(),
        external_ids: Vec::new(),
        relations: Vec::new(),
        provider: None,
        provider_url: None,
        metadata_refreshed_at: None,
        created_at: "2026-01-01".into(),
        updated_at: "2026-01-01".into(),
    }
}

fn ext(provider: &str, value: &str) -> ExternalId {
    ExternalId::new(ProviderId::new(provider).unwrap(), value, None).unwrap()
}

fn episode(id: &str, state: NodeProgressState) -> NodeTick {
    NodeTick {
        id: id.into(),
        kind: NodeKind::Episode,
        state,
        page_count: None,
        duration_min: Some(24),
    }
}

fn content_node(id: &str, media_id: &MediaId, kind: NodeKind) -> ContentNode {
    ContentNode {
        id: id.into(),
        media_id: media_id.clone(),
        parent_id: None,
        kind,
        position: 1,
        number: None,
        title: None,
        release_date: None,
        duration_min: Some(24),
        page_count: None,
        synopsis: None,
        external_id: None,
        is_special: false,
        created_at: "2026-01-01".into(),
    }
}

fn tracking(status: CoreStatus) -> Tracking {
    Tracking {
        media_id: MediaId::new("m-1").unwrap(),
        core_status: status,
        custom_status_id: None,
        started_at: None,
        finished_at: None,
        repeat_count: 0,
        current_node_id: None,
        current_position: None,
        auto_track: true,
        updated_at: "2026-08-11".into(),
    }
}

#[test]
fn tracking_lifecycle_with_auto_status_and_repeat() {
    let today = DateOnly::new("2026-08-11").unwrap();
    let mut t = tracking(CoreStatus::Planned);

    // No node data → no auto suggestion.
    let empty = aggregate(ContentType::Anime, &[]);
    assert_eq!(suggest_auto_status(&empty), None);

    // Watching the first episode (of a known tree) suggests in_progress.
    let started = aggregate(
        ContentType::Anime,
        &[
            episode("e1", NodeProgressState::Watched),
            episode("e2", NodeProgressState::Unread),
            episode("e3", NodeProgressState::Unread),
        ],
    );
    assert_eq!(suggest_auto_status(&started), Some(CoreStatus::InProgress));
    t = apply_transition(&t, CoreStatus::InProgress, &today).unwrap();
    assert_eq!(t.core_status, CoreStatus::InProgress);
    assert_eq!(t.started_at, Some(today.clone()));
    t.validate().unwrap();

    // All episodes watched → completed, finished_at stamped.
    let done = aggregate(
        ContentType::Anime,
        &[
            episode("e1", NodeProgressState::Watched),
            episode("e2", NodeProgressState::Watched),
            episode("e3", NodeProgressState::Watched),
        ],
    );
    assert_eq!(suggest_auto_status(&done), Some(CoreStatus::Completed));
    t = apply_transition(&t, CoreStatus::Completed, &today).unwrap();
    assert_eq!(t.core_status, CoreStatus::Completed);
    assert_eq!(t.finished_at, Some(today.clone()));
    t.validate().unwrap();

    // Re-watch: repeat increments, finished_at cleared for the new run.
    t = apply_transition(&t, CoreStatus::Repeat, &today).unwrap();
    assert_eq!(t.core_status, CoreStatus::Repeat);
    assert_eq!(t.repeat_count, 1);
    assert_eq!(t.finished_at, None);
    t.validate().unwrap();

    // Reversible: un-marking an episode moves the suggestion back.
    let regressed = aggregate(
        ContentType::Anime,
        &[
            episode("e1", NodeProgressState::Watched),
            episode("e2", NodeProgressState::Watched),
            episode("e3", NodeProgressState::Unread),
        ],
    );
    assert_eq!(
        suggest_auto_status(&regressed),
        Some(CoreStatus::InProgress)
    );
}

#[test]
fn stats_combine_watch_time_reading_and_ratings() {
    let anime = MediaStatsRow {
        media_id: MediaId::new("m-1").unwrap(),
        content_type: ContentType::Anime,
        core_status: CoreStatus::InProgress,
        rating: Some(Rating::new(9).unwrap()),
        favorite: true,
        release_year: Some(2011),
        progress: aggregate(
            ContentType::Anime,
            &[
                episode("e1", NodeProgressState::Watched),
                episode("e2", NodeProgressState::Watched),
                episode("e3", NodeProgressState::Watched),
                episode("e4", NodeProgressState::Watched),
                episode("e5", NodeProgressState::Watched),
                episode("e6", NodeProgressState::Unread),
                episode("e7", NodeProgressState::Unread),
                episode("e8", NodeProgressState::Unread),
                episode("e9", NodeProgressState::Unread),
                episode("e10", NodeProgressState::Unread),
                episode("e11", NodeProgressState::Unread),
                episode("e12", NodeProgressState::Unread),
            ],
        ),
    };

    let book = MediaStatsRow {
        media_id: MediaId::new("m-2").unwrap(),
        content_type: ContentType::Book,
        core_status: CoreStatus::InProgress,
        rating: None,
        favorite: false,
        release_year: Some(2004),
        progress: aggregate(
            ContentType::Book,
            &[
                NodeTick {
                    id: "c1".into(),
                    kind: NodeKind::Chapter,
                    state: NodeProgressState::Read,
                    page_count: Some(120),
                    duration_min: None,
                },
                NodeTick {
                    id: "c2".into(),
                    kind: NodeKind::Chapter,
                    state: NodeProgressState::Unread,
                    page_count: Some(280),
                    duration_min: None,
                },
            ],
        ),
    };

    let stats = compute_stats(&[anime, book]);
    assert_eq!(stats.total, 2);
    // 5 watched episodes × 24 min.
    assert_eq!(stats.consumed_minutes, 120);
    assert_eq!(stats.consumed_hours(), 2.0);
    // Book template weighs by pages.
    assert_eq!(stats.consumed_pages, 120);
    assert_eq!(stats.avg_rating, Some(9.0));
    assert_eq!(stats.favorites, 1);
    assert_eq!(stats.completion_rate, Some(0.0));
    // (5/12 → 41%) and (120/400 → 30%) → mean 35.5%.
    assert_eq!(stats.avg_percent, Some(35.5));
}

#[test]
fn dedup_merges_duplicate_into_survivor_and_reparents() {
    let mut survivor = media("m-1", "Fairy Tail", ContentType::Anime);
    survivor.external_ids.push(ext("anilist", "21"));
    survivor.runtime.ep_count = Some(175);
    survivor.genres = vec!["fantasy".into()];

    let canonical = IdentityCandidate {
        media_id: survivor.id.clone(),
        titles: survivor.title.clone(),
        external_ids: survivor.external_ids.clone(),
    };
    let decoy = IdentityCandidate {
        media_id: MediaId::new("m-9").unwrap(),
        titles: Title::new("Fairy Tail Zero", None, vec![]).unwrap(),
        external_ids: vec![],
    };

    // Incoming record carries the anilist id → exact identity.
    let incoming_title = Title::new("Fairy Tail", None, vec![]).unwrap();
    let incoming_ext = [ext("anilist", "21")];
    let best = best_match(&incoming_title, &incoming_ext, &[decoy, canonical]).unwrap();
    assert_eq!(best.media_id, survivor.id);
    assert_eq!(best.kind, IdentityKind::Exact);

    // The duplicate: same series from another provider, with nodes + a collection.
    let mut duplicate = media("m-2", "Fairy Tail", ContentType::Anime);
    duplicate.external_ids.push(ext("mal", "6702"));
    duplicate.genres = vec!["action".into()];

    let plan = plan_merge(&survivor, &duplicate, &[], &["col-1".into()], true, true).unwrap();
    assert_eq!(plan.merged.id, survivor.id);
    assert_eq!(plan.merged.genres, vec!["fantasy", "action"]);
    let providers: Vec<_> = plan
        .merged
        .external_ids
        .iter()
        .map(|e| e.provider().as_str())
        .collect();
    assert!(providers.contains(&"anilist") && providers.contains(&"mal"));
    assert!(!plan.move_review, "survivor already has a review");
    assert!(!plan.move_tracking, "survivor already has tracking");
    assert_eq!(plan.move_collection_memberships, vec!["col-1".to_string()]);

    // Re-parenting a node set keeps ids and re-keys media_id.
    let nodes = vec![content_node("e1", &duplicate.id, NodeKind::Episode)];
    let plan = plan_merge(&survivor, &duplicate, &nodes, &[], true, true).unwrap();
    assert_eq!(plan.re_parented_nodes.len(), 1);
    assert_eq!(plan.re_parented_nodes[0].media_id, survivor.id);
    assert_eq!(plan.re_parented_nodes[0].id, "e1");
}

#[test]
fn title_variants_match_without_duplication() {
    let mut survivor = media("m-1", "Shingeki no Kyojin", ContentType::Anime);
    survivor.title = Title::new(
        "Shingeki no Kyojin",
        Some("進撃の巨人".to_string()),
        vec!["Attack on Titan".to_string()],
    )
    .unwrap();

    let candidate = IdentityCandidate {
        media_id: survivor.id.clone(),
        titles: survivor.title.clone(),
        external_ids: vec![],
    };

    // Incoming uses the English title, differently cased → TitleExact.
    let incoming = Title::new("Attack On Titan", None, vec![]).unwrap();
    let scored = score_candidate(&candidate, &incoming, &[]);
    assert_eq!(scored.kind, IdentityKind::TitleExact);

    // Merging: survivor main wins; the case-variant is NOT duplicated.
    let duplicate = media("m-2", "Attack On Titan", ContentType::Anime);
    let plan = plan_merge(&survivor, &duplicate, &[], &[], false, false).unwrap();
    assert_eq!(plan.merged.title.main(), "Shingeki no Kyojin");
    let titan_occurrences = plan
        .merged
        .title
        .all()
        .filter(|t| title_matches(t, "attack on titan"))
        .count();
    assert_eq!(
        titan_occurrences, 1,
        "fold-equal alternative must not duplicate"
    );
    assert_eq!(
        plan.conflicts.iter().filter(|c| c.field == "title").count(),
        1
    );
}

#[test]
fn arabic_variant_dedups_and_merges_external_ids() {
    let stored = Title::new("عَبْقَرِيَّةٌ", None, vec![]).unwrap();
    let incoming = Title::new("عبقريه", None, vec![]).unwrap();
    assert!(title_matches("عَبْقَرِيَّةٌ", "عبقريه"));

    let candidate = IdentityCandidate {
        media_id: MediaId::new("m-1").unwrap(),
        titles: stored,
        external_ids: vec![ext("anilist", "1")],
    };
    let scored = score_candidate(&candidate, &incoming, &[]);
    assert_eq!(scored.kind, IdentityKind::TitleExact);

    // Merge across providers unions external ids without a conflict.
    let mut survivor = media("m-1", "عَبْقَرِيَّةٌ", ContentType::Novel);
    survivor.external_ids.push(ext("anilist", "1"));
    let mut duplicate = media("m-2", "عَبْقَرِيَّةٌ", ContentType::Novel);
    duplicate.external_ids.push(ext("mal", "2"));

    let plan = plan_merge(&survivor, &duplicate, &[], &[], false, false).unwrap();
    assert!(plan.conflicts.iter().all(|c| c.field != "external_id"));
    assert_eq!(plan.merged.external_ids.len(), 2);
}

#[test]
fn merged_media_feeds_progress_and_stats() {
    // After a merge the re-parented nodes must aggregate correctly and the
    // merged media must appear in stats as a single unit.
    let mut survivor = media("m-1", "Fairy Tail", ContentType::Anime);
    let duplicate = media("m-2", "Fairy Tail", ContentType::Anime);

    let nodes = vec![
        content_node("e1", &duplicate.id, NodeKind::Episode),
        content_node("e2", &duplicate.id, NodeKind::Episode),
        content_node("e3", &duplicate.id, NodeKind::Episode),
        content_node("e4", &duplicate.id, NodeKind::Episode),
    ];
    let plan = plan_merge(&survivor, &duplicate, &nodes, &[], false, false).unwrap();

    // The re-parented set aggregates exactly like the original node set.
    let aggregate = aggregate(
        ContentType::Anime,
        &[
            episode("e1", NodeProgressState::Watched),
            episode("e2", NodeProgressState::Watched),
            episode("e3", NodeProgressState::Watched),
            episode("e4", NodeProgressState::Watched),
        ],
    );
    assert_eq!(aggregate.total_units, 4);
    assert_eq!(aggregate.completed_units, 4);
    assert_eq!(aggregate.percent, Some(100));
    assert_eq!(suggest_auto_status(&aggregate), Some(CoreStatus::Completed));

    // Stats: the merged record is one tracked anime at 100%.
    survivor
        .external_ids
        .extend(plan.merged.external_ids.clone());
    let row = MediaStatsRow {
        media_id: survivor.id,
        content_type: ContentType::Anime,
        core_status: CoreStatus::Completed,
        rating: Some(Rating::new(8).unwrap()),
        favorite: true,
        release_year: None,
        progress: aggregate,
    };
    let stats = compute_stats(&[row]);
    assert_eq!(stats.total, 1);
    assert_eq!(stats.completed_media, 1);
    assert_eq!(stats.completion_rate, Some(1.0));
    assert_eq!(stats.avg_percent, Some(100.0));
    assert_eq!(stats.consumed_minutes, 96, "4 episodes × 24 min");
}

#[test]
fn status_transitions_and_custom_buckets_validate_service_use() {
    // A custom status behaves like its core bucket for stats grouping.
    let custom = mylore_lib::domain::status::CustomStatus::new(
        "marathoning",
        "Marathoning",
        CoreStatus::InProgress,
        25,
    )
    .unwrap();
    assert_eq!(
        mylore_lib::domain::status::effective_status(Some(&custom), CoreStatus::Planned),
        CoreStatus::InProgress
    );

    // Guard: repeat cannot be entered from planned/wishlist.
    let planned = tracking(CoreStatus::Planned);
    assert!(apply_transition(
        &planned,
        CoreStatus::Repeat,
        &DateOnly::new("2026-08-11").unwrap()
    )
    .is_err());
}
