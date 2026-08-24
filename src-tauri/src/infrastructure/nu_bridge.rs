//! NovelUpdates fetch bridge (MISSION-129).
//!
//! NU's Cloudflare zone enforces an active managed challenge AND marks its
//! clearance cookie HttpOnly — so neither TLS impersonation nor
//! `document.cookie` scraping can see it (both verified live, 2026-08-23).
//!
//! The definitive route: perform NU HTTP calls *inside* the harvester webview
//! itself via same-origin `fetch(credentials: include)`. The browser attaches
//! its own HttpOnly cookies, auto-renews challenges silently, and presents a
//! genuine fingerprint — there is nothing left to impersonate. Responses come
//! back through an intercepted navigation (`on_navigation`, cancelled before
//! any network touch), chunked because NU pages reach hundreds of KB.
//!
//! The visible mini window is required: WebView2 never ran scripts for a
//! never-shown window. It hides itself after the first successful fetch and
//! re-shows if a challenge is detected later (manual Turnstile fallback).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use tokio::sync::oneshot;

/// The hidden-ish window's label (also its capability scope).
pub const WINDOW_LABEL: &str = "nu-fetch";
/// The page the harvester loads; any NU page works, the homepage is lightest.
pub const TARGET_URL: &str = "https://www.novelupdates.com/";
/// Fake host carrying responses back to Rust (never resolved; navigations to
/// it are cancelled inside `handle_report_navigation`).
pub const REPORT_HOST: &str = "nu-report.mylore.internal";

/// Max base64 chars per report navigation (well under browser URL caps).
const CHUNK_SIZE: usize = 60_000;

type PendingResult = Result<(u16, String), String>;

/// Shared bridge state: pending request channels + whether the page ever
/// answered (drives window visibility).
#[derive(Default)]
pub struct BridgeState {
    pending: Mutex<HashMap<String, oneshot::Sender<PendingResult>>>,
}

impl BridgeState {
    fn register(self: &Arc<Self>, id: String) -> oneshot::Receiver<PendingResult> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        rx
    }

    fn resolve(self: &Arc<Self>, id: &str, result: PendingResult) {
        if let Some(tx) = self.pending.lock().unwrap().remove(id) {
            let _ = tx.send(result);
        }
    }
}

/// A Cloudflare challenge was detected in a response: bring the harvester
/// window back on-screen (centered enough to notice) and reload so the
/// interactive challenge can be solved; the next successful fetch hides it
/// again.
pub fn on_challenge() {
    if let Some(window) = WINDOW.get() {
        let _ = window.show();
        let _ = window.set_focus();
        // Pull it back from the offscreen parking position.
        let _ = window.set_position(tauri::PhysicalPosition::new(120, 120));
        let _ = window.eval("location.reload();");
    }
}

static GLOBAL: std::sync::OnceLock<Arc<BridgeState>> = std::sync::OnceLock::new();
static WINDOW: std::sync::OnceLock<tauri::WebviewWindow> = std::sync::OnceLock::new();

/// Install the process-wide bridge state (once, from app setup).
pub fn init_global(state: Arc<BridgeState>) -> bool {
    GLOBAL.set(state).is_ok()
}

fn global() -> Option<&'static Arc<BridgeState>> {
    GLOBAL.get()
}

/// Register the harvester window for visibility toggling.
pub fn attach_window(window: &tauri::WebviewWindow) {
    let _ = WINDOW.set(window.clone());
}

fn sync_visibility(page_proven_working: bool) {
    if let Some(window) = WINDOW.get() {
        if page_proven_working {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// One-shot bootstrap + call: defines the worker on `window` when missing,
/// then invokes it. Everything is escaped server-side via serde_json strings.
/// Any setup/invocation failure itself reports back through the same channel,
/// so a broken injection can never manifest as a silent timeout again.
fn call_worker_js(id: &str, url: &str, opts_json: &str) -> String {
    format!(
        r#"(function(){{
          const reportErr = (m) => {{
            const b64url = (s) => btoa(unescape(encodeURIComponent(s)))
              .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
            location.assign('https://{host}/#e|{id}|' + b64url(String(m)));
          }};
          try {{
            if (!window.__myloreFetch) {{
              const b64url = (s) => btoa(unescape(encodeURIComponent(s)))
                .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
              window.__myloreFetch = async (id2, url2, optsJson2) => {{
                try {{
                  const opts = JSON.parse(optsJson2 || '{{}}');
                  const init = {{ credentials: 'include', method: opts.method || 'GET' }};
                  if (opts.body) init.body = opts.body;
                  const response = await fetch(url2, init);
                  const text = await response.text();
                  const payload = b64url(JSON.stringify({{ status: response.status, text }}));
                  const total = Math.max(1, Math.ceil(payload.length / {chunk}));
                  const sendChunk = (i) => {{
                    if (i >= total) return;
                    const chunkPart = payload.substr(i * {chunk}, {chunk});
                    location.assign('https://{host}/#r|' + id2 + '|' + i + '|' + total + '|' + chunkPart);
                    setTimeout(() => sendChunk(i + 1), 60);
                  }};
                  sendChunk(0);
                }} catch (e2) {{
                  location.assign('https://{host}/#e|' + id2 + '|' +
                    b64url(String((e2 && e2.message) || e2)));
                }}
              }};
            }}
            window.__myloreFetch({id}, {url}, {opts});
          }} catch (setupError) {{
            reportErr('setup: ' + String(setupError));
          }}
        }})();"#,
        chunk = CHUNK_SIZE,
        host = REPORT_HOST,
        id = serde_json::to_string(id).unwrap_or_else(|_| "\"\"".into()),
        url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into()),
        opts = serde_json::to_string(opts_json).unwrap_or_else(|_| "\"\"".into()),
    )
}

