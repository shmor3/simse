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
pub struct Backoff {
    config: BackoffConfig,
    attempt: u32,
}

impl Backoff {
    pub fn new(config: BackoffConfig) -> Self {
        Self { config, attempt: 0 }
    }

    /// Get the next backoff duration and increment the attempt counter.
    pub fn next_delay(&mut self) -> Duration {
        let delay_ms = (self.config.initial_ms as f64
            * self.config.multiplier.powi(self.attempt as i32))
            as u64;
        let clamped = delay_ms.min(self.config.max_ms);
        self.attempt += 1;
        Duration::from_millis(clamped)
    }

    /// Reset on successful connection.
    pub fn reset(&mut self) {
        self.attempt = 0;
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
        let mut b = Backoff::new(BackoffConfig {
            initial_ms: 1_000,
            max_ms: 30_000,
            multiplier: 2.0,
        });
        assert_eq!(b.next_delay(), Duration::from_millis(1_000));
        assert_eq!(b.next_delay(), Duration::from_millis(2_000));
        assert_eq!(b.next_delay(), Duration::from_millis(4_000));
        assert_eq!(b.next_delay(), Duration::from_millis(8_000));
        assert_eq!(b.next_delay(), Duration::from_millis(16_000));
        assert_eq!(b.next_delay(), Duration::from_millis(30_000)); // clamped
        assert_eq!(b.next_delay(), Duration::from_millis(30_000)); // still clamped
    }

    #[test]
    fn backoff_reset() {
        let mut b = Backoff::new(BackoffConfig::default());
        b.next_delay();
        b.next_delay();
        assert_eq!(b.attempts(), 2);
        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Duration::from_millis(1_000));
    }
}
