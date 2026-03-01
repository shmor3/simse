// ---------------------------------------------------------------------------
// Resilience — circuit breaker, health monitor, retry with exponential backoff
// ---------------------------------------------------------------------------
//
// Three components for fault tolerance:
//   1. CircuitBreaker — prevents cascading failures by short-circuiting
//      requests to unhealthy services (Closed → Open → HalfOpen → Closed).
//   2. HealthMonitor — sliding window tracking of request success/failure
//      with Healthy/Degraded/Unhealthy status.
//   3. retry() — async retry with exponential backoff + jitter, only retrying
//      transient errors.
// ---------------------------------------------------------------------------

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::AcpError;

// ===========================================================================
// Circuit Breaker
// ===========================================================================

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
					// Transition to HalfOpen
					*state = CircuitState::HalfOpen;
					self.half_open_attempts.store(0, Ordering::SeqCst);
					// Allow the first probe request
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
				// Any failure in HalfOpen → re-open immediately
				*state = CircuitState::Open(Instant::now());
			}
			CircuitState::Closed => {
				if failures >= self.config.failure_threshold {
					*state = CircuitState::Open(Instant::now());
				}
			}
			CircuitState::Open(_) => {
				// Already open — nothing to do
			}
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

// ===========================================================================
// Health Monitor
// ===========================================================================

/// Overall service health assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
	/// Service is operating normally.
	Healthy,
	/// 3+ consecutive failures — service is showing signs of trouble.
	Degraded,
	/// 5+ consecutive failures — service should be considered down.
	Unhealthy,
}

/// Point-in-time snapshot of health statistics.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
	/// Number of failures in a row without any intervening success.
	pub consecutive_failures: u32,
	/// Total calls recorded since creation/reset.
	pub total_calls: u64,
	/// Total failures recorded since creation/reset.
	pub total_failures: u64,
	/// Failure rate within the sliding window (0.0 – 1.0).
	pub window_failure_rate: f64,
}

/// Sliding-window health monitor that tracks request outcomes.
pub struct HealthMonitor {
	consecutive_failures: AtomicU32,
	total_calls: AtomicU64,
	total_failures: AtomicU64,
	/// Sliding window of `(timestamp, success)` pairs.
	window: Mutex<VecDeque<(Instant, bool)>>,
	window_duration: Duration,
}

impl HealthMonitor {
	/// Create a new health monitor with the given sliding-window duration.
	pub fn new(window_duration: Duration) -> Self {
		Self {
			consecutive_failures: AtomicU32::new(0),
			total_calls: AtomicU64::new(0),
			total_failures: AtomicU64::new(0),
			window: Mutex::new(VecDeque::new()),
			window_duration,
		}
	}

	/// Record a successful request. Resets consecutive failure counter.
	pub fn record_success(&self) {
		self.consecutive_failures.store(0, Ordering::SeqCst);
		self.total_calls.fetch_add(1, Ordering::SeqCst);

		let mut window = self.window.lock().unwrap();
		window.push_back((Instant::now(), true));
	}

	/// Record a failed request.
	pub fn record_failure(&self, _error: &str) {
		self.consecutive_failures.fetch_add(1, Ordering::SeqCst);
		self.total_calls.fetch_add(1, Ordering::SeqCst);
		self.total_failures.fetch_add(1, Ordering::SeqCst);

		let mut window = self.window.lock().unwrap();
		window.push_back((Instant::now(), false));
	}

	/// Return a snapshot of the current health statistics.
	///
	/// Old entries outside the sliding window are pruned before computing the
	/// failure rate.
	pub fn snapshot(&self) -> HealthSnapshot {
		let mut window = self.window.lock().unwrap();
		let cutoff = Instant::now() - self.window_duration;

		// Prune expired entries
		while let Some(&(ts, _)) = window.front() {
			if ts < cutoff {
				window.pop_front();
			} else {
				break;
			}
		}

		let window_total = window.len() as f64;
		let window_failures = window.iter().filter(|(_, success)| !success).count() as f64;
		let window_failure_rate = if window_total > 0.0 {
			window_failures / window_total
		} else {
			0.0
		};

		HealthSnapshot {
			consecutive_failures: self.consecutive_failures.load(Ordering::SeqCst),
			total_calls: self.total_calls.load(Ordering::SeqCst),
			total_failures: self.total_failures.load(Ordering::SeqCst),
			window_failure_rate,
		}
	}

