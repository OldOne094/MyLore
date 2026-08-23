//! NovelUpdates transport (MISSION-065, MISSION-129, API_PROVIDERS §14).
//!
//! Production requests run **inside the harvester webview** via
//! `infrastructure::nu_bridge::fetch` — same-origin fetch with credentials,
//! so Cloudflare's HttpOnly clearance rides automatically and challenges
//! renew silently. Tests bypass the bridge with a plain reqwest client
//! pointed at wiremock. Status/captcha mapping is shared by both paths.

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// Browser-ish UA for the test transport only.
#[cfg(test)]
const TEST_UA: &str = "MyLore tests (NovelUpdates adapter)";

/// A NovelUpdates client.
#[derive(Clone)]
pub struct NovelUpdatesClient {
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
            endpoint: super::ENDPOINT.to_string(),
        }
    }

    /// Test hook: point the direct (non-bridged) path at a local endpoint.
    #[cfg(test)]
    pub(crate) fn with_test_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    /// GET `endpoint + path` with query params, returning the raw HTML body.
    pub async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<String, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        #[cfg(test)]
        return self
            .direct(reqwest::Method::GET, &url, None, Some(params))
            .await;
        #[cfg(not(test))]
        {
            let query = encode_query(params);
            let target = if query.is_empty() {
                url
            } else {
                format!("{url}?{query}")
            };
            let (status, text) = crate::infrastructure::nu_bridge::fetch(&target, None)
                .await
                .map_err(|e| transport_error(e.to_string()))?;
            self.finish(status, text)
        }
    }

    /// POST form-urlencoded to `endpoint + path` (the chapter feed).
    pub async fn post_form(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<String, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        #[cfg(test)]
        return self
            .direct(reqwest::Method::POST, &url, Some(form), None)
            .await;
        #[cfg(not(test))]
        {
            let (status, text) = crate::infrastructure::nu_bridge::fetch(&url, Some(form))
                .await
                .map_err(|e| transport_error(e.to_string()))?;
            self.finish(status, text)
        }
    }

    /// Shared status→error mapping + captcha detection on an already-read body.
    fn finish(&self, status: u16, text: String) -> Result<String, ProviderError> {
        if !(200..300).contains(&status) {
            return Err(ProviderError::from_http_status(PROVIDER_ID, status));
        }
        if super::response::is_captcha_page(&text) {
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "NovelUpdates served an anti-bot challenge (Cloudflare). \
                          The in-app browser session renews it automatically; try again shortly."
                    .to_string(),
            });
        }
        Ok(text)
    }

    /// Wiremock-facing direct request used only by unit tests.
    #[cfg(test)]
    async fn direct(
        &self,
        method: reqwest::Method,
        url: &str,
        form: Option<&[(&str, &str)]>,
        params: Option<&[(&str, &str)]>,
    ) -> Result<String, ProviderError> {
        static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
        let http = CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(TEST_UA)
                .build()
                .expect("reqwest client builds")
        });

        let mut request = http.request(method, url);
        if let Some(params) = params {
            request = request.query(params);
        }
        if let Some(form) = form {
            request = request.form(form);
        }
        let response = request.send().await.map_err(|e| ProviderError::Transport {
            provider: PROVIDER_ID.to_string(),
            message: e.to_string(),
        })?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        self.finish(status, text)
    }
}

#[cfg(not(test))]
fn transport_error(message: String) -> ProviderError {
    ProviderError::Transport {
        provider: PROVIDER_ID.to_string(),
        message,
    }
}

#[cfg(not(test))]
fn encode_query(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                transport_error_escape(k),
                transport_error_escape(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Minimal percent-encoding for the fixed, simple keys/values this adapter
/// sends (letters, digits and a few safe punctuation marks pass through).
#[cfg(not(test))]
fn transport_error_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
