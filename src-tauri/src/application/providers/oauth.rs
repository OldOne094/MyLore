//! AniList OAuth (MISSION-130).
//!
//! Desktop authorization-code flow over a loopback redirect (RFC 8252 §7):
//! MyLore binds `127.0.0.1:24110`, sends the user's browser to AniList's
//! authorize page, captures `?code=` on the way back, exchanges it (with the
//! registered client secret) for a personal access token, and stores it
//! through the existing provider-key pipeline (secret store + coordinator
//! rebuild). The token never touches the frontend or logs.

use std::io::{Read, Write};
use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

use crate::application::providers::settings::ProviderSettingsService;
use crate::error::AppError;

/// The registered AniList API v2 client ("MyLore").
pub const CLIENT_ID: &str = "48606";
/// Registered per AniList's requirement that code-exchange clients carry it;
/// embedded desktop apps treat this as public knowledge (RFC 8252 §8.4 note).
const CLIENT_SECRET: &str = "KgLCvf8Jyu0kc1w6tH7eYuXdaQpxhQvTaBSHJPBz";
/// Must match the redirect URL registered on the AniList developer page.
pub const REDIRECT_URI: &str = "http://127.0.0.1:24110/auth/anilist/callback";
/// Authorize entry point (implicit-free; we exchange a code server-side).
pub const AUTHORIZE_URL: &str = "https://anilist.co/api/v2/oauth/authorize";
/// Token exchange endpoint.
pub const TOKEN_URL: &str = "https://anilist.co/api/v1/oauth/token";
/// Event emitted to the frontend when the flow finishes either way.
pub const OAUTH_EVENT: &str = "anilist-oauth";

/// Extract `code` and `state` from a callback request target (`/path?query`).
fn extract_code_state(target: &str) -> Option<(String, String)> {
    let query = target.split('?').nth(1)?;
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("code"), Some(v)) => code = Some(v.to_string()),
            (Some("state"), Some(v)) => state = Some(v.to_string()),
            _ => {}
        }
    }
    Some((code?, state?))
}

/// Exchange an authorization code for an access token string.
async fn exchange_code(code: &str) -> Result<String, AppError> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }
    let http = reqwest::Client::new();
    let response = http
        .post(TOKEN_URL)
        .json(&json!({
            "grant_type": "authorization_code",
            "client_id": CLIENT_ID,
            "client_secret": CLIENT_SECRET,
            "redirect_uri": REDIRECT_URI,
            "code": code,
        }))
        .send()
        .await
        .map_err(|e| AppError::internal(format!("anilist token exchange failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::internal(format!(
            "anilist token exchange returned HTTP {}",
            response.status()
        )));
    }
    let payload: TokenResponse = response
        .json()
        .await
        .map_err(|e| AppError::internal(format!("anilist token envelope unreadable: {e}")))?;
    Ok(payload.access_token)
}

/// Kick off the browser flow. Binds the loopback listener synchronously so a
/// port conflict fails loudly, opens the system browser, then finishes the
/// capture/exchange/store on a background task.
pub fn start(
    app: AppHandle,
    settings: Arc<ProviderSettingsService>,
) -> Result<(), AppError> {
    let expected_state = uuid::Uuid::new_v4().to_string();
    let authorize = format!(
        "{AUTHORIZE_URL}?client_id={CLIENT_ID}&redirect_uri={REDIRECT_URI}&response_type=code&state={expected_state}"
    );

    // Bind before opening the browser: the redirect must have somewhere to go.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 24110))
        .map_err(|e| AppError::internal(format!("loopback port 24110 unavailable: {e}")))?;

    app.opener()
        .open_url(authorize, None::<&str>)
        .map_err(|e| AppError::internal(format!("browser launch failed: {e}")))?;

    let app_for_task = app.clone();
    std::thread::spawn(move || {
        let finish = |result: Result<(), String>| {
            let payload = match &result {
                Ok(()) => json!({ "ok": true }),
                Err(message) => json!({ "ok": false, "message": message }),
            };
            let _ = app_for_task.emit(OAUTH_EVENT, payload);
        };

        // One shot: accept until a plausible callback arrives (or time out).
        let _ = listener.set_nonblocking(false);
        let (mut stream, _) = match listener.accept() {
            Ok(conn) => conn,
            Err(e) => return finish(Err(format!("loopback accept failed: {e}"))),
        };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(15)));
        let mut raw = String::new();
        if stream.read_to_string(&mut raw).is_err() {
            // Browsers keep the socket half-open; read what arrived instead.
            raw.clear();
            let mut buf = [0u8; 4096];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            raw.push_str(&String::from_utf8_lossy(&buf));
        }
        let request_line = raw.lines().next().unwrap_or_default().to_string();
        let Some((code, state)) = extract_code_state(&request_line) else {
            return finish(Err("callback carried no code/state".into()));
        };
        if state != expected_state {
            return finish(Err("OAuth state mismatch".into()));
        }

        // Reply to the browser immediately; the user can close the tab.
        let body = "<html><body style=\"font-family:sans-serif;padding:2rem\">\
MyLore connected to AniList. You can close this tab.</body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        drop(stream);

        let token = match futures_block_on(exchange_code(&code)) {
            Ok(token) => token,
            Err(error) => return finish(Err(error.to_string())),
        };
        if let Err(error) = settings.set_key("anilist", &token) {
            return finish(Err(error.to_string()));
        }
        tracing::info!(provider = "anilist", "OAuth token stored");
        finish(Ok(()));
    });

    Ok(())
}

/// Run an async future to completion on this dedicated OS thread (the
/// loopback handler runs off the async runtime by design).
fn futures_block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut pinned = Box::pin(future);
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(pinned.as_mut()),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(pinned.as_mut()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_code_and_state_from_callback() {
        let (code, state) =
            extract_code_state("/auth/anilist/callback?code=abc123&state=st-1").unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state, "st-1");
        assert!(extract_code_state("/auth/anilist/callback?state=only").is_none());
    }
}
