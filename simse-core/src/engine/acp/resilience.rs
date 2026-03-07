// ---------------------------------------------------------------------------
// Resilience — circuit breaker, health monitor, retry
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::engine::acp::error::AcpError;

// ===========================================================================
// RetryConfig
// ===========================================================================

/// Configuration for exponential-backoff retry.
#[derive(Debug, Clone)]
pub struct RetryConfig {
	/// Maximum number of attempts (including the first). Default 3.
	pub max_attempts: u32,
	/// Base delay in milliseconds before the first retry. Default 1,000.
	pub base_delay_ms: u64,
	/// Maximum delay cap in milliseconds. Default 30,000.
	pub max_delay_ms: u64,
	/// Multiplier applied to the delay after each retry. Default 2.0.
	pub backoff_multiplier: f64,
	/// Jitter factor (0.0 - 1.0). Default 0.25.
	pub jitter_factor: f64,
}

impl Default for RetryConfig {
	fn default() -> Self {
		Self {
			max_attempts: 3,
			base_delay_ms: 1_000,
			max_delay_ms: 30_000,
			backoff_multiplier: 2.0,
			jitter_factor: 0.25,
		}
	}
}

// ===========================================================================
// CircuitBreaker
// ===========================================================================

/// Configuration for a circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
	/// Number of consecutive failures before opening the circuit.
	pub failure_threshold: u32,
	/// How long the circuit stays open before moving to half-open (ms).
	pub reset_timeout_ms: u64,
	/// Number of successes in half-open state to close the circuit.
	pub half_open_successes: u32,
}

impl Default for CircuitBreakerConfig {
	fn default() -> Self {
		Self {
			failure_threshold: 5,
			reset_timeout_ms: 30_000,
			half_open_successes: 2,
		}
	}
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
	Closed,
	Open,
	HalfOpen,
}

/// A simple circuit breaker that tracks consecutive failures and opens
/// when a threshold is exceeded.
///
/// Thread-safety: uses atomics for counters and a parking_lot-free design.
/// The state transitions are best-effort (no mutex) which is acceptable
/// for resilience heuristics.
pub struct CircuitBreaker {
	config: CircuitBreakerConfig,
	consecutive_failures: AtomicU64,
	consecutive_successes: AtomicU64,
	/// Timestamp (as millis since an arbitrary epoch) when the circuit opened.
	opened_at: std::sync::Mutex<Option<Instant>>,
}

impl std::fmt::Debug for CircuitBreaker {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CircuitBreaker")
			.field("config", &self.config)
			.field("consecutive_failures", &self.consecutive_failures.load(Ordering::Relaxed))
			.finish()
	}
}

impl CircuitBreaker {
	/// Create a new circuit breaker with the given configuration.
	pub fn new(config: CircuitBreakerConfig) -> Self {
		Self {
			config,
			consecutive_failures: AtomicU64::new(0),
			consecutive_successes: AtomicU64::new(0),
			opened_at: std::sync::Mutex::new(None),
		}
	}

	/// Returns `true` if the request should be allowed through.
	pub fn allow_request(&self) -> bool {
		match self.state() {
			CircuitState::Closed => true,
			CircuitState::HalfOpen => true,
			CircuitState::Open => false,
		}
	}

	/// Record a successful request.
	pub fn record_success(&self) {
		self.consecutive_failures.store(0, Ordering::Relaxed);
		let prev = self.consecutive_successes.fetch_add(1, Ordering::Relaxed);

		// If in half-open and enough successes, close the circuit.
		if self.state() == CircuitState::HalfOpen
			&& (prev + 1) >= self.config.half_open_successes as u64
		{
			*self.opened_at.lock().unwrap() = None;
			self.consecutive_successes.store(0, Ordering::Relaxed);
		}
	}

	/// Record a failed request.
	pub fn record_failure(&self) {
		self.consecutive_successes.store(0, Ordering::Relaxed);
		let prev = self.consecutive_failures.fetch_add(1, Ordering::Relaxed);

		// Open the circuit if threshold exceeded.
		if (prev + 1) >= self.config.failure_threshold as u64 {
			*self.opened_at.lock().unwrap() = Some(Instant::now());
		}
	}

	/// Current state of the circuit breaker.
	fn state(&self) -> CircuitState {
		let guard = self.opened_at.lock().unwrap();
		match *guard {
			None => CircuitState::Closed,
			Some(opened) => {
				let elapsed = opened.elapsed();
				if elapsed >= Duration::from_millis(self.config.reset_timeout_ms) {
					CircuitState::HalfOpen
				} else {
					CircuitState::Open
				}
			}
		}
	}
}

