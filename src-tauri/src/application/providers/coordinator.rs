//! The provider coordinator (MISSION-053, ARCHITECTURE §4, ADR-004).
//!
//! Owns the *policy* around provider calls; transport never does. For any
//! provider call it applies, in order: cancellation checks → rate limiting →
//! per-attempt timeout → retry with exponential backoff + jitter (honoring the
//! server's Retry-After) → typed `ProviderError` mapping. `search_all` fans out
//! across the enabled, capable providers and aggregates per-provider outcomes
//! so one bad provider never fails the whole search.
//!
//! The coordinator is HTTP-agnostic: it works over the `Provider` trait, so the
//! entire policy layer is unit-testable with fake adapters and paused time.

use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task::JoinSet;

use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::ProviderCapabilities;
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::ProviderCandidate;
use crate::domain::provider::Provider;

use super::config::ProviderConfig;
use super::rate_limiter::RateLimiter;

/// Cooperative cancellation: a shared, cloneable flag that downstream calls
/// poll. Any clone can cancel all clones. Implemented with
/// `tokio::sync::Notify` (no `tokio-util`); a missed notify is harmless because
/// callers re-check the atomic flag at every attempt boundary.
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationInner>,
}

struct CancellationInner {
    flag: AtomicBool,
    notify: Notify,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            inner: Arc::new(CancellationInner {
                flag: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.inner.flag.store(true, Ordering::Release);
        self.inner.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.flag.load(Ordering::Acquire)
    }

    /// Resolves once the token is canceled. Safe to race with `cancel()`: a
    /// missed notification is caught by the caller's `is_cancelled()` checks.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.inner.notify.notified().await;
    }
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// A provider registered with the coordinator, with the policy it runs under.
struct ProviderEntry {
    config: ProviderConfig,
    provider: Arc<dyn Provider>,
    limiter: Arc<RateLimiter>,
}

/// Read-only provider info exposed to the settings UI (REQ-PROV-001).
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub capabilities: ProviderCapabilities,
    pub enabled: bool,
}

/// One provider's contribution to a `search_all`.
pub struct SearchHit {
    pub provider: String,
    pub candidate: ProviderCandidate,
}

/// One provider that failed during `search_all`, without aborting the rest.
pub struct SearchFailure {
    pub provider: String,
    pub error: ProviderError,
}

/// Aggregated result of a multi-provider search: successes and failures are
/// kept separate so the UI can show partial results + per-provider errors.
pub struct SearchOutcome {
    pub hits: Vec<SearchHit>,
    pub failures: Vec<SearchFailure>,
}

pub struct ProviderCoordinator {
    entries: Vec<ProviderEntry>,
}

