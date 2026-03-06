use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Configuration for a [`CircuitBreaker`].
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
	/// Number of consecutive failures before opening the circuit. Default 5.
	pub failure_threshold: u32,
	/// Time in ms to wait before transitioning from Open to HalfOpen. Default 30,000.
	pub reset_timeout_ms: u64,
	/// Max attempts allowed in HalfOpen state before re-opening. Default 1.
	pub half_open_max_attempts: u32,
}

impl Default for CircuitBreakerConfig {
	fn default() -> Self {
		Self {
			failure_threshold: 5,
			reset_timeout_ms: 30_000,
			half_open_max_attempts: 1,
		}
	}
}

/// The three states of the circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
	/// Normal operation — all requests pass through.
	Closed,
	/// Tripped — requests are rejected. Stores the instant the circuit opened.
	Open(Instant),
	/// Probing — a limited number of test requests are allowed through.
	HalfOpen,
}

/// A circuit breaker that prevents cascading failures.
///
/// Thread-safe: all mutation goes through a `Mutex<CircuitState>` plus atomics.
pub struct CircuitBreaker {
	state: Mutex<CircuitState>,
	config: CircuitBreakerConfig,
	failure_count: AtomicU32,
	half_open_attempts: AtomicU32,
}

impl CircuitBreaker {
	/// Create a new circuit breaker with the given configuration.
	pub fn new(config: CircuitBreakerConfig) -> Self {
		Self {
			state: Mutex::new(CircuitState::Closed),
			config,
			failure_count: AtomicU32::new(0),
			half_open_attempts: AtomicU32::new(0),
		}
	}

	/// Check whether a request should be allowed through.
	///
	/// - **Closed** — always `true`.
	/// - **Open** — `true` only if the reset timeout has elapsed (transitions
	///   to HalfOpen). Otherwise `false`.
	/// - **HalfOpen** — `true` only if fewer than `half_open_max_attempts`
	///   have been issued.
	pub fn allow_request(&self) -> bool {
		let mut state = self.state.lock().unwrap();

		match &*state {
			CircuitState::Closed => true,
			CircuitState::Open(opened_at) => {
				let elapsed = opened_at.elapsed();
				if elapsed >= Duration::from_millis(self.config.reset_timeout_ms) {
					*state = CircuitState::HalfOpen;
					self.half_open_attempts.store(0, Ordering::SeqCst);
					self.half_open_attempts.fetch_add(1, Ordering::SeqCst);
					true
				} else {
					false
				}
			}
			CircuitState::HalfOpen => {
				let current = self.half_open_attempts.fetch_add(1, Ordering::SeqCst);
				current < self.config.half_open_max_attempts
			}
		}
	}

	/// Record a successful request. Resets failure count and transitions to Closed.
	pub fn record_success(&self) {
		let mut state = self.state.lock().unwrap();
		self.failure_count.store(0, Ordering::SeqCst);
		self.half_open_attempts.store(0, Ordering::SeqCst);
		*state = CircuitState::Closed;
	}

	/// Record a failed request. Increments failure count; if the threshold is
	/// reached the circuit transitions to Open.
	pub fn record_failure(&self) {
		let mut state = self.state.lock().unwrap();
		let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

		match &*state {
			CircuitState::HalfOpen => {
				*state = CircuitState::Open(Instant::now());
			}
			CircuitState::Closed => {
				if failures >= self.config.failure_threshold {
					*state = CircuitState::Open(Instant::now());
				}
			}
			CircuitState::Open(_) => {}
		}
	}

	/// Return the current circuit state.
	///
	/// Note: this performs a lazy check — if the circuit is Open and the reset
	/// timeout has elapsed, it reports HalfOpen (but does not mutate).
	pub fn state(&self) -> CircuitState {
		let state = self.state.lock().unwrap();
		match &*state {
			CircuitState::Open(opened_at) => {
				if opened_at.elapsed() >= Duration::from_millis(self.config.reset_timeout_ms) {
					CircuitState::HalfOpen
				} else {
					state.clone()
				}
			}
			other => other.clone(),
		}
	}

	/// Return the current failure count.
	pub fn failure_count(&self) -> u32 {
		self.failure_count.load(Ordering::SeqCst)
	}

	/// Reset the circuit breaker to Closed with zero failures.
	pub fn reset(&self) {
		let mut state = self.state.lock().unwrap();
		*state = CircuitState::Closed;
		self.failure_count.store(0, Ordering::SeqCst);
		self.half_open_attempts.store(0, Ordering::SeqCst);
	}
}

impl Default for CircuitBreaker {
	fn default() -> Self {
		Self::new(CircuitBreakerConfig::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn starts_closed_and_allows_requests() {
		let cb = CircuitBreaker::default();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert!(cb.allow_request());
	}

	#[test]
	fn opens_after_failure_threshold() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 3,
			..Default::default()
		});

		cb.record_failure();
		cb.record_failure();
		assert_eq!(cb.state(), CircuitState::Closed);

		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));
	}

	#[test]
	fn open_state_rejects_requests() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000,
			..Default::default()
		});

		cb.record_failure();
		assert!(!cb.allow_request());
	}

	#[test]
	fn transitions_to_half_open_after_timeout() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 0,
			..Default::default()
		});

		cb.record_failure();
		assert_eq!(cb.state(), CircuitState::HalfOpen);
		assert!(cb.allow_request());
	}

	#[test]
	fn half_open_success_closes() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 0,
			half_open_max_attempts: 1,
		});

		cb.record_failure();
		assert!(cb.allow_request());
		cb.record_success();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert_eq!(cb.failure_count(), 0);
	}

	#[test]
	fn half_open_failure_reopens() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000,
			half_open_max_attempts: 1,
		});

		cb.record_failure();
		{
			let mut state = cb.state.lock().unwrap();
			*state = CircuitState::HalfOpen;
			cb.half_open_attempts.store(0, Ordering::SeqCst);
		}

		assert!(cb.allow_request());
		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));
	}

	#[test]
	fn reset_returns_to_closed() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000,
			..Default::default()
		});

		cb.record_failure();
		cb.reset();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert_eq!(cb.failure_count(), 0);
	}

	#[test]
	fn default_config_has_sensible_values() {
		let cfg = CircuitBreakerConfig::default();
		assert_eq!(cfg.failure_threshold, 5);
		assert_eq!(cfg.reset_timeout_ms, 30_000);
		assert_eq!(cfg.half_open_max_attempts, 1);
	}
}
