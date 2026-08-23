//! NovelUpdates transport (MISSION-065, MISSION-129, API_PROVIDERS §14).
//!
//! NU is server-rendered HTML: one GET for search/details, one form POST to
//! `wp-admin/admin-ajax.php` for the chapter feed. Policy (rate limit,
//! timeout, retry/backoff, cancel) lives in the coordinator above this; the
//! client only maps transport/HTTP outcomes to `ProviderError`. Responses are
//! returned as raw text (HTML), which `response` parses. A Cloudflare/anti-bot
//! challenge is detected by `<title>` and reported as a non-retryable
//! `InvalidResponse`.
//!
//! **Transport (MISSION-129):** NU's Cloudflare layer fingerprints the TLS
//! handshake and rejects plain reqwest before an HTTP response exists. By
//! default (`nu-impersonate` feature) requests go through `wreq`, which
//! replays a real Chrome TLS + HTTP/2 fingerprint; builds without the feature
//! fall back to the legacy reqwest client (browser headers only).

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// The Chrome major version the impersonation profile emulates. Kept next to
/// the transport so a profile bump is one line.
#[cfg(feature = "nu-impersonate")]
use wreq_util::emulate::Profile;
#[cfg(feature = "nu-impersonate")]
const CHROME_EMULATION: Profile = Profile::Chrome149;

/// A polite, identifiable User-Agent for the fallback (non-impersonated)
/// transport and tests.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; NovelUpdates series metadata)"
);

/// Browser-ish default headers for the fallback transport (the impersonation
/// profile supplies its own complete header set).
#[cfg(not(feature = "nu-impersonate"))]
const BROWSER_HEADERS: &[(&str, &str)] = &[
    (
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    ),
    ("accept-language", "en-US,en;q=0.9"),
    ("referer", "https://www.novelupdates.com/"),
];

/// Which HTTP stack actually talks to NU. Exactly one exists per build so
/// there is never dead transport code compiled in.
#[cfg(feature = "nu-impersonate")]
mod transport {
    use super::CHROME_EMULATION;

    /// Full Chrome TLS + HTTP/2 fingerprint via BoringSSL (default build).
    #[derive(Clone)]
    pub struct Http(pub wreq::Client);

    impl Http {
        pub fn new() -> Self {
            let builder = wreq::Client::builder().emulation(CHROME_EMULATION);
            Self(builder.build().expect("wreq client builds"))
        }

        /// GET with an optional clearance cookie (MISSION-129 bridge).
        pub async fn get(
            &self,
            url: &str,
            params: &[(&str, &str)],
            creds: Option<(&str, &str)>,
        ) -> Result<(u16, String), crate::domain::provider::error::ProviderError> {
            let mut request = self.0.get(url).query(params);
            if let Some((ua, cookie)) = creds {
                request = request.header("user-agent", ua).header("cookie", cookie);
            }
            let response = request
                .send()
                .await
                .map_err(|e| super::transport_error(e.to_string()))?;
            super::read_body(response).await
        }

        /// POST form-urlencoded with an optional clearance cookie.
        pub async fn post_form(
            &self,
            url: &str,
            form: &[(&str, &str)],
            creds: Option<(&str, &str)>,
        ) -> Result<(u16, String), crate::domain::provider::error::ProviderError> {
            let mut request = self.0.post(url).form(form);
            if let Some((ua, cookie)) = creds {
                request = request.header("user-agent", ua).header("cookie", cookie);
            }
            let response = request
                .send()
                .await
                .map_err(|e| super::transport_error(e.to_string()))?;
            super::read_body(response).await
        }
    }
}

#[cfg(not(feature = "nu-impersonate"))]
mod transport {
    use super::{BROWSER_HEADERS, CHROME_UA};
    use crate::domain::provider::error::ProviderError;

    /// Legacy fallback: reqwest with a Chrome UA + browser headers only.
    /// Cloudflare may still fingerprint-reject it; kept so non-BoringSSL
    /// builds compile and behave exactly as before MISSION-129.
    #[derive(Clone)]
    pub struct Http(pub reqwest::Client);

    impl Http {
        pub fn new() -> Self {
            use reqwest::header::{HeaderMap, HeaderValue};
            let mut headers = HeaderMap::new();
            for (name, value) in BROWSER_HEADERS {
                if let Ok(v) = HeaderValue::from_str(value) {
                    headers.insert(*name, v);
                }
            }
            let http = reqwest::Client::builder()
                .user_agent(CHROME_UA)
                .default_headers(headers)
                .build()
                .expect("reqwest client builds");
            Self(http)
        }

        pub async fn get(
            &self,
            url: &str,
            params: &[(&str, &str)],
            creds: Option<(&str, &str)>,
        ) -> Result<(u16, String), ProviderError> {
            let mut request = self.0.get(url).query(params);
            if let Some((ua, cookie)) = creds {
                request = request.header("user-agent", ua).header("cookie", cookie);
            }
            let response = request
                .send()
                .await
                .map_err(|e| super::transport_error(e.to_string()))?;
            super::read_body(response).await
        }

