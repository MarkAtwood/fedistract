//! Exponential backoff schedule for ActivityPub delivery retries.

/// Maximum number of delivery attempts before giving up.
pub const MAX_ATTEMPTS: i32 = 6;

/// Returns the delay in milliseconds before the next retry attempt.
///
/// Schedule: 1m → 5m → 30m → 2h → 8h → 24h (repeats).
pub fn delay_ms(attempt: i32) -> i64 {
    match attempt {
        0 => 60_000,      // 1 minute
        1 => 300_000,     // 5 minutes
        2 => 1_800_000,   // 30 minutes
        3 => 7_200_000,   // 2 hours
        4 => 28_800_000,  // 8 hours
        _ => 86_400_000,  // 24 hours
    }
}

/// Returns `true` if the attempt count has exceeded [`MAX_ATTEMPTS`].
pub fn exhausted(attempt: i32) -> bool {
    attempt >= MAX_ATTEMPTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_increases() {
        let mut prev = 0;
        for attempt in 0..MAX_ATTEMPTS {
            let d = delay_ms(attempt);
            assert!(d > prev, "attempt {attempt}: {d} <= {prev}");
            prev = d;
        }
    }

    #[test]
    fn past_max_returns_24h() {
        assert_eq!(delay_ms(5), 86_400_000);
        assert_eq!(delay_ms(100), 86_400_000);
    }

    #[test]
    fn exhausted_at_max() {
        assert!(!exhausted(0));
        assert!(!exhausted(MAX_ATTEMPTS - 1));
        assert!(exhausted(MAX_ATTEMPTS));
    }
}