	/// Compute the current health status based on consecutive failures.
	///
	/// - `Healthy`   — fewer than 3 consecutive failures.
	/// - `Degraded`  — 3–4 consecutive failures.
	/// - `Unhealthy` — 5+ consecutive failures.
	pub fn status(&self) -> HealthStatus {
		let failures = self.consecutive_failures.load(Ordering::SeqCst);
		if failures >= 5 {
			HealthStatus::Unhealthy
		} else if failures >= 3 {
			HealthStatus::Degraded
		} else {
			HealthStatus::Healthy
		}
	}

	/// Reset all counters and clear the window.
	pub fn reset(&self) {
		self.consecutive_failures.store(0, Ordering::SeqCst);
		self.total_calls.store(0, Ordering::SeqCst);
		self.total_failures.store(0, Ordering::SeqCst);
		let mut window = self.window.lock().unwrap();
		window.clear();
	}
}

impl Default for HealthMonitor {
	fn default() -> Self {
		Self::new(Duration::from_secs(60))
	}
}

// ===========================================================================
// Retry with Exponential Backoff
// ===========================================================================

/// Configuration for the [`retry`] function.
#[derive(Debug, Clone)]
pub struct RetryConfig {
	/// Maximum number of attempts (including the first). Default 3.
	pub max_attempts: u32,
	/// Base delay in milliseconds before the first retry. Default 500.
	pub base_delay_ms: u64,
	/// Maximum delay cap in milliseconds. Default 15,000.
	pub max_delay_ms: u64,
	/// Multiplier applied to the delay after each retry. Default 2.0.
	pub backoff_multiplier: f64,
	/// Jitter factor (0.0 – 1.0). Adds deterministic jitter. Default 0.25.
	pub jitter_factor: f64,
}

impl Default for RetryConfig {
	fn default() -> Self {
		Self {
			max_attempts: 3,
			base_delay_ms: 500,
			max_delay_ms: 15_000,
			backoff_multiplier: 2.0,
			jitter_factor: 0.25,
		}
	}
}

/// Returns `true` if the error is likely transient and worth retrying.
///
/// Transient errors:
/// - `Timeout`
/// - `ServerUnavailable`
/// - `ConnectionFailed`
/// - `Io`
///
/// Non-transient (never retry):
/// - `NotInitialized`
/// - `PermissionDenied`
/// - `SessionError`
/// - `ProtocolError`
/// - `Serialization`
/// - `CircuitBreakerOpen`
/// - `StreamError`
pub fn is_transient(error: &AcpError) -> bool {
	matches!(
		error,
		AcpError::Timeout { .. }
			| AcpError::ServerUnavailable(_)
			| AcpError::ConnectionFailed(_)
			| AcpError::Io(_)
	)
}