impl ProviderCoordinator {
    /// Build the coordinator from `(config, adapter)` pairs. Adapters whose id
    /// doesn't match their config are rejected. A coordinator with no entries
    /// is valid — `search_all` just returns no hits.
    pub fn new(
        pairs: impl IntoIterator<Item = (ProviderConfig, Arc<dyn Provider>)>,
    ) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (config, provider) in pairs {
            if provider.id() != config.id {
                return Err(format!(
                    "provider adapter id {:?} does not match config id {:?}",
                    provider.id(),
                    config.id
                ));
            }
            let limiter = Arc::new(RateLimiter::new(config.rate_limit.requests_per_sec));
            entries.push(ProviderEntry {
                config,
                provider,
                limiter,
            });
        }
        Ok(Self { entries })
    }

    /// Snapshot of registered providers (for the capability/settings UI).
    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.entries
            .iter()
            .map(|e| ProviderInfo {
                id: e.config.id.clone(),
                name: e.provider.name().to_string(),
                capabilities: *e.provider.capabilities(),
                enabled: e.config.enabled,
            })
            .collect()
    }

    fn entry(&self, provider_id: &str) -> Option<&ProviderEntry> {
        self.entries.iter().find(|e| e.config.id == provider_id)
    }

    /// Run one provider call under full policy. `op` is a closure re-invoked
    /// per attempt (so a timed-out/canceled future is dropped and a fresh
    /// request is made on retry).
    pub async fn execute<F, Fut, T>(
        &self,
        provider_id: &str,
        token: &CancellationToken,
        op: F,
    ) -> Result<T, ProviderError>
    where
        F: Fn() -> Fut + Send,
        Fut: Future<Output = Result<T, ProviderError>> + Send,
    {
        let entry = self
            .entry(provider_id)
            .ok_or_else(|| ProviderError::Unsupported {
                provider: provider_id.to_string(),
                operation: "calls".to_string(),
            })?;
        run_with_policy(provider_id, &entry.config, &entry.limiter, token, op).await
    }

    /// Search every enabled provider that can serve `content_type` (empty set =
    /// domain-agnostic) in parallel. Returns partial hits + per-provider
    /// failures; never aborts the whole search because one provider errored.
    pub async fn search_all(
        &self,
        query: &str,
        content_type: Option<ContentType>,
        token: &CancellationToken,
    ) -> SearchOutcome {
        let targets: Vec<(usize, &ProviderEntry)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.config.enabled && serves(e, content_type))
            .collect();

        let mut hits = Vec::new();
        let mut failures = Vec::new();
        if targets.is_empty() {
            return SearchOutcome { hits, failures };
        }

        let mut set: JoinSet<(usize, Result<Vec<ProviderCandidate>, ProviderError>)> =
            JoinSet::new();
        for (idx, entry) in targets {
            let provider = entry.provider.clone();
            let id = entry.config.id.clone();
            let config = entry.config.clone();
            let limiter = entry.limiter.clone();
            let q = query.to_string();
            let token = token.clone();
            set.spawn(async move {
                let result = run_with_policy(&id, &config, &limiter, &token, || {
                    let provider = provider.clone();
                    let q = q.clone();
                    async move { provider.search(&q, content_type).await }
                })
                .await;
                (idx, result)
            });
        }

        let mut results: Vec<Option<Result<Vec<ProviderCandidate>, ProviderError>>> =
            vec![None; self.entries.len()];
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((idx, result)) => results[idx] = Some(result),
                Err(join_err) => failures.push(SearchFailure {
                    provider: "unknown".to_string(),
                    error: ProviderError::Transport {
                        provider: "unknown".to_string(),
                        message: format!("task failed: {join_err}"),
                    },
                }),
            }
        }

        // Re-emit in registration order for stable UI ordering.
        for (idx, result) in results.iter().enumerate() {
            match result {
                Some(Ok(found)) => {
                    for candidate in found {
                        hits.push(SearchHit {
                            provider: self.entries[idx].config.id.clone(),
                            candidate: candidate.clone(),
                        });
                    }
                }
                Some(Err(error)) => failures.push(SearchFailure {
                    provider: self.entries[idx].config.id.clone(),
                    error: error.clone(),
                }),
                None => {}
            }
        }

        SearchOutcome { hits, failures }
    }

    /// A fresh, never-cancelled token for callers that don't own one yet.
    pub fn token(&self) -> CancellationToken {
        CancellationToken::new()
    }
}

/// Whether a provider entry is eligible to serve a search for `content_type`.
/// Empty `content_types` = domain-agnostic (answers any search).
fn serves(entry: &ProviderEntry, content_type: Option<ContentType>) -> bool {
    match content_type {
        None => true,
        Some(ct) => {
            entry.config.content_types.is_empty() || entry.config.content_types.contains(&ct)
        }
    }
}

/// The retry loop. Not public API — shared by `execute` and `search_all`.
async fn run_with_policy<F, Fut, T>(
    provider: &str,
    config: &ProviderConfig,
    limiter: &RateLimiter,
    token: &CancellationToken,
    op: F,
) -> Result<T, ProviderError>
where
    F: Fn() -> Fut + Send,
    Fut: Future<Output = Result<T, ProviderError>> + Send,
{
    let mut attempt: u32 = 0;
    loop {
        if token.is_cancelled() {
            return Err(ProviderError::Canceled {
                provider: provider.to_string(),
            });
        }
        limiter.acquire().await;

        let result = tokio::select! {
            biased;
            _ = token.cancelled() => {
                return Err(ProviderError::Canceled { provider: provider.to_string() });
            }
            r = tokio::time::timeout(config.timeout, op()) => r,
        };

        match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err)) if err.is_retryable() && attempt < config.max_retries => {
                attempt += 1;
                wait_for_retry(config, attempt, err.retry_after(), token, provider).await?;
            }
            Ok(Err(err)) => return Err(err),
            Err(_elapsed) if attempt < config.max_retries => {
                attempt += 1;
                wait_for_retry(config, attempt, None, token, provider).await?;
            }
            Err(_elapsed) => {
                return Err(ProviderError::Timeout {
                    provider: provider.to_string(),
                });
            }
        }
    }
}

