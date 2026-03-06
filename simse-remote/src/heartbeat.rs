use std::time::Duration;

/// Exponential backoff configuration for reconnection.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    pub initial_ms: u64,
    pub max_ms: u64,
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 1_000,
            max_ms: 30_000,
            multiplier: 2.0,
        }
    }
}

/// Tracks reconnection attempts with exponential backoff.
///
/// Uses owned-return pattern: `next_delay` and `reset` consume self and
/// return the updated `Backoff`.
#[derive(Clone)]
pub struct Backoff {
    config: BackoffConfig,
    attempt: u32,
}

impl Backoff {
    pub fn new(config: BackoffConfig) -> Self {
        Self { config, attempt: 0 }
    }

    /// Get the next backoff duration and return updated state.
    pub fn next_delay(self) -> (Self, Duration) {
        let delay_ms = (self.config.initial_ms as f64
            * self.config.multiplier.powi(self.attempt as i32))
            as u64;
        let clamped = delay_ms.min(self.config.max_ms);
        let new = Self {
            config: self.config,
            attempt: self.attempt + 1,
        };
        (new, Duration::from_millis(clamped))
    }

    /// Reset on successful connection. Returns new state with attempt = 0.
    pub fn reset(self) -> Self {
        Self {
            config: self.config,
            attempt: 0,
        }
    }

    /// Current attempt count.
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

/// Keepalive ping interval.
pub const PING_INTERVAL_MS: u64 = 30_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_exponential() {
        let b = Backoff::new(BackoffConfig {
            initial_ms: 1_000,
            max_ms: 30_000,
            multiplier: 2.0,
        });
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(1_000));
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(2_000));
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(4_000));
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(8_000));
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(16_000));
        let (b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(30_000)); // clamped
        let (_b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(30_000)); // still clamped
    }

    #[test]
    fn backoff_reset() {
        let b = Backoff::new(BackoffConfig::default());
        let (b, _) = b.next_delay();
        let (b, _) = b.next_delay();
        assert_eq!(b.attempts(), 2);
        let b = b.reset();
        assert_eq!(b.attempts(), 0);
        let (_b, d) = b.next_delay();
        assert_eq!(d, Duration::from_millis(1_000));
    }
}
