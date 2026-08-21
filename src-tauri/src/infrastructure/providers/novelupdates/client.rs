//! NovelUpdates transport (MISSION-065, API_PROVIDERS §14).
//!
//! NU is server-rendered HTML: one GET for search/details, one form POST to
//! `wp-admin/admin-ajax.php` for the chapter feed. Policy (rate limit, timeout,
//! retry/backoff, cancel) lives in the coordinator above this; the client only
//! maps transport/HTTP outcomes to `ProviderError`. Responses are returned as
//! raw text (HTML), which `response` parses. A Cloudflare/anti-bot challenge is
//! detected by `<title>` and reported as a non-retryable `InvalidResponse` —
//! NU serves these to non-browser clients instead of HTTP 403.

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// A polite, identifiable User-Agent. NU's anti-bot layer tolerates browser-ish
/// clients; a descriptive UA also satisfies the site's acceptable-use spirit
/// (API_PROVIDERS §14 — light, throttled scraping).
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; NovelUpdates series metadata)"
);

/// A reqwest-backed NovelUpdates client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct NovelUpdatesClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for NovelUpdatesClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NovelUpdatesClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self::with_client(http)
    }

    pub fn with_client(http: reqwest::Client) -> Self {
        Self {
            http,
            endpoint: super::ENDPOINT.to_string(),
        }
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
        }
    }

    /// GET `endpoint + path` with query params, returning the raw HTML body.
    /// Maps transport failures → `Transport`, HTTP status → `from_http_status`
    /// (404→NotFound, 429→RateLimited, 5xx→ProviderDown) and an anti-bot
    /// challenge page → `InvalidResponse`.
    pub async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<String, ProviderError> {
        let response = self
            .http
            .get(format!("{}{}", self.endpoint, path))
            .query(params)
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;

        self.read_html(response).await
    }

    /// POST form-urlencoded to `endpoint + path` (the chapter feed). NU needs
    /// `Content-Type: application/x-www-form-urlencoded`, which `.form` sends.
    pub async fn post_form(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<String, ProviderError> {
        let response = self
            .http
            .post(format!("{}{}", self.endpoint, path))
            .form(form)
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;

        self.read_html(response).await
    }

    /// Shared status→error mapping + captcha detection for any response.
    async fn read_html(&self, response: reqwest::Response) -> Result<String, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::from_http_status(
                PROVIDER_ID,
                status.as_u16(),
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;

        if super::response::is_captcha_page(&text) {
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "NovelUpdates served an anti-bot challenge (Cloudflare). \
                          This is not retryable from the app — try again later."
                    .to_string(),
            });
        }

        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::novelupdates_fixture;

    fn client_with(server: &MockServer) -> NovelUpdatesClient {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .unwrap();
        NovelUpdatesClient::with_endpoint(http, server.uri())
    }

    #[tokio::test]
    async fn get_sends_ua_and_returns_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .and(query_param("sh", "dungeon"))
            .and(header("user-agent", APP_USER_AGENT))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(novelupdates_fixture("search_series.html")),
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
                    .set_body_string(novelupdates_fixture("chapters_dungeon_defender.html")),
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
                ResponseTemplate::new(200).set_body_string(novelupdates_fixture("captcha.html")),
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