/// Sleep the backoff (exponential + jitter, capped, honoring Retry-After),
/// cancellable at any moment.
async fn wait_for_retry(
    config: &ProviderConfig,
    attempt: u32,
    retry_after: Option<Duration>,
    token: &CancellationToken,
    provider: &str,
) -> Result<(), ProviderError> {
    let delay = backoff_delay(config, attempt, retry_after);
    tokio::select! {
        biased;
        _ = token.cancelled() => Err(ProviderError::Canceled {
            provider: provider.to_string(),
        }),
        _ = tokio::time::sleep(delay) => Ok(()),
    }
}

/// Exponential backoff with jitter: `backoff_base * 2^(attempt-1)`, capped at
/// `backoff_max`, plus up to 25% jitter. A server `Retry-After` overrides the
/// computed value (still capped at `backoff_max`).
///
/// Deterministic per `attempt` so tests can compute the exact delay.
fn backoff_delay(config: &ProviderConfig, attempt: u32, retry_after: Option<Duration>) -> Duration {
    let base_ms = config.backoff_base.as_millis() as u64;
    let exponent = attempt.saturating_sub(1).min(8);
    let exp_ms = base_ms.saturating_mul(1u64 << exponent);
    let capped_ms = exp_ms.min(config.backoff_max.as_millis() as u64);

    let ms = match retry_after {
        Some(server) => server.as_millis().min(config.backoff_max.as_millis()) as u64,
        None => capped_ms,
    };

    let jitter = xorshift64(attempt as u64 + 0x9e37_79b9) % (ms / 4 + 1);
    Duration::from_millis(ms + jitter)
}