impl Default for CircuitBreaker {
	fn default() -> Self {
		Self::new(CircuitBreakerConfig::default())
	}
}

// ===========================================================================
// HealthMonitor
// ===========================================================================

/// Health status of a monitored endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
	Healthy,
	Degraded,
	Unhealthy,
}

/// A snapshot of health metrics.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
	pub status: HealthStatus,
	pub consecutive_failures: u64,
	pub total_successes: u64,
	pub total_failures: u64,
	pub last_error: Option<String>,
}

/// Tracks success/failure counts and provides health status.
pub struct HealthMonitor {
	consecutive_failures: AtomicU64,
	total_successes: AtomicU64,
	total_failures: AtomicU64,
	last_error: std::sync::Mutex<Option<String>>,
	/// Threshold for degraded status.
	degraded_threshold: u64,
	/// Threshold for unhealthy status.
	unhealthy_threshold: u64,
}

impl std::fmt::Debug for HealthMonitor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HealthMonitor")
			.field("consecutive_failures", &self.consecutive_failures.load(Ordering::Relaxed))
			.finish()
	}
}

impl HealthMonitor {
	/// Create a new health monitor with the given thresholds.
	pub fn new(degraded_threshold: u64, unhealthy_threshold: u64) -> Self {
		Self {
			consecutive_failures: AtomicU64::new(0),
			total_successes: AtomicU64::new(0),
			total_failures: AtomicU64::new(0),
			last_error: std::sync::Mutex::new(None),
			degraded_threshold,
			unhealthy_threshold,
		}
	}

	/// Record a successful request.
	pub fn record_success(&self) {
		self.consecutive_failures.store(0, Ordering::Relaxed);
		self.total_successes.fetch_add(1, Ordering::Relaxed);
	}

	/// Record a failed request.
	pub fn record_failure(&self, error: &str) {
		self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
		self.total_failures.fetch_add(1, Ordering::Relaxed);
		*self.last_error.lock().unwrap() = Some(error.to_string());
	}

	/// Current health status.
	pub fn status(&self) -> HealthStatus {
		let failures = self.consecutive_failures.load(Ordering::Relaxed);
		if failures >= self.unhealthy_threshold {
			HealthStatus::Unhealthy
		} else if failures >= self.degraded_threshold {
			HealthStatus::Degraded
		} else {
			HealthStatus::Healthy
		}
	}

	/// Snapshot of current health metrics.
	pub fn snapshot(&self) -> HealthSnapshot {
		HealthSnapshot {
			status: self.status(),
			consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
			total_successes: self.total_successes.load(Ordering::Relaxed),
			total_failures: self.total_failures.load(Ordering::Relaxed),
			last_error: self.last_error.lock().unwrap().clone(),
		}
	}
}

impl Default for HealthMonitor {
	fn default() -> Self {
		Self::new(3, 5)
	}
}

// ===========================================================================
// Retry functions
// ===========================================================================

/// Returns `true` if the error is likely transient and worth retrying.
///
/// Transient: Timeout, ServerUnavailable, ConnectionFailed, Io.
/// Non-transient: NotInitialized, PermissionDenied, SessionError,
/// ProtocolError, Serialization, CircuitBreakerOpen, StreamError.
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
pub async fn retry<T, F, Fut>(config: &RetryConfig, f: F) -> Result<T, AcpError>
where
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, AcpError>>,
{
	retry_generic(config, is_transient, f).await
}