        pub async fn post_form(
            &self,
            url: &str,
            form: &[(&str, &str)],
            creds: Option<(&str, &str)>,
        ) -> Result<(u16, String), ProviderError> {
            let mut request = self.0.post(url).form(form);
            if let Some((ua, cookie)) = creds {
                request = request.header("user-agent", ua).header("cookie", cookie);
            }
            let response = request
                .send()
                .await
                .map_err(|e| super::transport_error(e.to_string()))?;
            super::read_body(response).await
        }
    }
}

use transport::Http;

/// The Chrome UA string for the fallback transport (the impersonated transport
/// derives its UA from the emulation profile).
#[cfg(not(feature = "nu-impersonate"))]
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// A NovelUpdates client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct NovelUpdatesClient {
    http: Http,
    endpoint: String,
}

impl Default for NovelUpdatesClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NovelUpdatesClient {
    pub fn new() -> Self {
        Self {
            http: Http::new(),
            endpoint: super::ENDPOINT.to_string(),
        }
    }

    /// Clearance (cookie + harvesting UA) from the hidden webview. The
    /// cf_clearance cookie is bound to the harvesting browser's UA, so both
    /// ride every request.
    fn clearance(&self) -> Option<(String, String)> {
        crate::infrastructure::nu_bridge::global()?
            .snapshot()
            .map(|c| (c.user_agent, c.cookie))
    }

    /// Test hook: point whichever transport is compiled at a local endpoint.
    #[cfg(test)]
    pub(crate) fn with_test_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            http: Http::new(),
            endpoint: endpoint.into(),
        }
    }

    /// GET `endpoint + path` with query params, returning the raw HTML body.
    pub async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<String, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        let creds = self.clearance();
        let creds_ref = creds
            .as_ref()
            .map(|(ua, cookie)| (ua.as_str(), cookie.as_str()));
        let (status, text) = self.http.get(&url, params, creds_ref).await?;
        self.finish(status, text)
    }

    /// POST form-urlencoded to `endpoint + path` (the chapter feed).
    pub async fn post_form(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<String, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        let creds = self.clearance();
        let creds_ref = creds
            .as_ref()
            .map(|(ua, cookie)| (ua.as_str(), cookie.as_str()));
        let (status, text) = self.http.post_form(&url, form, creds_ref).await?;
        self.finish(status, text)
    }

    /// Shared status→error mapping + captcha detection on an already-read body.
    fn finish(&self, status: u16, text: String) -> Result<String, ProviderError> {
        if !(200..300).contains(&status) {
            // A 403 carrying the challenge marker means our clearance died.
            if status == 403 || status == 503 {
                if let Some(state) = crate::infrastructure::nu_bridge::global() {
                    state.mark_stale();
                }
            }
            return Err(ProviderError::from_http_status(PROVIDER_ID, status));
        }
        if super::response::is_captcha_page(&text) {
            if let Some(state) = crate::infrastructure::nu_bridge::global() {
                state.mark_stale();
            }
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "NovelUpdates served an anti-bot challenge (Cloudflare). \
                          The hidden browser session is re-solving it; try again shortly."
                    .to_string(),
            });
        }
        Ok(text)
    }
}

/// Any transport-level failure is retryable.
fn transport_error(message: String) -> ProviderError {
    ProviderError::Transport {
        provider: PROVIDER_ID.to_string(),
        message,
    }
}

/// Read status + body off the compiled transport's response type.
#[cfg(feature = "nu-impersonate")]
async fn read_body(response: wreq::Response) -> Result<(u16, String), ProviderError> {
    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: e.to_string(),
        })?;
    Ok((status, text))
}

#[cfg(not(feature = "nu-impersonate"))]
async fn read_body(response: reqwest::Response) -> Result<(u16, String), ProviderError> {
    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: e.to_string(),
        })?;
    Ok((status, text))
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn client_with(server: &MockServer) -> NovelUpdatesClient {
        NovelUpdatesClient::with_test_endpoint(server.uri())
    }

    #[tokio::test]
    async fn get_returns_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .and(query_param("sh", "dungeon"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("novelupdates", "search_series.html")),
            )
            .mount(&server)
            .await;

        let html = client_with(&server)
            .get("/series-finder/", &[("sh", "dungeon")])
            .await
            .unwrap();
        assert!(html.contains("search_main_box_nu"));
    }

    #[tokio::test]
    async fn post_form_sends_urlencoded_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/wp-admin/admin-ajax.php"))
            .and(body_string("action=nd_getchapters&mygrr=0&mypostid=42817"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("novelupdates", "chapters_dungeon_defender.html")),
            )
            .mount(&server)
            .await;

        let html = client_with(&server)
            .post_form(
                "/wp-admin/admin-ajax.php",
                &[
                    ("action", "nd_getchapters"),
                    ("mygrr", "0"),
                    ("mypostid", "42817"),
                ],
            )
            .await
            .unwrap();
        assert!(html.contains("sp_li_chp"));
    }

    #[tokio::test]
    async fn maps_captcha_page_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("novelupdates", "captcha.html")),
            )
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get("/series-finder/", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get("/series-finder/", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series/nope/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get("/series/nope/", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get("/series-finder/", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }
}
