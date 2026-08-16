//! Bangumi → domain normalization (MISSION-066).
//!
//! Pure functions mapping parsed v0 JSON into the unified domain types. Bangumi
//! catalogs ACGN subjects (book/anime/music/game/real), so content type comes
//! from `type` (1 book · 2 anime · 3 music · 4 game · 6 real) plus the book
//! `platform` sub-type (`漫画` manga · `小说`/`网络小说` novel/web-novel).
//! Chinese labels are kept on `title_main` when present; the `infobox` wiki
//! keys (`作者`, `插图`, `导演`, `动画制作`) map to people credits. Bangumi has
//! no genre taxonomy — `tags` are user votes (mixed), so `genres` are a
//! best-effort top-tags subset with structural noise (years/formats/countries/
//! adaptation markers) filtered out.

use crate::domain::enums::{ContentType, MediaRelationKind, MediaStatus, NodeKind, PersonRole};
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson, ProviderRelation,
};

use super::response::{
    Episode, InfoboxValue, PagedEpisode, RelatedSubject, SlimSubject, Subject, Tag,
};
use super::{subject_url, PROVIDER_ID};

/// Cap the number of genres / tags surfaced per title.
const MAX_GENRES: usize = 8;
const MAX_TAGS: usize = 10;

/// Bangumi subject `type` + book `platform` → domain content type.
/// Book platforms seen in the wild: `漫画`, `小说`, `网络小说` (web novel).
pub(crate) fn content_type(type_id: i64, platform: Option<&str>) -> ContentType {
    let p = platform.unwrap_or("");
    match type_id {
        1 => {
            if p.contains("漫画") {
                ContentType::Manga
            } else if p.contains("小说") {
                if p.contains("网络") {
                    ContentType::WebNovel
                } else {
                    ContentType::Novel
                }
            } else {
                ContentType::Book
            }
        }
        2 => ContentType::Anime,
        _ => ContentType::Other,
    }
}

/// A known release date → completed (Bangumi exposes no airing-status field).
pub(crate) fn pub_status(date: Option<&str>) -> MediaStatus {
    match date {
        Some(d) if !d.trim().is_empty() => MediaStatus::Completed,
        _ => MediaStatus::Unknown,
    }
}

/// The first 4-digit run in `YYYY-MM-DD` (Bangumi always sends this format).
pub(crate) fn release_year(date: Option<&str>) -> Option<i32> {
    let mut buf = String::new();
    for c in date?.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            if buf.len() == 4 {
                let year: i32 = buf.parse().ok()?;
                return if year == 0 { None } else { Some(year) };
            }
        } else {
            buf.clear();
        }
    }
    None
}

/// Collapse whitespace/newlines on synopsis/description text.
fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The display title: `name_cn` when present (Bangumi is a CN community),
/// else the original `name`.
pub(crate) fn title(name_cn: &str, name: &str) -> Option<String> {
    let main = if name_cn.trim().is_empty() {
        name
    } else {
        name_cn
    };
    if main.trim().is_empty() {
        None
    } else {
        Some(main.trim().to_string())
    }
}

/// A search row → candidate. Rows outside the book/anime domain (music/game/
/// real slipping past the request filter) drop.
pub(crate) fn candidate(row: &SlimSubject) -> Option<ProviderCandidate> {
    if !matches!(row.r#type, 1 | 2) {
        return None;
    }
    let t = title(&row.name_cn, &row.name)?;
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: row.id.to_string(),
        title: t,
        content_type: content_type(row.r#type, row.platform.as_deref()),
        release_year: release_year(row.date.as_deref()),
        cover_url: row.images.as_ref().and_then(|i| i.large.clone()),
        synopsis: row
            .short_summary
            .as_deref()
            .map(clean_text)
            .filter(|s| !s.is_empty()),
        external_ids: Vec::new(),
        url: Some(subject_url(row.id)),
    })
}

