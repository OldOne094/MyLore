//! Merge service (MISSION-028).
//!
//! Plans a merge of a duplicate `Media` into a survivor (canonical) `Media`:
//!   - **merged metadata** — the survivor's identity is kept; scalars prefer
//!     the survivor and fall back to the duplicate; sets (titles, genres, tags,
//!     people, relations, external ids) are unioned,
//!   - **conflict report** — every field where both records carry *different*
//!     non-empty values (the application surfaces these to the user),
//!   - **re-parenting** — the duplicate's content nodes are re-keyed onto the
//!     survivor (parent links stay valid: ids are unchanged),
//!   - **moves** — duplicate's review/tracking are moved only when the survivor
//!     has none; collection memberships are always re-keyed,
//!   - **before-image** — snapshots of both records so the merge can be undone.
//!
//! Pure and side-effect free: the caller (a service with repository access)
//! applies the plan inside one transaction.

use std::collections::HashSet;

use crate::domain::content_node::ContentNode;
use crate::domain::error::DomainError;
use crate::domain::media::{Media, MediaRelation, MediaRuntime, PersonCredit};
use crate::domain::normalize::fold_title;
use crate::domain::value_objects::{ExternalId, Title};

/// A field-level conflict between the survivor and the duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldConflict {
    /// Stable field name for the UI (e.g. "synopsis", "release_year").
    pub field: &'static str,
    pub survivor: String,
    pub duplicate: String,
}

/// A snapshot of both records before any change, enabling undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeforeImage {
    pub survivor: Media,
    pub duplicate: Media,
}

/// The result of planning a merge.
#[derive(Debug, Clone)]
pub struct MergePlan {
    /// Resolved metadata (survivor identity, survivor-preferred scalars, unions).
    pub merged: Media,
    /// Fields where both records had different non-empty values.
    pub conflicts: Vec<FieldConflict>,
    /// The duplicate's nodes re-keyed onto the survivor.
    pub re_parented_nodes: Vec<ContentNode>,
    /// Whether the duplicate's review should be moved (survivor has none).
    pub move_review: bool,
    /// Whether the duplicate's tracking should be moved (survivor has none).
    pub move_tracking: bool,
    /// Collection memberships to re-key from the duplicate onto the survivor.
    pub move_collection_memberships: Vec<String>,
    /// Both records before any change (undo).
    pub before: BeforeImage,
}