/// Execute an async closure with automatic retries and exponential backoff.
///
/// Only transient errors (as determined by [`is_transient`]) trigger a retry.
/// Non-transient errors propagate immediately.
///
/// # Delay calculation
///
/// ```text
/// base_delay = min(base_delay_ms * multiplier^(attempt-1), max_delay_ms)
/// jitter     = base_delay * jitter_factor * deterministic_fraction
/// final      = base_delay + jitter
/// ```
///
/// The deterministic jitter uses a simple hash of the attempt number to avoid
/// requiring an external RNG crate.
pub async fn retry<T, F, Fut>(config: &RetryConfig, f: F) -> Result<T, AcpError>
where
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, AcpError>>,
{
	let max_attempts = config.max_attempts.max(1);
	let mut last_error: Option<AcpError> = None;

	for attempt in 1..=max_attempts {
		match f().await {
			Ok(value) => return Ok(value),
			Err(err) => {
				// Don't retry on the last attempt
				if attempt >= max_attempts {
					last_error = Some(err);
					break;
				}

				// Non-transient errors propagate immediately
				if !is_transient(&err) {
					return Err(err);
				}

				last_error = Some(err);

				// Calculate delay with exponential backoff + deterministic jitter
				let exponential =
					config.base_delay_ms as f64 * config.backoff_multiplier.powi(attempt as i32 - 1);
				let capped = exponential.min(config.max_delay_ms as f64);

				// Deterministic jitter: use a simple hash of the attempt number
				// to produce a fraction in [0, 1). This avoids needing the `rand` crate.
				let jitter_frac = deterministic_jitter_fraction(attempt);
				let jitter = capped * config.jitter_factor * jitter_frac;

				let delay_ms = (capped + jitter).max(0.0).round() as u64;
				tokio::time::sleep(Duration::from_millis(delay_ms)).await;
			}
		}
	}

	// All attempts exhausted
	Err(last_error.unwrap_or_else(|| {
		AcpError::ConnectionFailed("retry exhausted with no error captured".into())
	}))
}