/// Perform one NU HTTP round-trip through the webview. `form` implies POST
/// form-urlencoded; otherwise GET.
pub async fn fetch(
    url: &str,
    form: Option<&[(&str, &str)]>,
) -> Result<(u16, String), crate::error::AppError> {
    let state = global()
        .ok_or_else(|| crate::error::AppError::internal("NovelUpdates bridge not initialised"))?;
    let Some(window) = WINDOW.get() else {
        return Err(crate::error::AppError::internal(
            "NovelUpdates bridge window missing",
        ));
    };

    let id = uuid::Uuid::new_v4().to_string();
    let rx = state.register(id.clone());

    let body = form.map(|pairs| {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencoding_escape(v)))
            .collect::<Vec<_>>()
            .join("&")
    });
    let method = if form.is_some() { "POST" } else { "GET" };
    // Note: NU's admin-ajax POST expects the URL-encoded body verbatim; keys
    // are fixed literals from the adapter, values are numeric/simple ids.
    let opts = serde_json::json!({ "method": method, "body": body }).to_string();

    let js = call_worker_js(&id, url, &opts);
    window
        .eval(&js)
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;

    match tokio::time::timeout(std::time::Duration::from_secs(45), rx).await {
        Ok(Ok(result)) => {
            sync_visibility(true);
            result.map_err(crate::error::AppError::internal)
        }
        Ok(Err(_)) => Err(crate::error::AppError::internal(
            "NovelUpdates bridge dropped the response channel",
        )),
        Err(_) => Err(crate::error::AppError::internal(
            "NovelUpdates bridge timed out (45s)",
        )),
    }
}

/// Minimal application/x-www-form-urlencoded value escaping for the fixed,
/// known-simple values this app sends (ids and digits).
fn urlencoding_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => {
                out.push('%');
                out.push_str(&format!("{other:02X}"));
            }
        }
    }
    out
}

/// Accumulates chunked report navigations per request id.
#[derive(Default)]
struct Assembly {
    parts: Mutex<HashMap<String, BTreeMap<usize, String>>>,
    totals: Mutex<HashMap<String, usize>>,
}

static ASSEMBLY: std::sync::OnceLock<Assembly> = std::sync::OnceLock::new();

fn assembly() -> &'static Assembly {
    ASSEMBLY.get_or_init(Default::default)
}

/// Handle one intercepted navigation. Returns true only for URLs the webview
/// should really load.
pub fn handle_report_navigation(state: &Arc<BridgeState>, url: &tauri::Url) -> bool {
    if url.host_str() != Some(REPORT_HOST) {
        return true;
    }
    let Some(fragment) = url.fragment().filter(|f| !f.is_empty()) else {
        tracing::debug!(%url, "NU report navigation without payload");
        return false;
    };
    let segments: Vec<&str> = fragment.split('|').collect();
    tracing::debug!(
        kind = segments.first().copied().unwrap_or("?"),
        id = segments.get(1).copied().unwrap_or("?"),
        chunk_index = segments.get(2).copied().unwrap_or("?"),
        total = segments.get(3).copied().unwrap_or("?"),
        "NU report navigation"
    );

    match segments.first().copied() {
        Some("r") if segments.len() >= 5 => {
            let id = segments[1].to_string();
            let index: usize = segments[2].parse().unwrap_or(0);
            let total: usize = segments[3].parse().unwrap_or(1);
            let chunk = segments[4].to_string();

            {
                let mut totals = assembly().totals.lock().unwrap();
                totals.entry(id.clone()).or_insert(total);
                let expected = *totals.get(&id).unwrap();
                let _ = expected;
                assembly()
                    .parts
                    .lock()
                    .unwrap()
                    .entry(id.clone())
                    .or_default()
                    .insert(index, chunk);
            }

            let complete = {
                let totals = assembly().totals.lock().unwrap();
                let parts = assembly().parts.lock().unwrap();
                match (totals.get(&id), parts.get(&id)) {
                    (Some(total), Some(parts)) => parts.len() == *total,
                    _ => false,
                }
            };

            if complete {
                let assembled: String = assembly()
                    .parts
                    .lock()
                    .unwrap()
                    .remove(&id)
                    .unwrap_or_default()
                    .into_values()
                    .collect();
                assembly().totals.lock().unwrap().remove(&id);

                let result =
                    decode_response(&assembled).map_err(|e| format!("response decode failed: {e}"));
                state.resolve(&id, result);
            }
        }
        Some("e") if segments.len() >= 3 => {
            let id = segments[1].to_string();
            let message = segments[2..].join("|");
            state.resolve(&id, Err(format!("in-page fetch failed: {message}")));
        }
        _ => {}
    }
    false // cancel: report navigations are envelopes, not destinations
}

/// Decode `{status, text}` JSON transported as base64url. The JS side strips
/// the padding, so decode with the no-pad engine.
fn decode_response(assembled_b64: &str) -> Result<(u16, String), String> {
    let normalized = assembled_b64
        .trim_end_matches('=')
        .replace("-", "+")
        .replace("_", "/");
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(normalized.as_bytes())
        .map_err(|e| format!("base64: {e}"))?;
    #[derive(serde::Deserialize)]
    struct Payload {
        status: u16,
        text: String,
    }
    let payload: Payload = serde_json::from_slice(&bytes).map_err(|e| format!("json: {e}"))?;
    Ok((payload.status, payload.text))
}
