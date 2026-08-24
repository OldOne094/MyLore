//! Live network integration test — exercises the REAL AniList adapter against
//! the REAL GraphQL endpoint. Run manually: `cargo test --test live_network -- --nocapture`
//! These tests require internet access and are skipped in CI.
use mylore_lib::domain::enums::ContentType;
use mylore_lib::domain::provider::Provider;
use mylore_lib::infrastructure::providers::anilist::{AniListClient, AniListProvider};

#[tokio::test]
#[ignore = "requires the app webview bridge"]
async fn live_novelupdates_search() {
    use mylore_lib::infrastructure::providers::novelupdates::{
        NovelUpdatesClient, NovelUpdatesProvider,
    };

    let client = NovelUpdatesClient::new();
    // Raw transport probe first: what does Cloudflare actually say?
    match client
        .get("/series-finder/", &[("sh", "dungeon"), ("sf", "1")])
        .await
    {
        Ok(html) => println!(
            "RAW GET OK: {} bytes; head: {}",
            html.len(),
            html.chars().take(160).collect::<String>()
        ),
        Err(e) => println!("RAW GET ERR: {e}"),
    }

    let provider = NovelUpdatesProvider::new(client);

    println!("Calling NovelUpdates search for 'dungeon defender'...");
    match provider.search("dungeon defender", None).await {
        Ok(hits) => {
            println!("SUCCESS: {} hits", hits.len());
            for hit in hits.iter().take(3) {
                println!("  [{:?}] {}", hit.content_type, hit.title);
            }
            assert!(!hits.is_empty(), "expected non-empty results");
        }
        Err(e) => {
            println!("FAILED: {e}");
            panic!("NovelUpdates search should succeed behind Cloudflare: {e}");
        }
    }
}

#[tokio::test]
async fn live_anilist_search_berserk() {
    let client = AniListClient::new();
    let provider = AniListProvider::new(client);

    println!("Calling AniList search for 'Berserk'...");
    match provider.search("Berserk", Some(ContentType::Manga)).await {
        Ok(hits) => {
            println!("SUCCESS: {} hits", hits.len());
            for hit in hits.iter().take(3) {
                println!(
                    "  [{:?}] {} ({:?})",
                    hit.content_type, hit.title, hit.release_year
                );
            }
            assert!(!hits.is_empty(), "expected non-empty results");
        }
        Err(e) => {
            println!("FAILED: {e}");
            panic!("AniList search should succeed: {e}");
        }
    }
}

#[tokio::test]
async fn live_anilist_search_no_type() {
    let client = AniListClient::new();
    let provider = AniListProvider::new(client);

    println!("Calling AniList search for 'Frieren' without type filter...");
    match provider.search("Frieren", None).await {
        Ok(hits) => {
            println!("SUCCESS: {} hits", hits.len());
            assert!(!hits.is_empty(), "expected non-empty results");
        }
        Err(e) => {
            println!("FAILED: {e}");
            panic!("AniList search should succeed: {e}");
        }
    }
}

#[tokio::test]
async fn live_openlibrary_search() {
    use mylore_lib::infrastructure::providers::openlibrary::{
        OpenLibraryClient, OpenLibraryProvider,
    };

    let client = OpenLibraryClient::new();
    let provider = OpenLibraryProvider::new(client);

    println!("Calling OpenLibrary search for 'dune'...");
    match provider.search("dune", Some(ContentType::Book)).await {
        Ok(hits) => {
            println!("SUCCESS: {} hits", hits.len());
            assert!(!hits.is_empty());
        }
        Err(e) => {
            println!("FAILED: {e}");
            panic!("OpenLibrary search should succeed: {e}");
        }
    }
}

#[tokio::test]
#[ignore = "live network"]
async fn live_wtrlab_search_and_details() {
    use mylore_lib::infrastructure::providers::wtrlab::{client::WtrLabClient, WtrLabProvider};

    let provider = WtrLabProvider::new(WtrLabClient::new());
    println!("Searching WTR-LAB for 'emperor'...");
    let hits = provider
        .search(
            "emperor",
            Some(mylore_lib::domain::enums::ContentType::WebNovel),
        )
        .await
        .expect("live search");
    println!("SEARCH OK: {} hits", hits.len());
    assert!(!hits.is_empty(), "expected results");

    let first = &hits[0];
    println!("First hit: {} ({})", first.title, first.provider_id);
    let media = provider
        .get_details(&first.provider_id)
        .await
        .expect("live details");
    println!(
        "DETAILS OK: {} | status={:?} | chapters={:?} | genres={} | tags={}",
        media.title_main,
        media.pub_status,
        media.ch_count,
        media.genres.join("/"),
        media.tags.join("/")
    );
}
