//! Live network integration test — exercises the REAL AniList adapter against
//! the REAL GraphQL endpoint. Run manually: `cargo test --test live_network -- --nocapture`
//! These tests require internet access and are skipped in CI.
use mylore_lib::infrastructure::providers::anilist::{AniListClient, AniListProvider};
use mylore_lib::domain::enums::ContentType;
use mylore_lib::domain::provider::Provider;

#[tokio::test]
async fn live_anilist_search_berserk() {
    let client = AniListClient::new();
    let provider = AniListProvider::new(client);

    println!("Calling AniList search for 'Berserk'...");
    match provider.search("Berserk", Some(ContentType::Manga)).await {
        Ok(hits) => {
            println!("SUCCESS: {} hits", hits.len());
            for hit in hits.iter().take(3) {
                println!("  [{:?}] {} ({:?})", hit.content_type, hit.title, hit.release_year);
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