//! Snowflake ID generation with backpressure.
//!
//! Produces monotonically increasing 63-bit IDs (positive i64) from a
//! 47-bit millisecond timestamp and 16-bit sequence counter. The 47-bit
//! mask keeps values positive until ~2039.
//!
//! Under contention, spins briefly then sleeps 1ms (backpressure instead
//! of busy-wait).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static STATE: AtomicU64 = AtomicU64::new(0);

/// Mask timestamp to 47 bits to keep the final i64 positive (sign bit clear) until ~2039.
const TIMESTAMP_MASK: u64 = (1 << 47) - 1;

/// Generate a unique, monotonically increasing snowflake ID.
///
/// Thread-safe via atomic CAS. Under high contention (>65535 IDs per
/// millisecond), applies backpressure by sleeping 1ms after 1000 spins.
pub fn generate_id() -> i64 {
    let mut retries = 0u32;
    loop {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64
            & TIMESTAMP_MASK;

        let prev = STATE.load(Ordering::Acquire);
        let prev_ms = prev >> 16;
        let prev_seq = prev & 0xFFFF;

        let (ms, seq) = if now_ms == prev_ms {
            if prev_seq >= 0xFFFF {
                retries += 1;
                if retries >= 1000 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    retries = 0;
                } else {
                    std::hint::spin_loop();
                }
                continue;
            }
            (now_ms, prev_seq + 1)
        } else {
            (now_ms, 0)
        };

        let new_state = (ms << 16) | seq;
        if STATE
            .compare_exchange(prev, new_state, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return ((ms << 16) | seq) as i64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonic() {
        let mut prev = 0i64;
        for _ in 0..1000 {
            let id = generate_id();
            assert!(id > prev, "ID {id} not greater than {prev}");
            prev = id;
        }
    }

    #[test]
    fn ids_are_positive() {
        for _ in 0..100 {
            assert!(generate_id() > 0);
        }
    }
}
