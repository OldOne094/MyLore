//! NovelUpdates Cloudflare-clearance bridge (MISSION-129).
//!
//! NU's Cloudflare zone enforces an *active* managed JavaScript challenge
//! (`cf-mitigated: challenge`) — a perfect TLS fingerprint alone receives
//! "Just a moment…" (live-verified across Chrome/Firefox/Safari/Edge profiles,
//! 2026-08-23). A real browser engine solves the challenge once, silently, and
//! earns a `cf_clearance` cookie that is then honored for plain API-shaped
//! requests from the same IP as long as the UA stays coherent.
//!
//! This module owns that handshake: a hidden webview (`nu-fetch` capability)
//! loads novelupdates.com at startup; its initialization script reports
//! `navigator.userAgent` + `document.cookie` back through the
//! [`NU_CLEARANCE_COMMAND`] command; the credentials live here and are read by
//! the NovelUpdates transport on every request. When a response ever comes
//! back as a challenge page, the client calls [`NuClearanceState::stale`]
//! and the refresher task reloads the hidden page to re-earn clearance.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::Manager;
use tokio::sync::Notify;

/// The hidden window's label (also its capability scope).
pub const WINDOW_LABEL: &str = "nu-fetch";
/// The page the harvester loads; any NU page works, the homepage is lightest.
pub const TARGET_URL: &str = "https://www.novelupdates.com/";
/// The Tauri command the initialization script invokes with its report.
pub const NU_CLEARANCE_COMMAND: &str = "nu_clearance_report";

/// How often the refresher re-solves proactively (clearances outlive this on
/// most zones; refreshing early keeps the cookie perpetually warm).
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// Credentials harvested from the hidden browser session.
#[derive(Debug, Clone)]
pub struct Clearance {
    pub user_agent: String,
    pub cookie: String,
    #[allow(dead_code)]
    pub harvested_at: Instant,
}

/// Shared between the hidden webview's report command and the NU transport.
#[derive(Default)]
pub struct NuClearanceState {
    inner: Mutex<HashMap<String, Clearance>>,
    /// Bumped whenever the client hits a challenge page so the refresher
    /// knows to re-solve immediately instead of waiting for the interval.
    stale_generation: AtomicU64,
    seen_generation: AtomicU64,
    notify: Notify,
}

impl NuClearanceState {
    /// Record a harvest report (last write wins; identical reports no-op).
    pub fn store(&self, user_agent: String, cookie: String) {
        let mut map = self.inner.lock().unwrap();
        let fresh = match map.get("current") {
            Some(prev) => prev.user_agent != user_agent || prev.cookie != cookie,
            None => true,
        };
        if fresh {
            tracing::info!(
                ua_len = user_agent.len(),
                cookie_len = cookie.len(),
                "NovelUpdates clearance harvested"
            );
            map.insert(
                "current".to_string(),
                Clearance {
                    user_agent,
                    cookie,
                    harvested_at: Instant::now(),
                },
            );
        }
        self.notify.notify_waiters();
    }

    /// Current credentials, if a harvest has landed yet.
    pub fn snapshot(&self) -> Option<Clearance> {
        self.inner.lock().unwrap().get("current").cloned()
    }

    /// Mark the current clearance as rejected by Cloudflare.
    pub fn mark_stale(&self) {
        let gen = self.stale_generation.fetch_add(1, Ordering::SeqCst);
        tracing::warn!(generation = gen, "NovelUpdates clearance marked stale");
        self.notify.notify_waiters();
    }

    /// Resolve once the stale flag has moved past `seen`, or after `max`.
    pub async fn wait_refresh(&self, max: std::time::Duration) {
        let seen = self.seen_generation.load(Ordering::SeqCst);
        let target = self.stale_generation.load(Ordering::SeqCst);
        if target != seen {
            self.seen_generation.store(target, Ordering::SeqCst);
            return;
        }
        let _ = tokio::time::timeout(max, self.notify.notified()).await;
        self.seen_generation.store(
            self.stale_generation.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }
}

/// Spawn the periodic (and stale-triggered) refresher for the hidden window.
/// `window` must already exist; the loop only drives `location.reload()`.
pub fn spawn_refresher(app: tauri::AppHandle, state: Arc<NuClearanceState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            state.wait_refresh(REFRESH_INTERVAL).await;
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                tracing::debug!("reloading NovelUpdates harvester window");
                let _ = window.eval("location.reload();");
            }
        }
    });
}

static GLOBAL: std::sync::OnceLock<Arc<NuClearanceState>> = std::sync::OnceLock::new();

/// Install the process-wide clearance state. Called once from app setup.
/// The NovelUpdates transport reads through [`global`] because adapters are
/// constructed by the settings factory, which carries no app state.
pub fn init_global(state: Arc<NuClearanceState>) -> bool {
    GLOBAL.set(state).is_ok()
}

/// The process-wide state, after [`init_global`].
pub fn global() -> Option<&'static Arc<NuClearanceState>> {
    GLOBAL.get()
}
