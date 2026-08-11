//! Identity service (MISSION-026, DOMAIN_MODEL §2.4 / dedup).
//!
//! Decides whether an incoming record (from an import or provider) is the same
//! media as an already-stored candidate. Pure and side-effect free — the caller
//! fetches candidates; this module only scores and ranks them.
//!
//!   - **Exact** match: the same (provider, external id) is already on file —
//!     definitive (score 1.0).
//!   - **Title exact**: any title (main / original / alternative) folds equal
//!     (score 0.95, DOMAIN_MODEL: "same title" is a strong signal).
//!   - **Fuzzy**: best title similarity across all title pairs (0..1) using the
//!     MISSION-025 fold, ranked below the definitive tiers.

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::domain::normalize::{fold_title, title_matches};
use crate::domain::value_objects::{ExternalId, MediaId, ProviderId, Title};

/// Score for a fold-equal title match (below an exact external id).
pub const TITLE_EXACT_SCORE: f64 = 0.95;

/// A stored media the identity matcher compares an incoming record against.
#[derive(Debug, Clone)]
pub struct IdentityCandidate {
    pub media_id: MediaId,
    pub titles: Title,
    pub external_ids: Vec<ExternalId>,
}

/// How strong a match is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    /// The same (provider, external id) is already on file — definitive.
    Exact,
    /// A title folds equal to a stored title.
    TitleExact,
    /// Best-effort fuzzy match via title similarity.
    Fuzzy,
    /// No meaningful overlap.
    None,
}

/// A scored match between an incoming record and one candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentityMatch {
    pub media_id: MediaId,
    pub kind: IdentityKind,
    /// 1.0 (exact id) / 0.95 (exact title) / the best title similarity.
    pub score: f64,
    /// Best fuzzy title similarity across all title pairs (0..1), even for the
    /// definitive tiers (so callers can surface it).
    pub title_similarity: f64,
}

/// Find the provider for which `incoming` and the candidate share the same
/// external id value, if any (definitive identity).
pub fn exact_external_id(
    candidate: &IdentityCandidate,
    incoming: &[ExternalId],
) -> Option<ProviderId> {
    incoming.iter().find_map(|inc| {
        candidate
            .external_ids
            .iter()
            .find(|c| c.provider() == inc.provider() && c.value() == inc.value())
            .map(|_| inc.provider().clone())
    })
}

/// Whether any pair of titles (main/original/alternative) folds equal.
pub fn titles_exact(left: &Title, right: &Title) -> bool {
    left.all().any(|a| right.all().any(|b| title_matches(a, b)))
}

/// Similarity of two titles on their folded keys in `[0, 1]`.
///
/// Token Jaccard and containment (shorter fully inside the longer, e.g. "Fairy
/// Tail" vs "Fairy Tail 100 Years Quest") are the primary signals; character
/// bigram Jaccard only refines the score *within* pairs that already share a
/// token or containment — otherwise a stray shared bigram like the "on" in
/// "One Piece" / "Attack on Titan" would produce false positives. 1.0 only
/// when the fold keys are identical; 0.0 when there is no overlap at all.
pub fn title_similarity(left: &str, right: &str) -> f64 {
    let a = fold_title(left);
    let b = fold_title(right);
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let token = token_jaccard(&a, &b);
    let containment = containment(&a, &b);
    let base = token.max(containment);
    if base <= 0.0 {
        return 0.0;
    }
    base.max(jaccard_bigrams(&a, &b))
}

/// Best similarity over every pair of titles of two media.
pub fn best_title_similarity(left: &Title, right: &Title) -> f64 {
    left.all()
        .flat_map(|a| right.all().map(move |b| (a, b)))
        .map(|(a, b)| title_similarity(a, b))
        .fold(0.0, f64::max)
}

/// Score one candidate against the incoming record.
pub fn score_candidate(
    candidate: &IdentityCandidate,
    incoming_titles: &Title,
    incoming_external_ids: &[ExternalId],
) -> IdentityMatch {
    let similarity = best_title_similarity(&candidate.titles, incoming_titles);
    let kind = if exact_external_id(candidate, incoming_external_ids).is_some() {
        IdentityKind::Exact
    } else if titles_exact(&candidate.titles, incoming_titles) {
        IdentityKind::TitleExact
    } else if similarity > 0.0 {
        IdentityKind::Fuzzy
    } else {
        IdentityKind::None
    };
    let score = match kind {
        IdentityKind::Exact => 1.0,
        IdentityKind::TitleExact => TITLE_EXACT_SCORE,
        IdentityKind::Fuzzy => similarity,
        IdentityKind::None => 0.0,
    };
    IdentityMatch {
        media_id: candidate.media_id.clone(),
        kind,
        score,
        title_similarity: similarity,
    }
}