/// A full subject → `ProviderMedia`. `provider_id` is the caller's id (the
/// subject carries it too; the param matches the other adapters).
pub(crate) fn media(subject: &Subject, provider_id: &str) -> Option<ProviderMedia> {
    let t = title(&subject.name_cn, &subject.name)?;
    let ct = content_type(subject.r#type, subject.platform.as_deref());
    let total_eps = subject.total_episodes.unwrap_or(0).max(0) as u32;
    let is_anime = matches!(ct, ContentType::Anime);
    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: provider_id.to_string(),
        title_main: t,
        title_original: if subject.name_cn.trim().is_empty() {
            None
        } else {
            Some(subject.name.clone())
        },
        alt_titles: Vec::new(),
        content_type: ct,
        format: subject.platform.clone(),
        pub_status: pub_status(subject.date.as_deref()),
        synopsis: subject
            .summary
            .as_deref()
            .map(clean_text)
            .filter(|s| !s.is_empty()),
        start_date: subject.date.clone(),
        end_date: None,
        release_year: release_year(subject.date.as_deref()),
        language: None,
        country: None,
        content_rating: None,
        pages: None,
        duration_min: None,
        ep_count: if is_anime && total_eps > 0 {
            Some(total_eps)
        } else {
            None
        },
        ch_count: if !is_anime && total_eps > 0 {
            Some(total_eps)
        } else {
            None
        },
        cover_url: subject.images.as_ref().and_then(|i| i.large.clone()),
        banner_url: None,
        url: Some(subject_url(subject.id)),
        people: people(subject),
        genres: genres(subject),
        tags: tags(subject),
        external_ids: Vec::new(),
    })
}

/// Wiki `infobox` keys → domain person credits. Bangumi book fields use
/// `作者`/`插图`; anime fields use `导演`/`动画制作`.
pub(crate) fn people(subject: &Subject) -> Vec<ProviderPerson> {
    let mut out: Vec<ProviderPerson> = Vec::new();
    for item in &subject.infobox {
        let Some(role) = (match item.key.as_str() {
            "作者" => Some(PersonRole::Author),
            "插图" => Some(PersonRole::Artist),
            "导演" => Some(PersonRole::Director),
            "动画制作" | "制作" => Some(PersonRole::Studio),
            _ => None,
        }) else {
            continue;
        };
        let Some(value) = item.value.as_ref().and_then(InfoboxValue::text) else {
            continue;
        };
        for name in value.split(['、', '，', ',', '/']) {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let person = ProviderPerson {
                role,
                name: name.to_string(),
            };
            if !out.contains(&person) {
                out.push(person);
            }
        }
    }
    out
}