/// Plan a merge of `duplicate` into `survivor`.
///
/// `duplicate_nodes` and `duplicate_collection_ids` are the duplicate's owned
/// rows; `survivor_has_review` / `survivor_has_tracking` tell the planner
/// whether moving those aggregates would collide.
pub fn plan_merge(
    survivor: &Media,
    duplicate: &Media,
    duplicate_nodes: &[ContentNode],
    duplicate_collection_ids: &[String],
    survivor_has_review: bool,
    survivor_has_tracking: bool,
) -> Result<MergePlan, DomainError> {
    let mut conflicts = Vec::new();

    if survivor.content_type != duplicate.content_type {
        conflicts.push(FieldConflict {
            field: "content_type",
            survivor: survivor.content_type.as_str().to_string(),
            duplicate: duplicate.content_type.as_str().to_string(),
        });
    }

    push_option_conflict(
        &mut conflicts,
        "format",
        survivor.format.as_deref(),
        duplicate.format.as_deref(),
    );
    push_option_conflict(
        &mut conflicts,
        "synopsis",
        survivor.synopsis.as_deref(),
        duplicate.synopsis.as_deref(),
    );
    push_option_conflict(
        &mut conflicts,
        "start_date",
        survivor.start_date.as_ref().map(|d| d.as_str()),
        duplicate.start_date.as_ref().map(|d| d.as_str()),
    );
    push_option_conflict(
        &mut conflicts,
        "end_date",
        survivor.end_date.as_ref().map(|d| d.as_str()),
        duplicate.end_date.as_ref().map(|d| d.as_str()),
    );
    push_option_conflict(
        &mut conflicts,
        "release_year",
        survivor.release_year.map(|y| y.to_string()).as_deref(),
        duplicate.release_year.map(|y| y.to_string()).as_deref(),
    );
    push_option_conflict(
        &mut conflicts,
        "language",
        survivor.language.as_ref().map(|l| l.as_str()),
        duplicate.language.as_ref().map(|l| l.as_str()),
    );
    push_option_conflict(
        &mut conflicts,
        "country",
        survivor.country.as_deref(),
        duplicate.country.as_deref(),
    );
    push_option_conflict(
        &mut conflicts,
        "content_rating",
        survivor.content_rating.as_deref(),
        duplicate.content_rating.as_deref(),
    );
    push_option_conflict(
        &mut conflicts,
        "provider",
        survivor.provider.as_ref().map(|p| p.as_str()),
        duplicate.provider.as_ref().map(|p| p.as_str()),
    );
    push_option_conflict(
        &mut conflicts,
        "provider_url",
        survivor.provider_url.as_deref(),
        duplicate.provider_url.as_deref(),
    );
    if fold_title(survivor.title.main()) != fold_title(duplicate.title.main()) {
        conflicts.push(FieldConflict {
            field: "title",
            survivor: survivor.title.main().to_string(),
            duplicate: duplicate.title.main().to_string(),
        });
    }

    // Sets: external ids must stay provider-unique on the survivor.
    for external_id in &duplicate.external_ids {
        match survivor
            .external_ids
            .iter()
            .find(|e| e.provider() == external_id.provider())
        {
            Some(existing) if existing.value() != external_id.value() => {
                conflicts.push(FieldConflict {
                    field: "external_id",
                    survivor: format!("{}:{}", existing.provider().as_str(), existing.value()),
                    duplicate: format!(
                        "{}:{}",
                        external_id.provider().as_str(),
                        external_id.value()
                    ),
                })
            }
            _ => {}
        }
    }

    let mut merged = survivor.clone();
    merged.title = merge_titles(&survivor.title, &duplicate.title)?;
    merged.synopsis = survivor
        .synopsis
        .clone()
        .or_else(|| duplicate.synopsis.clone());
    merged.format = survivor.format.clone().or_else(|| duplicate.format.clone());
    merged.start_date = survivor
        .start_date
        .clone()
        .or_else(|| duplicate.start_date.clone());
    merged.end_date = survivor
        .end_date
        .clone()
        .or_else(|| duplicate.end_date.clone());
    merged.release_year = survivor.release_year.or(duplicate.release_year);
    merged.language = survivor
        .language
        .clone()
        .or_else(|| duplicate.language.clone());
    merged.country = survivor
        .country
        .clone()
        .or_else(|| duplicate.country.clone());
    merged.content_rating = survivor
        .content_rating
        .clone()
        .or_else(|| duplicate.content_rating.clone());
    merged.runtime = merge_runtime(&survivor.runtime, &duplicate.runtime);
    merged.genres = union_strings(&survivor.genres, &duplicate.genres);
    merged.tags = union_strings(&survivor.tags, &duplicate.tags);
    merged.people = union_people(&survivor.people, &duplicate.people);
    merged.relations = union_relations(&survivor.relations, &duplicate.relations, &survivor.id);
    merged.external_ids = merge_external_ids(&survivor.external_ids, &duplicate.external_ids);
    merged.validate()?;

    let re_parented_nodes = duplicate_nodes
        .iter()
        .map(|node| ContentNode {
            media_id: survivor.id.clone(),
            ..node.clone()
        })
        .collect();

    Ok(MergePlan {
        merged,
        conflicts,
        re_parented_nodes,
        move_review: !survivor_has_review,
        move_tracking: !survivor_has_tracking,
        move_collection_memberships: duplicate_collection_ids.to_vec(),
        before: BeforeImage {
            survivor: survivor.clone(),
            duplicate: duplicate.clone(),
        },
    })
}

/// Merge two titles: survivor main + original wins; every alternative title of
/// both sides is unioned (case-insensitively) and the duplicate's main title is
/// added as an alternative when it differs from the survivor's main title.
fn merge_titles(survivor: &Title, duplicate: &Title) -> Result<Title, DomainError> {
    let main = survivor.main();
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(main.to_lowercase());

    let original = match survivor.original() {
        Some(original) => {
            seen.insert(original.to_lowercase());
            Some(original.to_string())
        }
        None => duplicate.original().map(|original| {
            seen.insert(original.to_lowercase());
            original.to_string()
        }),
    };

    let mut alternatives = Vec::new();
    let push_alt = |title: &str, seen: &mut HashSet<String>, out: &mut Vec<String>| {
        let lower = title.to_lowercase();
        if !title.trim().is_empty() && seen.insert(lower) {
            out.push(title.to_string());
        }
    };
    for alt in survivor.alternatives() {
        push_alt(alt, &mut seen, &mut alternatives);
    }
    for alt in duplicate.alternatives() {
        push_alt(alt, &mut seen, &mut alternatives);
    }
    if fold_title(duplicate.main()) != fold_title(main) {
        push_alt(duplicate.main(), &mut seen, &mut alternatives);
    }

    Title::new(main, original, alternatives)
}

fn merge_runtime(survivor: &MediaRuntime, duplicate: &MediaRuntime) -> MediaRuntime {
    MediaRuntime {
        pages: survivor.pages.or(duplicate.pages),
        duration_min: survivor.duration_min.or(duplicate.duration_min),
        ep_count: survivor.ep_count.or(duplicate.ep_count),
        ch_count: survivor.ch_count.or(duplicate.ch_count),
    }
}

