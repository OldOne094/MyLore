//! Network diagnostics — tests basic HTTPS connectivity from Rust reqwest.
//! Run: cargo test --test live_network -- --nocapture
use std::time::Duration;

#[tokio::test]
async fn diagnose_simple_get_google() {
    println!("Testing https://www.google.com...");
    let client = reqwest::Client::new();
    match client.get("https://www.google.com").send().await {
        Ok(resp) => println!("  → OK status={}", resp.status()),
        Err(e) => {
            println!("  → FAILED: {e}");
            if e.is_connect() { println!("  → connection error"); }
            if e.is_timeout() { println!("  → timeout"); }
            if e.is_redirect() { println!("  → redirect"); }
            let source = std::error::Error::source(&e);
            if let Some(s) = source { println!("  → source: {s}"); }
        }
    }
}

#[tokio::test]
async fn diagnose_simple_get_http() {
    println!("Testing http://httpbin.org/get (no TLS)...");
    let client = reqwest::Client::new();
    match client.get("http://httpbin.org/get").send().await {
        Ok(resp) => println!("  → OK status={}", resp.status()),
        Err(e) => println!("  → FAILED: {e}"),
    }
}

#[tokio::test]
async fn diagnose_anilist_graphql() {
    println!("Testing graphql.anilist.co POST...");
    let client = reqwest::Client::builder()
        .user_agent("MyLore/0.1.0-alpha.1")
        .timeout(Duration::from_secs(15))
        .build()
        .expect("client");
    let body = serde_json::json!({
        "query": "{ Page(page:1, perPage:1) { media(sort: POPULARITY_DESC) { id title { romaji } } } }"
    });
    match client.post("https://graphql.anilist.co").json(&body).send().await {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(text) => println!("  → status={status} body={}", &text[..text.len().min(200)]),
                Err(e) => println!("  → status={status} read error: {e}"),
            }
        }
        Err(e) => println!("  → FAILED: {e}"),
    }
}

#[tokio::test]
async fn diagnose_dns() {
    println!("Testing DNS resolution for graphql.anilist.co...");
    match tokio::net::lookup_host("graphql.anilist.co:443").await {
        Ok(addrs) => {
            for addr in addrs.take(3) {
                println!("  → resolved: {addr}");
            }
        }
        Err(e) => println!("  → DNS FAILED: {e}"),
    }
}

#[tokio::test]
async fn diagnose_tcp_connect() {
    println!("Testing TCP connect to graphql.anilist.co:443...");
    match tokio::net::TcpStream::connect("graphql.anilist.co:443").await {
        Ok(stream) => println!("  → TCP connected: {:?}", stream.peer_addr()),
        Err(e) => println!("  → TCP FAILED: {e}"),
    }
}