/// Produce a deterministic fraction in [0.0, 1.0) from an attempt number.
///
/// Uses a simple multiplicative hash (Knuth's golden ratio method) to spread
/// values without requiring a full RNG.
fn deterministic_jitter_fraction(attempt: u32) -> f64 {
	// Knuth's multiplicative hash
	let hash = (attempt as u64).wrapping_mul(2_654_435_761);
	(hash % 1_000_000) as f64 / 1_000_000.0
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::AtomicU32 as TestAtomicU32;
	use std::sync::Arc;

	// -----------------------------------------------------------------------
	// CircuitBreaker tests
	// -----------------------------------------------------------------------

	#[test]
	fn circuit_breaker_starts_closed_and_allows_requests() {
		let cb = CircuitBreaker::default();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert!(cb.allow_request());
	}

	#[test]
	fn circuit_breaker_opens_after_failure_threshold() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 3,
			..Default::default()
		});

		// First two failures: still closed
		cb.record_failure();
		cb.record_failure();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert!(cb.allow_request());

		// Third failure: opens the circuit
		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));
	}

	#[test]
	fn circuit_breaker_open_state_rejects_requests() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000, // Long timeout so it stays open
			..Default::default()
		});

		cb.record_failure();
		assert!(!cb.allow_request());
	}

	#[test]
	fn circuit_breaker_transitions_to_half_open_after_timeout() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 0, // Instant timeout
			..Default::default()
		});

		cb.record_failure();
		// With 0ms timeout, it should immediately be eligible for HalfOpen
		assert_eq!(cb.state(), CircuitState::HalfOpen);
		assert!(cb.allow_request());
	}

	#[test]
	fn circuit_breaker_half_open_success_transitions_to_closed() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 0,
			half_open_max_attempts: 1,
		});

		cb.record_failure();
		// Transition to HalfOpen via allow_request
		assert!(cb.allow_request());
		// Record success while in HalfOpen
		cb.record_success();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert_eq!(cb.failure_count(), 0);
	}

	#[test]
	fn circuit_breaker_half_open_failure_transitions_to_open() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000, // Long timeout so Open is observable
			half_open_max_attempts: 1,
		});

		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));

		// Manually transition to HalfOpen by forcing the internal state.
		{
			let mut state = cb.state.lock().unwrap();
			*state = CircuitState::HalfOpen;
			cb.half_open_attempts.store(0, Ordering::SeqCst);
		}
		assert_eq!(cb.state(), CircuitState::HalfOpen);

		// allow the probe request
		assert!(cb.allow_request());

		// Record failure while in HalfOpen — should re-open
		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));
		// Verify requests are now rejected (long timeout)
		assert!(!cb.allow_request());
	}

	#[test]
	fn circuit_breaker_reset_returns_to_closed() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 1,
			reset_timeout_ms: 60_000,
			..Default::default()
		});

		cb.record_failure();
		assert!(matches!(cb.state(), CircuitState::Open(_)));

		cb.reset();
		assert_eq!(cb.state(), CircuitState::Closed);
		assert_eq!(cb.failure_count(), 0);
	}

	// -----------------------------------------------------------------------
	// HealthMonitor tests
	// -----------------------------------------------------------------------

	#[test]
	fn health_monitor_starts_healthy() {
		let hm = HealthMonitor::default();
		assert_eq!(hm.status(), HealthStatus::Healthy);
		let snap = hm.snapshot();
		assert_eq!(snap.consecutive_failures, 0);
		assert_eq!(snap.total_calls, 0);
	}

	#[test]
	fn health_monitor_degraded_after_3_failures() {
		let hm = HealthMonitor::default();
		hm.record_failure("err1");
		hm.record_failure("err2");
		assert_eq!(hm.status(), HealthStatus::Healthy);

		hm.record_failure("err3");
		assert_eq!(hm.status(), HealthStatus::Degraded);
	}

	#[test]
	fn health_monitor_unhealthy_after_5_failures() {
		let hm = HealthMonitor::default();
		for i in 0..5 {
			hm.record_failure(&format!("err{i}"));
		}
		assert_eq!(hm.status(), HealthStatus::Unhealthy);
	}

	#[test]
	fn health_monitor_success_resets_consecutive_failures() {
		let hm = HealthMonitor::default();
		hm.record_failure("err1");
		hm.record_failure("err2");
		hm.record_failure("err3");
		assert_eq!(hm.status(), HealthStatus::Degraded);

		hm.record_success();
		assert_eq!(hm.status(), HealthStatus::Healthy);
		assert_eq!(hm.snapshot().consecutive_failures, 0);
	}

	#[test]
	fn health_monitor_window_prunes_old_entries() {
		// Use a very short window so entries expire immediately
		let hm = HealthMonitor::new(Duration::from_millis(0));

		hm.record_failure("err1");
		hm.record_failure("err2");

		// After pruning, the window should be empty → 0% failure rate
		let snap = hm.snapshot();
		assert_eq!(snap.window_failure_rate, 0.0);
		// But total counters still reflect the calls
		assert_eq!(snap.total_calls, 2);
		assert_eq!(snap.total_failures, 2);
	}

	#[test]
	fn health_monitor_snapshot_computes_failure_rate() {
		let hm = HealthMonitor::new(Duration::from_secs(60));

		hm.record_success();
		hm.record_failure("err");
		hm.record_success();
		hm.record_failure("err");

		let snap = hm.snapshot();
		assert_eq!(snap.total_calls, 4);
		assert_eq!(snap.total_failures, 2);
		assert!((snap.window_failure_rate - 0.5).abs() < f64::EPSILON);
	}

	#[test]
	fn health_monitor_reset_clears_everything() {
		let hm = HealthMonitor::default();
		hm.record_failure("err");
		hm.record_failure("err");
		hm.record_failure("err");
		assert_eq!(hm.status(), HealthStatus::Degraded);

		hm.reset();
		assert_eq!(hm.status(), HealthStatus::Healthy);
		let snap = hm.snapshot();
		assert_eq!(snap.total_calls, 0);
		assert_eq!(snap.total_failures, 0);
	}

	// -----------------------------------------------------------------------
	// Retry tests
	// -----------------------------------------------------------------------

	#[tokio::test]
	async fn retry_succeeds_on_first_attempt() {
		let config = RetryConfig::default();
		let call_count = Arc::new(TestAtomicU32::new(0));
		let cc = call_count.clone();

		let result = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Ok::<_, AcpError>(42)
			}
		})
		.await;

		assert_eq!(result.unwrap(), 42);
		assert_eq!(call_count.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn retry_retries_transient_error_and_succeeds() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1, // Very short for testing
			max_delay_ms: 10,
			..Default::default()
		};
		let call_count = Arc::new(TestAtomicU32::new(0));
		let cc = call_count.clone();

		let result = retry(&config, || {
			let cc = cc.clone();
			async move {
				let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
				if attempt < 2 {
					Err(AcpError::Timeout {
						method: "test".into(),
						timeout_ms: 1000,
					})
				} else {
					Ok(99)
				}
			}
		})
		.await;

		assert_eq!(result.unwrap(), 99);
		assert_eq!(call_count.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn retry_stops_after_max_attempts() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let call_count = Arc::new(TestAtomicU32::new(0));
		let cc = call_count.clone();

		let result: Result<i32, AcpError> = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(AcpError::ConnectionFailed("down".into()))
			}
		})
		.await;

		assert!(result.is_err());
		assert_eq!(call_count.load(Ordering::SeqCst), 3);
	}

	#[tokio::test]
	async fn retry_does_not_retry_non_transient_errors() {
		let config = RetryConfig {
			max_attempts: 5,
			base_delay_ms: 1,
			..Default::default()
		};
		let call_count = Arc::new(TestAtomicU32::new(0));
		let cc = call_count.clone();

		let result: Result<i32, AcpError> = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(AcpError::PermissionDenied("nope".into()))
			}
		})
		.await;

		assert!(result.is_err());
		// Should have been called exactly once — no retry for PermissionDenied
		assert_eq!(call_count.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn is_transient_classifies_errors_correctly() {
		// Transient errors
		assert!(is_transient(&AcpError::Timeout {
			method: "m".into(),
			timeout_ms: 100,
		}));
		assert!(is_transient(&AcpError::ServerUnavailable("x".into())));
		assert!(is_transient(&AcpError::ConnectionFailed("x".into())));
		assert!(is_transient(&AcpError::Io(std::io::Error::new(
			std::io::ErrorKind::Other,
			"io"
		))));

		// Non-transient errors
		assert!(!is_transient(&AcpError::NotInitialized));
		assert!(!is_transient(&AcpError::PermissionDenied("x".into())));
		assert!(!is_transient(&AcpError::SessionError("x".into())));
		assert!(!is_transient(&AcpError::ProtocolError("x".into())));
		assert!(!is_transient(&AcpError::Serialization("x".into())));
		assert!(!is_transient(&AcpError::CircuitBreakerOpen("x".into())));
		assert!(!is_transient(&AcpError::StreamError("x".into())));
	}

	// -----------------------------------------------------------------------
	// Default impls
	// -----------------------------------------------------------------------

	#[test]
	fn default_circuit_breaker_config_has_sensible_values() {
		let cfg = CircuitBreakerConfig::default();
		assert_eq!(cfg.failure_threshold, 5);
		assert_eq!(cfg.reset_timeout_ms, 30_000);
		assert_eq!(cfg.half_open_max_attempts, 1);
	}

	#[test]
	fn default_retry_config_has_sensible_values() {
		let cfg = RetryConfig::default();
		assert_eq!(cfg.max_attempts, 3);
		assert_eq!(cfg.base_delay_ms, 500);
		assert_eq!(cfg.max_delay_ms, 15_000);
		assert!((cfg.backoff_multiplier - 2.0).abs() < f64::EPSILON);
		assert!((cfg.jitter_factor - 0.25).abs() < f64::EPSILON);
	}

	#[test]
	fn default_health_monitor_uses_60s_window() {
		let hm = HealthMonitor::default();
		assert_eq!(hm.window_duration, Duration::from_secs(60));
	}

	#[test]
	fn deterministic_jitter_produces_valid_fraction() {
		for attempt in 1..=100 {
			let frac = deterministic_jitter_fraction(attempt);
			assert!(
				(0.0..1.0).contains(&frac),
				"attempt {attempt} produced {frac}"
			);
		}
	}
}