fn union_strings(first: &[String], second: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    first
        .iter()
        .chain(second)
        .filter(|value| seen.insert(value.to_lowercase()))
        .cloned()
        .collect()
}

fn union_people(first: &[PersonCredit], second: &[PersonCredit]) -> Vec<PersonCredit> {
    let mut seen: HashSet<String> = HashSet::new();
    first
        .iter()
        .chain(second)
        .filter(|person| seen.insert(person.person_id.clone()))
        .cloned()
        .collect()
}

fn union_relations(
    first: &[MediaRelation],
    second: &[MediaRelation],
    survivor_id: &crate::domain::value_objects::MediaId,
) -> Vec<MediaRelation> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    first
        .iter()
        .chain(second)
        .filter(|relation| relation.to_id != *survivor_id)
        .filter(|relation| {
            seen.insert((
                relation.to_id.as_str().to_string(),
                relation.kind.as_str().to_string(),
            ))
        })
        .cloned()
        .collect()
}

fn merge_external_ids(first: &[ExternalId], second: &[ExternalId]) -> Vec<ExternalId> {
    let mut providers: HashSet<String> = HashSet::new();
    first
        .iter()
        .chain(second)
        .filter(|id| {
            let provider = id.provider().as_str().to_string();
            providers.insert(provider)
        })
        .cloned()
        .collect()
}