/// Tags sorted by vote count, names only.
fn top_tags(subject: &Subject) -> Vec<String> {
    let mut items: Vec<&Tag> = subject.tags.iter().collect();
    items.sort_by_key(|tag| std::cmp::Reverse(tag.count));
    items
        .into_iter()
        .filter_map(|tag| {
            let name = tag.name.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

/// Best-effort genres: top user tags minus structural noise (years, formats,
/// countries, adaptation markers, `漫画`/`小说` series labels). Bangumi has no
/// genre taxonomy — documented in API_PROVIDERS §13.
pub(crate) fn genres(subject: &Subject) -> Vec<String> {
    top_tags(subject)
        .into_iter()
        .filter(|tag| !is_noise_tag(tag))
        .take(MAX_GENRES)
        .collect()
}

/// All top user tags (unfiltered) for the `tags` field.
pub(crate) fn tags(subject: &Subject) -> Vec<String> {
    top_tags(subject).into_iter().take(MAX_TAGS).collect()
}

fn is_noise_tag(tag: &str) -> bool {
    let s = tag.trim();
    if s.is_empty() || is_year_tag(s) {
        return true;
    }
    matches!(
        s,
        "TV" | "OVA"
            | "OAD"
            | "WEB"
            | "剧场版"
            | "电影"
            | "动画电影"
            | "日本"
            | "中国"
            | "美国"
            | "原创"
            | "漫画改"
            | "漫改"
            | "小说改"
            | "游戏改"
            | "续作"
            | "半年番"
            | "短篇"
            | "漫画"
            | "小说"
            | "小说系列"
    )
}

/// `2017`, `2017年`, `2017年10月` → structural year tags.
fn is_year_tag(tag: &str) -> bool {
    let digits: String = tag
        .chars()
        .filter(|c| !matches!(c, '年' | '月' | ' ' | '.'))
        .collect();
    digits.len() >= 4 && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The chapter/episode tree from the episodes feed. Only main-story (type 0)
/// and SP (type 1) rows are kept — OP/ED/PV/MAD rows are dropped. `kind`
/// follows the subject's content type (Episode for anime, Chapter for books).
pub(crate) fn nodes(
    episodes: &PagedEpisode,
    provider_id: &str,
    ct: ContentType,
) -> Vec<ProviderNode> {
    let kind = if matches!(ct, ContentType::Anime) {
        NodeKind::Episode
    } else {
        NodeKind::Chapter
    };
    let mut rows: Vec<&Episode> = episodes
        .data
        .iter()
        .filter(|e| matches!(e.r#type, 0 | 1))
        .collect();
    rows.sort_by(|a, b| {
        a.position()
            .partial_cmp(&b.position())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.iter()
        .map(|ep| {
            let position = ep.position();
            let number = format_f64(ep.ep.or(ep.sort));
            let label = node_label(kind);
            let t = title(&ep.name_cn, &ep.name).unwrap_or_else(|| format!("{label} {position}"));
            ProviderNode {
                id: format!("{provider_id}-ep-{}", ep.id),
                kind,
                position,
                number,
                title: Some(t),
                release_date: ep.airdate.clone(),
                duration_min: ep.duration_seconds.map(|s| s / 60).filter(|m| *m > 0),
                page_count: None,
                synopsis: ep.desc.as_deref().map(clean_text).filter(|s| !s.is_empty()),
                is_special: ep.r#type == 1,
                children: Vec::new(),
            }
        })
        .collect()
}

impl Episode {
    /// The node position: `ep` (within-subject number) when present, else the
    /// all-seasons `sort`. Specials without an `ep` fall back to `sort`.
    fn position(&self) -> i64 {
        self.ep.or(self.sort).map(|n| n.round() as i64).unwrap_or(0)
    }
}

/// `1.0` → `"1"`, `6.5` → `"6.5"`.
fn format_f64(n: Option<f64>) -> Option<String> {
    let n = n?;
    if n.fract() == 0.0 {
        Some((n as i64).to_string())
    } else {
        Some(format!("{n}"))
    }
}

/// Human node-kind label for the no-title fallback ("Episode 3" / "Chapter 3").
fn node_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Episode => "Episode",
        _ => "Chapter",
    }
}

/// Relation edges → domain kinds. `原作`/`动画`/`漫画`/`小说`/`书籍` point at
/// adaptation sources; `前传`/`续集` are the temporals; everything else (music,
/// `其他`, …) is `Other`.
pub(crate) fn relations(items: &[RelatedSubject]) -> Vec<ProviderRelation> {
    items
        .iter()
        .map(|r| ProviderRelation {
            to_provider: PROVIDER_ID.to_string(),
            to_id: r.id.to_string(),
            relation: relation_kind(r.relation.as_deref()),
            title: title(&r.name_cn, &r.name),
        })
        .collect()
}

fn relation_kind(relation: Option<&str>) -> MediaRelationKind {
    let r = relation.unwrap_or("").to_lowercase();
    if r.contains("前传") || r.contains("前作") {
        MediaRelationKind::Prequel
    } else if r.contains("续集") || r.contains("续作") {
        MediaRelationKind::Sequel
    } else if r.contains("原作")
        || r.contains("动画")
        || r.contains("漫画")
        || r.contains("小说")
        || r.contains("书籍")
    {
        MediaRelationKind::Adaptation
    } else {
        MediaRelationKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::providers::test_support::bangumi_fixture;

    fn subject_fixture() -> Subject {
        let data: crate::infrastructure::providers::bangumi::response::Subject =
            serde_json::from_str(&bangumi_fixture("subject_detail.json")).unwrap();
        data
    }

    fn search_fixture() -> PagedEpisode {
        serde_json::from_str(&bangumi_fixture("episodes.json")).unwrap()
    }

    #[test]
    fn content_type_maps_type_and_platform() {
        assert_eq!(content_type(1, Some("漫画")), ContentType::Manga);
        assert_eq!(content_type(1, Some("小说")), ContentType::Novel);
        assert_eq!(content_type(1, Some("网络小说")), ContentType::WebNovel);
        assert_eq!(content_type(1, Some("画集")), ContentType::Book);
        assert_eq!(content_type(1, None), ContentType::Book);
        assert_eq!(content_type(2, Some("TV")), ContentType::Anime);
        assert_eq!(content_type(3, None), ContentType::Other);
        assert_eq!(content_type(4, None), ContentType::Other);
    }

    #[test]
    fn title_prefers_cn_name() {
        assert_eq!(
            title("三月的狮子", "3月のライオン").as_deref(),
            Some("三月的狮子")
        );
        assert_eq!(
            title("", "夢探偵フロイト").as_deref(),
            Some("夢探偵フロイト")
        );
        assert_eq!(title("", ""), None);
    }

    #[test]
    fn release_year_parses_date() {
        assert_eq!(release_year(Some("2017-10-14")), Some(2017));
        assert_eq!(release_year(Some("2008-02-22")), Some(2008));
        assert_eq!(release_year(None), None);
    }

    #[test]
    fn pub_status_uses_date_presence() {
        assert_eq!(pub_status(Some("2017-10-14")), MediaStatus::Completed);
        assert_eq!(pub_status(None), MediaStatus::Unknown);
    }

    #[test]
    fn candidate_maps_search_row() {
        let data: super::super::response::PagedSubject =
            serde_json::from_str(&bangumi_fixture("search_subjects.json")).unwrap();
        let c = candidate(&data.data[0]).unwrap();
        assert_eq!(c.provider, "bangumi");
        assert_eq!(c.provider_id, "1902");
        assert_eq!(c.title, "三月的狮子");
        assert_eq!(c.content_type, ContentType::Manga);
        assert_eq!(c.release_year, Some(2008));
        assert_eq!(c.url.as_deref(), Some("https://bgm.tv/subject/1902"));
        assert!(c.cover_url.as_deref().unwrap().contains("lain.bgm.tv"));
        assert!(c.synopsis.as_deref().unwrap().contains("桐山零"));
    }

    #[test]
    fn candidate_drops_non_book_anime_domains() {
        let mut row = SlimSubject {
            id: 1,
            r#type: 3,
            name: "Album".into(),
            name_cn: String::new(),
            short_summary: None,
            date: None,
            platform: None,
            images: None,
        };
        assert!(candidate(&row).is_none());
        row.r#type = 1;
        assert!(candidate(&row).is_some());
    }

    #[test]
    fn media_maps_anime_detail() {
        let m = media(&subject_fixture(), "211567").unwrap();
        assert_eq!(m.provider_id, "211567");
        assert_eq!(m.title_main, "3月的狮子 第二季");
        assert_eq!(
            m.title_original.as_deref(),
            Some("3月のライオン 第2シリーズ")
        );
        assert_eq!(m.content_type, ContentType::Anime);
        assert_eq!(m.format.as_deref(), Some("TV"));
        assert_eq!(m.pub_status, MediaStatus::Completed);
        assert_eq!(m.release_year, Some(2017));
        assert_eq!(m.ep_count, Some(22));
        assert_eq!(m.ch_count, None);
        assert!(!m.people.is_empty());
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Director && p.name == "新房昭之"));
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Studio && p.name == "SHAFT"));
        assert!(m.genres.iter().any(|g| g == "治愈"));
        assert!(!m.genres.iter().any(|g| g == "TV"), "format tag filtered");
        assert!(
            !m.genres.iter().any(|g| g == "日本"),
            "country tag filtered"
        );
        assert!(
            !m.genres.iter().any(|g| g == "2017年10月"),
            "year tag filtered"
        );
    }

    #[test]
    fn media_derives_novel_from_book_platform() {
        let mut s = subject_fixture();
        s.r#type = 1;
        s.platform = Some("小说".to_string());
        s.name = "夢探偵フロイト".to_string();
        s.name_cn = String::new();
        s.infobox = vec![
            crate::infrastructure::providers::bangumi::response::InfoboxItem {
                key: "作者".into(),
                value: Some(
                    crate::infrastructure::providers::bangumi::response::InfoboxValue::Text(
                        "内藤了".into(),
                    ),
                ),
            },
            crate::infrastructure::providers::bangumi::response::InfoboxItem {
                key: "插图".into(),
                value: Some(
                    crate::infrastructure::providers::bangumi::response::InfoboxValue::List(vec![
                        crate::infrastructure::providers::bangumi::response::InfoboxEntry {
                            v: "syo5".into(),
                        },
                    ]),
                ),
            },
        ];
        let m = media(&s, "308479").unwrap();
        assert_eq!(m.content_type, ContentType::Novel);
        assert_eq!(m.title_main, "夢探偵フロイト");
        assert_eq!(m.title_original, None);
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Author && p.name == "内藤了"));
        assert!(m
            .people
            .iter()
            .any(|p| p.role == PersonRole::Artist && p.name == "syo5"));
    }

    #[test]
    fn nodes_build_chapter_tree_for_books() {
        let tree = nodes(&search_fixture(), "211567", ContentType::Novel);
        assert_eq!(tree.len(), 3, "2 main + 1 special; OP/ED dropped");
        assert_eq!(tree[0].kind, NodeKind::Chapter);
        assert_eq!(tree[0].position, 1);
        assert_eq!(tree[0].number.as_deref(), Some("1"));
        assert!(!tree[0].is_special);
        assert_eq!(tree[1].position, 2);
        assert!(tree[2].is_special);
    }

    #[test]
    fn relations_map_to_domain_kinds() {
        let data: Vec<RelatedSubject> =
            serde_json::from_str(&bangumi_fixture("relations.json")).unwrap();
        let rels = relations(&data);
        assert_eq!(rels.len(), 3);
        assert_eq!(
            rels[0].relation,
            MediaRelationKind::Adaptation,
            "原作 manga"
        );
        assert_eq!(
            rels[1].relation,
            MediaRelationKind::Prequel,
            "前传 first season"
        );
        assert_eq!(rels[2].relation, MediaRelationKind::Other, "音乐");
    }
}
