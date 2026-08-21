//! NovelUpdates HTML response parsing (MISSION-065, API_PROVIDERS §14).
//!
//! NovelUpdates serves server-rendered HTML (no JSON metadata API), so the
//! selectors here mirror the maintained LNReader `novelupdates` plugin: search
//! rows on `/series-finder/`, the series page (`/series/{slug}/`) and the
//! chapter feed returned by the `admin-ajax.php` POST. `scraper` turns those
//! documents into plain data; `normalize` maps the data into domain types.

use scraper::{Html, Selector};

/// A search-finder row (`div.search_main_box_nu`).
#[derive(Debug, Clone)]
pub(crate) struct SearchRow {
    pub title: String,
    pub slug: String,
    pub cover: Option<String>,
}

/// A parsed series page.
#[derive(Debug, Clone)]
pub(crate) struct SeriesPage {
    pub title: String,
    pub cover: Option<String>,
    /// Author names from `#authtag`, split on `,`.
    pub authors: Vec<String>,
    /// Genre names from `#seriesgenre a`.
    pub genres: Vec<String>,
    /// Raw status text from `#editstatus`.
    pub status: String,
    /// Raw type text from `#showtype`.
    pub show_type: String,
    /// Cleaned synopsis from `#editdescription`.
    pub synopsis: Option<String>,
    /// The numeric post id from `input#mypostid` (chapters fetch key).
    pub post_id: String,
}

/// Collapse whitespace and trim (NU markup is noisy).
fn collapse(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip HTML tags (NU synopses carry `<br>`/`<p>` markup).
fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    collapse(&out)
}

/// `https://www.novelupdates.com/series/{slug}/` (or `/series/{slug}/`) → slug.
fn slug_from_href(href: &str) -> Option<String> {
    href.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// Parse the `/series-finder/` results into rows. Rows without a slug are
/// dropped; the title defaults to "Untitled" like the LNReader plugin.
pub(crate) fn parse_search_rows(html: &str) -> Vec<SearchRow> {
    let doc = Html::parse_document(html);
    let row_sel = Selector::parse("div.search_main_box_nu").expect("static selector");
    let link_sel = Selector::parse(".search_title > a").expect("static selector");
    let img_sel = Selector::parse("img").expect("static selector");

    let mut rows = Vec::new();
    for row in doc.select(&row_sel) {
        let Some(link) = row.select(&link_sel).next() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let Some(slug) = slug_from_href(href) else {
            continue;
        };
        let title = collapse(&link.text().collect::<String>());
        let title = if title.is_empty() { "Untitled" } else { &title };
        let cover = row
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(str::to_string);
        rows.push(SearchRow {
            title: title.to_string(),
            slug,
            cover,
        });
    }
    rows
}

/// Parse a series page. Returns `None` when the page is not a series page
/// (e.g. a not-found or an unrelated document).
pub(crate) fn parse_series_page(html: &str) -> Option<SeriesPage> {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse(".seriestitlenu").expect("static selector");
    let title_el = doc.select(&title_sel).next()?;
    let title = collapse(&title_el.text().collect::<String>());
    let title = if title.is_empty() { "Untitled" } else { &title };

    let cover = doc
        .select(&Selector::parse(".wpb_wrapper img").expect("static selector"))
        .next()
        .and_then(|img| img.value().attr("src"))
        .map(str::to_string);

    let authors = doc
        .select(&Selector::parse("#authtag").expect("static selector"))
        .flat_map(|el| {
            collapse(&el.text().collect::<String>())
                .split(',')
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let genres = doc
        .select(&Selector::parse("#seriesgenre a").expect("static selector"))
        .map(|el| collapse(&el.text().collect::<String>()))
        .filter(|s| !s.is_empty())
        .collect();

    let text_of = |selector: &str| {
        doc.select(&Selector::parse(selector).expect("static selector"))
            .next()
            .map(|el| collapse(&el.text().collect::<String>()))
    };

    let post_id = doc
        .select(&Selector::parse("input#mypostid").expect("static selector"))
        .next()
        .and_then(|input| input.value().attr("value"))
        .map(str::to_string)
        .unwrap_or_default();

    Some(SeriesPage {
        title: title.to_string(),
        cover,
        authors,
        genres,
        status: text_of("#editstatus").unwrap_or_default(),
        show_type: text_of("#showtype").unwrap_or_default(),
        synopsis: text_of("#editdescription")
            .map(|s| strip_html(&s))
            .filter(|s| !s.is_empty()),
        post_id,
    })
}

/// Parse the `admin-ajax.php` chapter feed into raw labels (e.g. `v1c1part1`,
/// `c3`, `ss1`). Returns them newest-first as served.
pub(crate) fn parse_chapter_labels(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let li_sel = Selector::parse("li.sp_li_chp").expect("static selector");
    doc.select(&li_sel)
        .map(|el| collapse(&el.text().collect::<String>()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether the response is a Cloudflare/anti-bot challenge page (NU serves a
/// captcha page with a recognizable `<title>` to non-browser clients).
pub(crate) fn is_captcha_page(html: &str) -> bool {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&Selector::parse("title").expect("static selector"))
        .next()
        .map(|el| collapse(&el.text().collect::<String>()))
        .unwrap_or_default()
        .to_lowercase();
    [
        "just a moment",
        "bot verification",
        "redirecting",
        "you are being redirected",
        "un instant",
    ]
    .iter()
    .any(|needle| title.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    #[test]
    fn search_rows_parse_into_rows() {
        let rows = parse_search_rows(&fixture("novelupdates", "search_series.html"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Dungeon Defender");
        assert_eq!(rows[0].slug, "dungeon-defender");
        assert!(rows[0]
            .cover
            .as_deref()
            .unwrap()
            .contains("cdn.novelupdates.com"));
        assert_eq!(rows[1].title, "The Second Coming of Gluttony");
        assert_eq!(rows[1].slug, "the-second-coming-of-gluttony");
    }

    #[test]
    fn series_page_parses_all_fields() {
        let page = parse_series_page(&fixture("novelupdates", "series_dungeon_defender.html"))
            .expect("series page present");
        assert_eq!(page.title, "Dungeon Defender");
        assert!(page
            .cover
            .as_deref()
            .unwrap()
            .contains("cdn.novelupdates.com"));
        assert_eq!(page.authors, vec!["Golam"]);
        assert!(page.genres.iter().any(|g| g == "Action"));
        assert!(page.genres.iter().any(|g| g == "Fantasy"));
        assert_eq!(page.status, "Ongoing");
        assert_eq!(page.show_type, "Web Novel");
        assert!(page.synopsis.as_deref().unwrap().contains("Lester"));
        assert_eq!(page.post_id, "42817");
    }

    #[test]
    fn series_page_missing_is_none() {
        assert!(parse_series_page("<html><body>not a series</body></html>").is_none());
    }

    #[test]
    fn chapter_labels_newest_first() {
        let labels =
            parse_chapter_labels(&fixture("novelupdates", "chapters_dungeon_defender.html"));
        assert_eq!(labels.len(), 4);
        assert_eq!(labels[0], "v1c4part3", "newest first as served");
        assert_eq!(labels[1], "v1c4part2");
        assert_eq!(labels[2], "v1c4part1");
        assert_eq!(labels[3], "ss1", "side story");
    }

    #[test]
    fn captcha_pages_detected() {
        assert!(is_captcha_page(
            "<html><head><title>Just a moment...</title></head></html>"
        ));
        assert!(is_captcha_page(&fixture("novelupdates", "captcha.html")));
        assert!(!is_captcha_page(&fixture(
            "novelupdates",
            "search_series.html"
        )));
    }
}