fn push_option_conflict(
    conflicts: &mut Vec<FieldConflict>,
    field: &'static str,
    survivor: Option<&str>,
    duplicate: Option<&str>,
) {
    if let (Some(survivor), Some(duplicate)) = (survivor, duplicate) {
        if survivor != duplicate {
            conflicts.push(FieldConflict {
                field,
                survivor: survivor.to_string(),
                duplicate: duplicate.to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::{ContentType, MediaRelationKind, MediaStatus};
    use crate::domain::value_objects::{MediaId, ProviderId};

    fn media(id: &str, main_title: &str) -> Media {
        Media {
            id: MediaId::new(id).unwrap(),
            content_type: ContentType::Novel,
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

    fn plan(survivor: &Media, duplicate: &Media) -> MergePlan {
        plan_merge(survivor, duplicate, &[], &[], false, false).unwrap()
    }

    #[test]
    fn survivor_identity_is_kept_and_scalars_prefer_survivor() {
        let mut survivor = media("m-1", "Fairy Tail");
        survivor.synopsis = Some("the survivor synopsis".into());
        survivor.release_year = Some(2009);

        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.synopsis = Some("the duplicate synopsis".into());
        duplicate.release_year = Some(2011);

        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.id, survivor.id);
        assert_eq!(
            result.merged.synopsis.as_deref(),
            Some("the survivor synopsis")
        );
        assert_eq!(result.merged.release_year, Some(2009));
    }

    #[test]
    fn conflicts_are_reported_only_for_different_non_empty_values() {
        let mut survivor = media("m-1", "Fairy Tail");
        survivor.release_year = Some(2009);
        survivor.country = Some("JP".into());

        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.release_year = Some(2011);

        let result = plan(&survivor, &duplicate);
        let fields: Vec<_> = result.conflicts.iter().map(|c| c.field).collect();
        assert!(
            fields.contains(&"release_year"),
            "different values conflict"
        );
        assert!(
            !fields.contains(&"country"),
            "duplicate had no value -> no conflict"
        );
        assert!(!fields.contains(&"synopsis"), "both None -> no conflict");
    }

    #[test]
    fn differing_main_titles_conflict() {
        let survivor = media("m-1", "Shingeki no Kyojin");
        let duplicate = media("m-2", "Attack on Titan");
        let result = plan(&survivor, &duplicate);
        assert!(result.conflicts.iter().any(|c| c.field == "title"));
    }

    #[test]
    fn duplicate_main_title_becomes_an_alternative() {
        let survivor = media("m-1", "Shingeki no Kyojin");
        let duplicate = media("m-2", "Attack on Titan");
        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.title.main(), "Shingeki no Kyojin");
        assert!(result.merged.title.all().any(|t| t == "Attack on Titan"));
        assert!(result.merged.validate().is_ok());
    }

    #[test]
    fn same_main_title_is_not_duplicated() {
        let survivor = media("m-1", "Fairy Tail");
        let duplicate = media("m-2", "Fairy Tail");
        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.title.main(), "Fairy Tail");
        assert_eq!(
            result.merged.title.alternatives().len(),
            0,
            "same main title must not be added as an alternative"
        );
    }

    #[test]
    fn sets_are_unioned_and_deduped() {
        let mut survivor = media("m-1", "Fairy Tail");
        survivor.genres = vec!["fantasy".into(), "action".into()];
        survivor.tags = vec!["shounen".into()];

        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.genres = vec!["action".into(), "adventure".into()];

        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.genres, vec!["fantasy", "action", "adventure"]);
        assert_eq!(result.merged.tags, vec!["shounen"]);
    }

    #[test]
    fn external_ids_union_and_provider_conflict() {
        let mut survivor = media("m-1", "Fairy Tail");
        survivor
            .external_ids
            .push(ExternalId::new(ProviderId::new("anilist").unwrap(), "21", None).unwrap());
        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate
            .external_ids
            .push(ExternalId::new(ProviderId::new("mal").unwrap(), "6702", None).unwrap());

        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.external_ids.len(), 2);
        assert!(result.conflicts.is_empty());

        let mut duplicate = media("m-3", "Fairy Tail");
        duplicate
            .external_ids
            .push(ExternalId::new(ProviderId::new("anilist").unwrap(), "99", None).unwrap());
        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.external_ids.len(), 1, "provider stays unique");
        assert!(result.conflicts.iter().any(|c| c.field == "external_id"));
    }

    #[test]
    fn relations_are_unioned_and_self_relations_dropped() {
        let survivor = media("m-1", "Fairy Tail");
        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.relations.push(MediaRelation {
            to_id: MediaId::new("m-9").unwrap(),
            kind: MediaRelationKind::Sequel,
        });
        // A relation from the duplicate back to the survivor must not be moved.
        duplicate.relations.push(MediaRelation {
            to_id: survivor.id.clone(),
            kind: MediaRelationKind::SameUniverse,
        });
        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.relations.len(), 1);
        assert_eq!(result.merged.relations[0].to_id.as_str(), "m-9");
    }

    #[test]
    fn runtime_merges_optionally_preferring_survivor() {
        let mut survivor = media("m-1", "Fairy Tail");
        survivor.runtime.ep_count = Some(175);
        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.runtime.pages = Some(42);
        duplicate.runtime.ep_count = Some(999);

        let result = plan(&survivor, &duplicate);
        assert_eq!(result.merged.runtime.ep_count, Some(175));
        assert_eq!(result.merged.runtime.pages, Some(42));
    }

    #[test]
    fn nodes_are_re_parented_keeping_ids_and_parent_links() {
        let survivor = media("m-1", "Fairy Tail");
        let duplicate = media("m-2", "Fairy Tail");
        let nodes = vec![
            ContentNode {
                id: "n-1".into(),
                media_id: duplicate.id.clone(),
                parent_id: None,
                kind: crate::domain::enums::NodeKind::Chapter,
                position: 1,
                number: None,
                title: Some("ch1".into()),
                release_date: None,
                duration_min: None,
                page_count: None,
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
            ContentNode {
                id: "n-2".into(),
                media_id: duplicate.id.clone(),
                parent_id: Some("n-1".into()),
                kind: crate::domain::enums::NodeKind::Chapter,
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
        ];
        let result = plan_merge(&survivor, &duplicate, &nodes, &[], false, false).unwrap();
        assert_eq!(result.re_parented_nodes.len(), 2);
        assert!(result
            .re_parented_nodes
            .iter()
            .all(|n| n.media_id == survivor.id
                && n.parent_id == nodes.iter().find(|o| o.id == n.id).unwrap().parent_id));
    }

    #[test]
    fn user_data_moves_only_when_survivor_lacks_it() {
        let survivor = media("m-1", "Fairy Tail");
        let duplicate = media("m-2", "Fairy Tail");
        let collections = vec!["col-1".into()];

        let taken = plan_merge(&survivor, &duplicate, &[], &collections, false, false).unwrap();
        assert!(taken.move_review);
        assert!(taken.move_tracking);
        assert_eq!(taken.move_collection_memberships, vec!["col-1".to_string()]);

        let kept = plan_merge(&survivor, &duplicate, &[], &[], true, true).unwrap();
        assert!(!kept.move_review);
        assert!(!kept.move_tracking);
    }

    #[test]
    fn before_image_snapshots_both_records_for_undo() {
        let survivor = media("m-1", "Fairy Tail");
        let duplicate = media("m-2", "Fairy Tail");
        let result = plan(&survivor, &duplicate);
        assert_eq!(result.before.survivor, survivor);
        assert_eq!(result.before.duplicate, duplicate);
    }

    #[test]
    fn content_type_mismatch_is_reported_and_survivor_wins() {
        let survivor = media("m-1", "Fairy Tail");
        let mut duplicate = media("m-2", "Fairy Tail");
        duplicate.content_type = ContentType::Manga;
        let result = plan(&survivor, &duplicate);
        assert!(result.conflicts.iter().any(|c| c.field == "content_type"));
        assert_eq!(result.merged.content_type, ContentType::Novel);
    }
}
