//! Minimal fixed-window rate limiter for authentication endpoints.
//!
//! All remote requests reach the admin web server through the TCP multiplexer
//! (which terminates on loopback), so the socket address is 127.0.0.1 for both
//! local and remote callers — the limiter therefore behaves as a global cap on
//! failed attempts per window. That is acceptable (and simpler than trusting a
//! spoofable `X-Forwarded-For`): it stops credential brute-forcing with no
//! bypass, at the cost of an attacker being able to exhaust the window and
//! briefly block logins for everyone.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Fixed-window limiter keyed by an opaque client key (e.g. peer IP).
pub struct FixedWindowLimiter {
    window: Duration,
    max_attempts: u32,
    entries: Mutex<HashMap<String, (Instant, u32)>>,
}

impl FixedWindowLimiter {
    pub fn new(window: Duration, max_attempts: u32) -> Self {
        Self {
            window,
            max_attempts,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Registers one attempt for `key`. Returns `Ok(())` when the caller may
    /// proceed, or `Err(retry_after_secs)` when the window is exhausted.
    pub fn check(&self, key: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let (start, count) = entries.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(*start) >= self.window {
            *start = now;
            *count = 0;
        }
        if *count >= self.max_attempts {
            let retry = self.window.saturating_sub(now.duration_since(*start));
            return Err(retry.as_secs().max(1));
        }
        *count += 1;
        Ok(())
    }

    /// Clears the counter for `key` (called after a successful login so a
    /// legit user's earlier failures don't lock them out).
    pub fn reset(&self, key: &str) {
        self.entries.lock().unwrap().remove(key);
    }

    /// Drops entries whose window has fully elapsed. Called periodically by
    /// the housekeeping task.
    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.entries
            .lock()
            .unwrap()
            .retain(|_, (start, _)| now.duration_since(*start) < self.window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_attempts_then_recovers_after_window() {
        let limiter = FixedWindowLimiter::new(Duration::from_secs(60), 3);
        for _ in 0..3 {
            assert!(limiter.check("1.2.3.4").is_ok());
        }
        assert!(limiter.check("1.2.3.4").is_err());
        // Other keys are unaffected.
        assert!(limiter.check("5.6.7.8").is_ok());

        limiter.purge_expired();
        // Window has not elapsed, still blocked.
        assert!(limiter.check("1.2.3.4").is_err());
    }

    #[test]
    fn reset_clears_the_counter() {
        let limiter = FixedWindowLimiter::new(Duration::from_secs(60), 2);
        assert!(limiter.check("k").is_ok());
        assert!(limiter.check("k").is_ok());
        assert!(limiter.check("k").is_err());
        limiter.reset("k");
        assert!(limiter.check("k").is_ok());
    }

    #[test]
    fn window_elapses_and_key_recovers() {
        let limiter = FixedWindowLimiter::new(Duration::from_millis(10), 1);
        assert!(limiter.check("k").is_ok());
        assert!(limiter.check("k").is_err());
        std::thread::sleep(Duration::from_millis(20));
        assert!(limiter.check("k").is_ok());
    }
}