/// Score every candidate and rank them by score (descending), tie-broken by
/// `media_id` so the order is deterministic.
pub fn rank_candidates(
    incoming_titles: &Title,
    incoming_external_ids: &[ExternalId],
    candidates: &[IdentityCandidate],
) -> Vec<IdentityMatch> {
    let mut matches: Vec<IdentityMatch> = candidates
        .iter()
        .map(|c| score_candidate(c, incoming_titles, incoming_external_ids))
        .collect();
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.media_id.as_str().cmp(b.media_id.as_str()))
    });
    matches
}

/// The strongest match, or `None` when nothing overlaps (kind `None`).
pub fn best_match(
    incoming_titles: &Title,
    incoming_external_ids: &[ExternalId],
    candidates: &[IdentityCandidate],
) -> Option<IdentityMatch> {
    rank_candidates(incoming_titles, incoming_external_ids, candidates)
        .into_iter()
        .find(|m| m.kind != IdentityKind::None)
}

fn jaccard_bigrams(a: &str, b: &str) -> f64 {
    let x = bigrams(a);
    let y = bigrams(b);
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let inter = x.intersection(&y).count() as f64;
    let union = x.union(&y).count() as f64;
    inter / union
}

fn bigrams(s: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

fn token_jaccard(a: &str, b: &str) -> f64 {
    let x: HashSet<&str> = a.split(' ').filter(|t| !t.is_empty()).collect();
    let y: HashSet<&str> = b.split(' ').filter(|t| !t.is_empty()).collect();
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let inter = x.intersection(&y).count() as f64;
    let union = x.union(&y).count() as f64;
    inter / union
}

/// `len(shorter) / len(longer)` when the shorter folded key is fully contained
/// in the longer (prefix/suffix/subtitle variants); 0 otherwise.
fn containment(a: &str, b: &str) -> f64 {
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    if long.contains(short) {
        short.chars().count() as f64 / long.chars().count() as f64
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_id(id: &str) -> MediaId {
        MediaId::new(id).unwrap()
    }

    fn title(main: &str) -> Title {
        Title::new(main, None, vec![]).unwrap()
    }

    fn candidate(id: &str, main: &str, ext: &[(&str, &str)]) -> IdentityCandidate {
        let external_ids = ext
            .iter()
            .map(|(provider, value)| {
                ExternalId::new(ProviderId::new(*provider).unwrap(), *value, None).unwrap()
            })
            .collect();
        IdentityCandidate {
            media_id: media_id(id),
            titles: title(main),
            external_ids,
        }
    }

    fn ext(provider: &str, value: &str) -> ExternalId {
        ExternalId::new(ProviderId::new(provider).unwrap(), value, None).unwrap()
    }

    #[test]
    fn exact_external_id_is_definitive_and_ranked_first() {
        let cand_a = candidate("m-1", "One Piece", &[("anilist", "21")]);
        let cand_b = candidate("m-2", "One Piece", &[("mal", "21")]);
        let incoming = title("One Piece");
        let incoming_ext = [ext("anilist", "21")];

        assert_eq!(
            exact_external_id(&cand_a, &incoming_ext).unwrap().as_str(),
            "anilist"
        );
        assert!(exact_external_id(&cand_b, &incoming_ext).is_none());

        let ranked = rank_candidates(&incoming, &incoming_ext, &[cand_b, cand_a]);
        assert_eq!(ranked[0].media_id, media_id("m-1"));
        assert_eq!(ranked[0].kind, IdentityKind::Exact);
        assert_eq!(ranked[0].score, 1.0);
    }

    #[test]
    fn exact_requires_same_provider_and_value() {
        let cand = candidate("m-1", "Sword", &[("anilist", "42")]);
        let incoming = [ext("mal", "42")]; // same value, different provider
        assert!(exact_external_id(&cand, &incoming).is_none());
        let incoming = [ext("anilist", "43")]; // same provider, different value
        assert!(exact_external_id(&cand, &incoming).is_none());
    }

    #[test]
    fn title_exact_ranks_above_fuzzy() {
        let exact = candidate("m-1", "Attack on Titan", &[]);
        let fuzzy = candidate("m-2", "Attack on Titan: Junior High", &[]);
        let incoming = title("Attack on Titan");
        let ranked = rank_candidates(&incoming, &[], &[fuzzy, exact]);
        assert_eq!(ranked[0].media_id, media_id("m-1"));
        assert_eq!(ranked[0].kind, IdentityKind::TitleExact);
        assert_eq!(ranked[0].score, TITLE_EXACT_SCORE);
    }

    #[test]
    fn alternative_titles_participate_in_exact_matching() {
        let cand = IdentityCandidate {
            media_id: media_id("m-1"),
            titles: Title::new(
                "Shingeki no Kyojin",
                Some("進撃の巨人".to_string()),
                vec!["Attack on Titan".to_string()],
            )
            .unwrap(),
            external_ids: vec![],
        };
        let incoming = title("Attack on Titan");
        let match_ = score_candidate(&cand, &incoming, &[]);
        assert_eq!(match_.kind, IdentityKind::TitleExact);
        assert!(title_matches("進撃の巨人", "進撃の巨人"));
    }

    #[test]
    fn similarity_is_symmetric_and_sane() {
        assert_eq!(title_similarity("One Piece", "one piece"), 1.0);
        assert_eq!(title_similarity("One Piece", "Attack on Titan"), 0.0);
        let a = title_similarity("Fairy Tail", "Fairy Tail 100 Years Quest");
        let b = title_similarity("Fairy Tail 100 Years Quest", "Fairy Tail");
        assert_eq!(a, b);
        assert!(a >= 0.4, "subset title should score moderate similarity");
        let typo = title_similarity("Hunter X Hunter", "Hunter x Hunter");
        assert_eq!(typo, 1.0, "fold already handles case/width variants");
    }

    #[test]
    fn fuzzy_matches_rank_by_similarity() {
        let stronger = candidate("m-1", "Fairy Tail", &[]);
        let weaker = candidate("m-2", "Fairy Tale", &[]);
        let unrelated = candidate("m-3", "Berserk", &[]);
        let incoming = title("Fairy Tail: Final Season");
        let ranked = rank_candidates(&incoming, &[], &[unrelated, weaker, stronger]);

        assert_eq!(ranked[0].media_id, media_id("m-1"));
        assert_eq!(ranked[0].kind, IdentityKind::Fuzzy);
        assert!(ranked[0].score >= ranked[1].score);
        assert_eq!(ranked[2].media_id, media_id("m-3"));
        assert_eq!(ranked[2].kind, IdentityKind::None);
    }

    #[test]
    fn best_match_ignores_no_overlap_and_breaks_ties_deterministically() {
        let a = candidate("m-a", "One Piece", &[]);
        let b = candidate("m-b", "One Piece", &[]);
        let incoming = title("One Piece");
        let ranked = rank_candidates(&incoming, &[], &[b.clone(), a.clone()]);
        // Identical scores → deterministic media_id order.
        assert_eq!(ranked[0].media_id, media_id("m-a"));
        assert_eq!(ranked[1].media_id, media_id("m-b"));

        let best = best_match(&incoming, &[], &[a.clone(), b.clone()]).unwrap();
        assert_eq!(best.media_id, media_id("m-a"));

        let none = best_match(&title("Berserk"), &[], &[a.clone(), b.clone()]);
        assert!(none.is_none());
    }

    #[test]
    fn external_id_wins_over_strong_title() {
        // Same title, but only one carries the external id that the incoming
        // record references — the id match must win.
        let with_id = candidate("m-1", "One Piece", &[("anilist", "21")]);
        let same_title = candidate("m-2", "One Piece", &[]);
        let incoming = title("One Piece");
        let incoming_ext = [ext("anilist", "21")];
        let ranked = rank_candidates(&incoming, &incoming_ext, &[same_title, with_id]);
        assert_eq!(ranked[0].media_id, media_id("m-1"));
        assert_eq!(ranked[0].kind, IdentityKind::Exact);
    }

    #[test]
    fn cjk_titles_fold_and_match() {
        let cand = candidate("m-1", "魔法使いの夜", &[]);
        let incoming = title("魔法使いの夜");
        let match_ = score_candidate(&cand, &incoming, &[]);
        assert_eq!(match_.kind, IdentityKind::TitleExact);
    }
}
