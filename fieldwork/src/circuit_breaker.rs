//! Per-domain circuit breaker for ActivityPub delivery.
//!
//! Tracks consecutive failures per domain and pauses delivery after a threshold
//! is reached. Includes bounded eviction to prevent unbounded memory growth.

use std::collections::HashMap;
use std::sync::Mutex;

const THRESHOLD: u32 = 10;
const OPEN_DURATION_MS: i64 = 3_600_000; // 1 hour
const MAX_ENTRIES: usize = 10_000;

struct CircuitState {
    consecutive_failures: u32,
    /// Unix timestamp (ms) until which the circuit is open; `None` = closed.
    open_until: Option<i64>,
}

/// Per-domain circuit breaker.
///
/// After [`THRESHOLD`] consecutive failures, the circuit opens for 1 hour.
/// A single success resets the circuit for that domain.
///
/// # Example
///
/// ```
/// use fieldwork::circuit_breaker::CircuitBreaker;
///
/// let cb = CircuitBreaker::new();
/// let now = 1_000_000_000_000i64;
///
/// assert!(!cb.is_open("example.com", now));
/// for _ in 0..10 {
///     cb.record_failure("example.com", now);
/// }
/// assert!(cb.is_open("example.com", now));
/// ```
pub struct CircuitBreaker {
    map: Mutex<HashMap<String, CircuitState>>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if deliveries to `domain` are currently paused.
    pub fn is_open(&self, domain: &str, now_ms: i64) -> bool {
        let map = self.map.lock().unwrap();
        match map.get(domain) {
            Some(state) => matches!(state.open_until, Some(until) if now_ms < until),
            None => false,
        }
    }

    /// Record a successful delivery, resetting the circuit for this domain.
    pub fn record_success(&self, domain: &str) {
        let mut map = self.map.lock().unwrap();
        map.remove(domain);
    }

    /// Record a failed delivery. Opens the circuit after [`THRESHOLD`] consecutive failures.
    pub fn record_failure(&self, domain: &str, now_ms: i64) {
        let mut map = self.map.lock().unwrap();

        // ponytail: prevent unbounded growth; evict expired entries when map is large
        if map.len() > MAX_ENTRIES {
            map.retain(|_, state| {
                matches!(state.open_until, Some(until) if now_ms < until)
                    || state.consecutive_failures > 0
            });
            if map.len() > MAX_ENTRIES {
                map.retain(|_, state| {
                    matches!(state.open_until, Some(until) if now_ms < until)
                });
            }
        }

        let state = map.entry(domain.to_owned()).or_insert(CircuitState {
            consecutive_failures: 0,
            open_until: None,
        });
        state.consecutive_failures += 1;
        if state.consecutive_failures >= THRESHOLD {
            state.open_until = Some(now_ms + OPEN_DURATION_MS);
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn initially_closed() {
        let cb = CircuitBreaker::new();
        assert!(!cb.is_open("example.com", NOW));
    }

    #[test]
    fn opens_after_threshold() {
        let cb = CircuitBreaker::new();
        for _ in 0..THRESHOLD - 1 {
            cb.record_failure("example.com", NOW);
            assert!(!cb.is_open("example.com", NOW));
        }
        cb.record_failure("example.com", NOW);
        assert!(cb.is_open("example.com", NOW));
    }

    #[test]
    fn closes_after_duration() {
        let cb = CircuitBreaker::new();
        for _ in 0..THRESHOLD {
            cb.record_failure("example.com", NOW);
        }
        assert!(cb.is_open("example.com", NOW));
        assert!(!cb.is_open("example.com", NOW + OPEN_DURATION_MS));
    }

    #[test]
    fn success_resets() {
        let cb = CircuitBreaker::new();
        for _ in 0..THRESHOLD {
            cb.record_failure("example.com", NOW);
        }
        assert!(cb.is_open("example.com", NOW));
        cb.record_success("example.com");
        assert!(!cb.is_open("example.com", NOW));
    }

    #[test]
    fn independent_domains() {
        let cb = CircuitBreaker::new();
        for _ in 0..THRESHOLD {
            cb.record_failure("bad.com", NOW);
        }
        assert!(cb.is_open("bad.com", NOW));
        assert!(!cb.is_open("good.com", NOW));
    }
}
