//! Temporary MISSION-129 diagnostic — raw wreq vs reqwest against NovelUpdates.
use std::time::Duration;

#[tokio::test]
#[ignore = "live network"]
async fn nu_raw_wreq_probe() {
    let client = wreq::Client::builder()
        .emulation(wreq_util::emulate::Profile::Chrome149)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("wreq builds");

    let response = client
        .get("https://www.novelupdates.com/series-finder/?sh=dungeon&sf=1")
        .send()
        .await
        .expect("send");

    let status = response.status();
    let server = response
        .headers()
        .get("server")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("?")
        .to_string();
    let cf_ray = response
        .headers()
        .get("cf-ray")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("no-cf-ray")
        .to_string();
    let body = response.text().await.unwrap_or_default();
    let head: String = body.chars().take(300).collect();
    println!(
        "STATUS: {status}\nSERVER: {server}\nCF-RAY: {cf_ray}\nLEN: {}\nHEAD: {head}",
        body.len()
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn nu_profile_matrix() {
    let profiles: Vec<(&str, wreq_util::emulate::Profile)> = vec![
        ("chrome149", wreq_util::emulate::Profile::Chrome149),
        ("firefox14x", wreq_util::emulate::Profile::Firefox144),
        ("safari18", wreq_util::emulate::Profile::Safari18),
        ("edge13x", wreq_util::emulate::Profile::Edge135),
    ];
    for (name, profile) in profiles {
        let client = match wreq::Client::builder()
            .emulation(profile)
            .timeout(Duration::from_secs(20))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                println!("{name}: profile build failed");
                continue;
            }
        };
        let outcome = match client.get("https://www.novelupdates.com/").send().await {
            Ok(r) => format!(
                "{} len={}",
                r.status(),
                r.text().await.unwrap_or_default().len()
            ),
            Err(e) => format!("ERR {e}"),
        };
        println!("PROFILE {name}: {outcome}");
    }
}

#[tokio::test]
#[ignore = "live network"]
async fn nu_raw_reqwest_probe() {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest builds");

    let response = client
        .get("https://www.novelupdates.com/series-finder/?sh=dungeon&sf=1")
        .send()
        .await
        .expect("send");

    let status = response.status();
    let cf_mitigated = response
        .headers()
        .get("cf-mitigated")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();
    let body = response.text().await.unwrap_or_default();
    let head: String = body.chars().take(300).collect();
    println!(
        "REQWEST STATUS: {status}\nCF-MITIGATED: {cf_mitigated}\nLEN: {}\nHEAD: {head}",
        body.len()
    );
}
