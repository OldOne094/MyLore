//! iTunes Search → domain normalization (MISSION-109).

use crate::domain::enums::ContentType;
use crate::domain::provider::types::ProviderCandidate;

use super::response::SearchItem;
use super::PROVIDER_ID;

/// A search row → candidate. Rows without a title drop.
pub(crate) fn candidate(item: &SearchItem) -> Option<ProviderCandidate> {
    let title = item.collection_name.as_deref()?.trim();
    if title.is_empty() {
        return None;
    }
    let id = item.collection_id?;
    let cover = item
        .artwork_url_600
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(item
            .artwork_url_100
            .as_deref()
            .map(str::to_string)
            .as_deref())
        .map(str::to_string);

    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id.to_string(),
        title: title.to_string(),
        content_type: ContentType::Podcast, // overridden by caller for music
        release_year: item
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4).and_then(|y| y.parse().ok())),
        cover_url: cover,
        synopsis: None,
        external_ids: Vec::new(),
        url: item.collection_view_url.clone(),
    })
}