/// Small deterministic PRNG so jitter needs no external crate and tests are
/// reproducible.
fn xorshift64(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering as AOrdering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::domain::provider::{AuthKind, ProviderMedia};

    #[derive(Clone)]
    enum Behavior {
        Ok(Vec<ProviderCandidate>),
        Fail(ProviderError),
        FailTimes {
            remaining: u32,
            error: ProviderError,
            then: Vec<ProviderCandidate>,
        },
        Hang,
    }

    struct FakeProvider {
        id: String,
        name: String,
        caps: ProviderCapabilities,
        behavior: Mutex<Behavior>,
    }

    impl FakeProvider {
        fn make(id: &str, caps: ProviderCapabilities, behavior: Behavior) -> Arc<dyn Provider> {
            Arc::new(FakeProvider {
                id: id.to_string(),
                name: format!("Fake {id}"),
                caps,
                behavior: Mutex::new(behavior),
            })
        }
    }

    #[async_trait::async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            &self.caps
        }
        async fn search(
            &self,
            _query: &str,
            _content_type: Option<ContentType>,
        ) -> Result<Vec<ProviderCandidate>, ProviderError> {
            let behavior = self.behavior.lock().unwrap().clone();
            match behavior {
                Behavior::Ok(hits) => Ok(hits),
                Behavior::Fail(error) => Err(error),
                Behavior::FailTimes {
                    remaining,
                    error,
                    then,
                } => {
                    if remaining > 0 {
                        *self.behavior.lock().unwrap() = Behavior::FailTimes {
                            remaining: remaining - 1,
                            error: error.clone(),
                            then: then.clone(),
                        };
                        Err(error)
                    } else {
                        Ok(then)
                    }
                }
                Behavior::Hang => std::future::pending().await,
            }
        }
        async fn get_details(&self, _provider_id: &str) -> Result<ProviderMedia, ProviderError> {
            Err(ProviderError::Unsupported {
                provider: self.id.clone(),
                operation: "details".into(),
            })
        }
    }

    fn hits(n: usize) -> Vec<ProviderCandidate> {
        (0..n)
            .map(|i| ProviderCandidate {
                provider: String::new(),
                provider_id: format!("hit-{i}"),
                title: format!("Title {i}"),
                content_type: ContentType::Book,
                release_year: None,
                cover_url: None,
                synopsis: None,
                external_ids: Vec::new(),
                url: None,
            })
            .collect()
    }

    fn base_config(id: &str) -> ProviderConfig {
        ProviderConfig::new(id).with_requests_per_sec(0.0)
    }

    fn coord(pairs: Vec<(ProviderConfig, Arc<dyn Provider>)>) -> ProviderCoordinator {
        ProviderCoordinator::new(pairs).unwrap()
    }

    #[test]
    fn rejects_mismatched_adapter_config() {
        let provider =
            FakeProvider::make("a", ProviderCapabilities::default(), Behavior::Ok(vec![]));
        assert!(ProviderCoordinator::new(vec![(ProviderConfig::new("b"), provider)]).is_err());
    }

    #[test]
    fn providers_snapshot_reports_capabilities_and_enabled() {
        let caps = ProviderCapabilities {
            search: true,
            auth: AuthKind::Key,
            ..Default::default()
        };
        let provider = FakeProvider::make("anilist", caps, Behavior::Ok(vec![]));
        let c = coord(vec![(base_config("anilist"), provider)]);
        let info = c.providers();
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].id, "anilist");
        assert!(info[0].capabilities.supports_search());
        assert!(info[0].enabled);
    }

    #[tokio::test(start_paused = true)]
    async fn execute_retries_retryable_errors_then_succeeds() {
        let provider = FakeProvider::make(
            "fake",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::FailTimes {
                remaining: 2,
                error: ProviderError::RateLimited {
                    provider: "fake".into(),
                    retry_after: None,
                },
                then: hits(1),
            },
        );
        let c = coord(vec![(base_config("fake"), provider.clone())]);
        let token = c.token();
        let calls = Arc::new(AtomicU32::new(0));
        let calls1 = calls.clone();
        let result = c
            .execute("fake", &token, move || {
                calls1.fetch_add(1, AOrdering::SeqCst);
                let provider = provider.clone();
                async move { provider.search("x", None).await }
            })
            .await;
        assert!(result.is_ok());
        assert_eq!(calls.load(AOrdering::SeqCst), 3, "1 initial + 2 retries");
    }

    #[tokio::test(start_paused = true)]
    async fn non_retryable_errors_are_not_retried() {
        let provider = FakeProvider::make(
            "fake",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Fail(ProviderError::NotFound {
                provider: "fake".into(),
            }),
        );
        let c = coord(vec![(base_config("fake"), provider.clone())]);
        let token = c.token();
        let calls = Arc::new(AtomicU32::new(0));
        let calls1 = calls.clone();
        let result = c
            .execute("fake", &token, move || {
                calls1.fetch_add(1, AOrdering::SeqCst);
                let provider = provider.clone();
                async move { provider.search("x", None).await }
            })
            .await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
        assert_eq!(calls.load(AOrdering::SeqCst), 1, "no retry on 404");
    }

    #[tokio::test(start_paused = true)]
    async fn timeouts_after_the_configured_limit() {
        let provider = FakeProvider::make(
            "fake",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Hang,
        );
        let mut config = base_config("fake");
        config.timeout = Duration::from_millis(100);
        config.max_retries = 0;
        let config = Arc::new(config);
        let limiter = Arc::new(RateLimiter::new(0.0));
        let token = CancellationToken::new();
        let handle = tokio::spawn({
            let config = config.clone();
            let limiter = limiter.clone();
            let token = token.clone();
            let provider = provider.clone();
            async move {
                run_with_policy("fake", &config, &limiter, &token, || {
                    let provider = provider.clone();
                    async move { provider.search("x", None).await }
                })
                .await
            }
        });
        tokio::time::advance(Duration::from_millis(101)).await;
        let result = handle.await.unwrap();
        assert!(matches!(result, Err(ProviderError::Timeout { .. })));
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_wakes_the_backoff_sleep() {
        let provider = FakeProvider::make(
            "fake",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Fail(ProviderError::RateLimited {
                provider: "fake".into(),
                retry_after: None,
            }),
        );
        let mut config = base_config("fake");
        config.backoff_base = Duration::from_secs(10);
        config.backoff_max = Duration::from_secs(10);
        config.max_retries = 3;
        let config = Arc::new(config);
        let limiter = Arc::new(RateLimiter::new(0.0));
        let token = CancellationToken::new();
        let handle = tokio::spawn({
            let config = config.clone();
            let limiter = limiter.clone();
            let token = token.clone();
            let provider = provider.clone();
            async move {
                run_with_policy("fake", &config, &limiter, &token, || {
                    let provider = provider.clone();
                    async move { provider.search("x", None).await }
                })
                .await
            }
        });
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        token.cancel();
        tokio::time::advance(Duration::from_millis(1)).await;
        let result = handle.await.unwrap();
        assert!(matches!(result, Err(ProviderError::Canceled { .. })));
    }

    #[tokio::test(start_paused = true)]
    async fn search_all_aggregates_successes_and_failures() {
        let ok_provider = FakeProvider::make(
            "anilist",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Ok(hits(2)),
        );
        let failing_provider = FakeProvider::make(
            "tmdb",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Fail(ProviderError::NotFound {
                provider: "tmdb".into(),
            }),
        );
        let c = coord(vec![
            (base_config("anilist"), ok_provider),
            (base_config("tmdb"), failing_provider),
        ]);
        let token = c.token();
        let outcome = c.search_all("naruto", None, &token).await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "anilist"));
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].provider, "tmdb");
        assert!(matches!(
            outcome.failures[0].error,
            ProviderError::NotFound { .. }
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn search_all_skips_disabled_and_unsupported_providers() {
        let mut disabled = base_config("disabled");
        disabled.enabled = false;
        let mut wrong_domain = base_config("anilist");
        wrong_domain.content_types = vec![ContentType::Manga];
        let agnostic = base_config("openlibrary");

        let c = coord(vec![
            (
                disabled,
                FakeProvider::make(
                    "disabled",
                    ProviderCapabilities {
                        search: true,
                        ..Default::default()
                    },
                    Behavior::Ok(hits(1)),
                ),
            ),
            (
                wrong_domain,
                FakeProvider::make(
                    "anilist",
                    ProviderCapabilities {
                        search: true,
                        ..Default::default()
                    },
                    Behavior::Ok(hits(1)),
                ),
            ),
            (
                agnostic,
                FakeProvider::make(
                    "openlibrary",
                    ProviderCapabilities {
                        search: true,
                        ..Default::default()
                    },
                    Behavior::Ok(hits(1)),
                ),
            ),
        ]);
        let token = c.token();
        let outcome = c.search_all("x", Some(ContentType::Book), &token).await;
        assert_eq!(outcome.hits.len(), 1);
        assert_eq!(outcome.hits[0].provider, "openlibrary");
        assert!(outcome.failures.is_empty());
    }

    #[test]
    fn backoff_delay_grows_exponentially_and_is_capped() {
        let mut config = base_config("x");
        config.backoff_base = Duration::from_millis(100);
        config.backoff_max = Duration::from_millis(1000);
        let d1 = backoff_delay(&config, 1, None);
        let d2 = backoff_delay(&config, 2, None);
        let d3 = backoff_delay(&config, 3, None);
        assert!(d1 < d2, "exponential growth");
        assert!(d2 < d3, "exponential growth");
        assert!(d3 <= Duration::from_millis(1250), "capped + jitter");
        let capped = backoff_delay(&config, 1, Some(Duration::from_secs(30)));
        assert!(capped >= Duration::from_millis(1000), "Retry-After wins");
        assert!(capped <= Duration::from_millis(1250), "Retry-After capped");
    }

    #[test]
    fn jitter_is_deterministic_per_attempt() {
        let mut config = base_config("x");
        config.backoff_base = Duration::from_millis(100);
        let a = backoff_delay(&config, 2, None);
        let b = backoff_delay(&config, 2, None);
        assert_eq!(a, b);
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_spaces_parallel_execute_calls() {
        let provider = FakeProvider::make(
            "fake",
            ProviderCapabilities {
                search: true,
                ..Default::default()
            },
            Behavior::Ok(hits(1)),
        );
        let mut config = base_config("fake");
        config.rate_limit.requests_per_sec = 2.0; // 500ms spacing
        let c = Arc::new(coord(vec![(config, provider)]));
        let token = c.token();
        let op = || async { Ok(()) };
        let one = tokio::spawn({
            let c = c.clone();
            let token = token.clone();
            async move { c.execute("fake", &token, op).await }
        });
        let two = tokio::spawn({
            let c = c.clone();
            let token = token.clone();
            async move { c.execute("fake", &token, op).await }
        });
        let three = tokio::spawn({
            let c = c.clone();
            let token = token.clone();
            async move { c.execute("fake", &token, op).await }
        });
        tokio::task::yield_now().await;
        // The three calls share one limiter; the 2nd/3rd acquires sleep until
        // virtual 500ms / 1000ms. Nothing resolves until we advance the clock.
        tokio::time::advance(Duration::from_millis(1100)).await;
        let (r1, r2, r3) = tokio::join!(one, two, three);
        assert!(r1.unwrap().is_ok() && r2.unwrap().is_ok() && r3.unwrap().is_ok());
    }
}
