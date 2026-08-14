//! Per-provider rate limiter (MISSION-053, ARCHITECTURE §4).
//!
//! A serialized token-bucket-by-spacing limiter: no two request *starts* from a
//! provider are closer than `1 / rps`. One `RateLimiter` is shared by all
//! in-flight calls for a provider, so parallel fan-out still respects the
//! provider's limit (AniList ~90/min, Jikan 3 rps, OpenLibrary 1 rps, …).
//!
//! `reserve()` is the synchronous spacing math (returns the delay to wait and
//! claims the slot); `acquire()` sleeps that delay. Splitting them makes the
//! policy deterministic to test and keeps the `std::sync::Mutex` guard out of
//! any `await`.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::application::providers::config::interval_for;

#[derive(Debug)]
struct State {
    next_start: Instant,
}

/// Serializes request starts for one provider.
#[derive(Debug)]
pub struct RateLimiter {
    interval: Duration,
    state: Mutex<State>,
}

impl RateLimiter {
    pub fn new(requests_per_sec: f64) -> Self {
        let interval = interval_for(requests_per_sec);
        let now = Instant::now();
        Self {
            interval,
            state: Mutex::new(State { next_start: now }),
        }
    }

    /// Claim the next request-start slot and return how long to wait before
    /// it. Thread-safe: concurrent callers get distinct, spaced slots. A
    /// `ZERO` interval (disabled limit) always returns `ZERO` immediately.
    pub fn reserve(&self) -> Duration {
        if self.interval.is_zero() {
            return Duration::ZERO;
        }
        let mut state = self.state.lock().expect("rate limiter state poisoned");
        let now = Instant::now();
        let start = now.max(state.next_start);
        state.next_start = start + self.interval;
        start.saturating_duration_since(now)
    }

    /// Wait until this provider is allowed to start another request.
    pub async fn acquire(&self) {
        let delay = self.reserve();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// Deterministically poll a future to `Ready`/`Pending` without waiting on
    /// the scheduler, so time-driver tests need no spawned-task races.
    fn poll_once<F: Future>(fut: Pin<&mut F>) -> Poll<F::Output> {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        fut.poll(&mut cx)
    }

    #[tokio::test]
    async fn requests_are_spaced_by_the_interval() {
        let limiter = RateLimiter::new(2.0); // 500ms spacing
        assert_eq!(limiter.reserve(), Duration::ZERO, "first request immediate");
        let d2 = limiter.reserve();
        let d3 = limiter.reserve();
        // Real clock moves a little between calls, so allow small slack.
        assert!(
            (Duration::from_millis(495)..=Duration::from_millis(500)).contains(&d2),
            "second waits ~500ms, got {d2:?}"
        );
        assert!(
            (Duration::from_millis(995)..=Duration::from_millis(1000)).contains(&d3),
            "third waits ~1000ms, got {d3:?}"
        );
        assert!(d3 > d2, "slots strictly grow");
    }

    #[tokio::test]
    async fn reserved_slots_are_claimed_under_concurrency() {
        let limiter = Arc::new(RateLimiter::new(2.0));
        limiter.reserve(); // slot 0
        let a = tokio::spawn({
            let limiter = limiter.clone();
            async move { limiter.reserve() }
        });
        let b = tokio::spawn({
            let limiter = limiter.clone();
            async move { limiter.reserve() }
        });
        let (a, b) = tokio::join!(a, b);
        let (a, b) = (a.unwrap(), b.unwrap());
        let mut delays = [a, b];
        delays.sort_unstable();
        assert!(
            (Duration::from_millis(495)..=Duration::from_millis(500)).contains(&delays[0]),
            "first concurrent slot ~500ms, got {:?}",
            delays[0]
        );
        assert!(
            (Duration::from_millis(995)..=Duration::from_millis(1000)).contains(&delays[1]),
            "second concurrent slot ~1000ms, got {:?}",
            delays[1]
        );
    }

    #[tokio::test]
    async fn disabled_limit_skips_waiting() {
        let limiter = RateLimiter::new(0.0);
        assert_eq!(limiter.reserve(), Duration::ZERO);
        assert_eq!(limiter.reserve(), Duration::ZERO);
        assert_eq!(limiter.reserve(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn acquire_sleeps_the_reserved_delay() {
        let limiter = RateLimiter::new(2.0);
        let t0 = Instant::now();
        limiter.acquire().await;
        assert_eq!(t0.elapsed(), Duration::ZERO, "first is immediate");

        let mut pending = Box::pin(limiter.acquire());
        assert!(
            poll_once(pending.as_mut()).is_pending(),
            "second acquire registered a sleep"
        );
        tokio::time::advance(Duration::from_millis(499)).await;
        assert!(
            poll_once(pending.as_mut()).is_pending(),
            "still waiting before 500ms"
        );
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(
            poll_once(pending.as_mut()).is_ready(),
            "second acquire fired at +500ms"
        );
    }
}