/// Generic retry function that works with any error type.
///
/// The `is_retryable` predicate determines which errors trigger a retry.
pub async fn retry_generic<T, E, F, Fut>(
	config: &RetryConfig,
	is_retryable: fn(&E) -> bool,
	f: F,
) -> Result<T, E>
where
	E: std::fmt::Display,
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, E>>,
{
	let mut delay_ms = config.base_delay_ms;

	for attempt in 1..=config.max_attempts {
		match f().await {
			Ok(val) => return Ok(val),
			Err(e) => {
				// Non-retryable errors propagate immediately.
				if !is_retryable(&e) {
					return Err(e);
				}

				// Last attempt — propagate.
				if attempt == config.max_attempts {
					return Err(e);
				}

				tracing::debug!(
					"Retry attempt {}/{}: {}",
					attempt,
					config.max_attempts,
					e,
				);

				// Sleep with jitter.
				let jitter = (delay_ms as f64 * config.jitter_factor) as u64;
				let actual_delay = if jitter > 0 {
					// Simple deterministic jitter: alternate adding/subtracting.
					if attempt % 2 == 0 {
						delay_ms.saturating_add(jitter / 2)
					} else {
						delay_ms.saturating_sub(jitter / 2)
					}
				} else {
					delay_ms
				};

				tokio::time::sleep(Duration::from_millis(actual_delay)).await;

				// Exponential backoff, capped.
				delay_ms = ((delay_ms as f64 * config.backoff_multiplier) as u64)
					.min(config.max_delay_ms);
			}
		}
	}

	unreachable!("retry loop should have returned")
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicU32, Ordering};
	use std::sync::Arc;

	// -----------------------------------------------------------------------
	// is_transient classification
	// -----------------------------------------------------------------------

	#[test]
	fn is_transient_classifies_errors_correctly() {
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

		assert!(!is_transient(&AcpError::NotInitialized));
		assert!(!is_transient(&AcpError::PermissionDenied("x".into())));
		assert!(!is_transient(&AcpError::SessionError("x".into())));
		assert!(!is_transient(&AcpError::ProtocolError("x".into())));
		assert!(!is_transient(&AcpError::Serialization("x".into())));
		assert!(!is_transient(&AcpError::CircuitBreakerOpen("x".into())));
		assert!(!is_transient(&AcpError::StreamError("x".into())));
	}

	// -----------------------------------------------------------------------
	// retry wrapper
	// -----------------------------------------------------------------------

	#[tokio::test]
	async fn retry_succeeds_on_first_attempt() {
		let config = RetryConfig::default();
		let call_count = Arc::new(AtomicU32::new(0));
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
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let call_count = Arc::new(AtomicU32::new(0));
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
	async fn retry_does_not_retry_non_transient_errors() {
		let config = RetryConfig {
			max_attempts: 5,
			base_delay_ms: 1,
			..Default::default()
		};
		let call_count = Arc::new(AtomicU32::new(0));
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
		assert_eq!(call_count.load(Ordering::SeqCst), 1);
	}

	// -----------------------------------------------------------------------
	// CircuitBreaker
	// -----------------------------------------------------------------------

	#[test]
	fn circuit_breaker_starts_closed() {
		let cb = CircuitBreaker::default();
		assert!(cb.allow_request());
	}

	#[test]
	fn circuit_breaker_opens_after_threshold() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 3,
			reset_timeout_ms: 60_000,
			half_open_successes: 1,
		});

		cb.record_failure();
		cb.record_failure();
		assert!(cb.allow_request()); // 2 failures, threshold is 3
		cb.record_failure();
		assert!(!cb.allow_request()); // 3 failures, circuit opens
	}

	#[test]
	fn circuit_breaker_success_resets_failures() {
		let cb = CircuitBreaker::new(CircuitBreakerConfig {
			failure_threshold: 3,
			reset_timeout_ms: 60_000,
			half_open_successes: 1,
		});

		cb.record_failure();
		cb.record_failure();
		cb.record_success(); // resets consecutive failures
		cb.record_failure();
		cb.record_failure();
		assert!(cb.allow_request()); // still only 2 consecutive failures
	}

	// -----------------------------------------------------------------------
	// HealthMonitor
	// -----------------------------------------------------------------------

	#[test]
	fn health_monitor_starts_healthy() {
		let hm = HealthMonitor::default();
		assert_eq!(hm.status(), HealthStatus::Healthy);
	}

	#[test]
	fn health_monitor_becomes_degraded() {
		let hm = HealthMonitor::new(2, 5);
		hm.record_failure("err");
		hm.record_failure("err");
		assert_eq!(hm.status(), HealthStatus::Degraded);
	}

	#[test]
	fn health_monitor_becomes_unhealthy() {
		let hm = HealthMonitor::new(2, 5);
		for _ in 0..5 {
			hm.record_failure("err");
		}
		assert_eq!(hm.status(), HealthStatus::Unhealthy);
	}

	#[test]
	fn health_monitor_success_resets() {
		let hm = HealthMonitor::new(2, 5);
		for _ in 0..4 {
			hm.record_failure("err");
		}
		assert_eq!(hm.status(), HealthStatus::Degraded);
		hm.record_success();
		assert_eq!(hm.status(), HealthStatus::Healthy);
	}

	#[test]
	fn health_snapshot_captures_state() {
		let hm = HealthMonitor::default();
		hm.record_success();
		hm.record_failure("oops");

		let snap = hm.snapshot();
		assert_eq!(snap.total_successes, 1);
		assert_eq!(snap.total_failures, 1);
		assert_eq!(snap.consecutive_failures, 1);
		assert_eq!(snap.last_error, Some("oops".to_string()));
	}
}
