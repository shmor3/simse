use std::time::Duration;

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

/// Execute an async closure with automatic retries and exponential backoff.
///
/// The `is_transient` closure determines which errors should trigger a retry.
/// Non-transient errors propagate immediately.
///
/// # Delay calculation
///
/// ```text
/// base_delay = min(base_delay_ms * multiplier^(attempt-1), max_delay_ms)
/// jitter     = base_delay * jitter_factor * deterministic_fraction
/// final      = base_delay + jitter
/// ```
pub async fn retry<T, E, F, Fut>(
	config: &RetryConfig,
	is_transient: impl Fn(&E) -> bool,
	f: F,
) -> Result<T, E>
where
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, E>>,
{
	let max_attempts = config.max_attempts.max(1);
	let mut last_error: Option<E> = None;

	for attempt in 1..=max_attempts {
		match f().await {
			Ok(value) => return Ok(value),
			Err(err) => {
				if attempt >= max_attempts {
					last_error = Some(err);
					break;
				}

				if !is_transient(&err) {
					return Err(err);
				}

				last_error = Some(err);

				let exponential =
					config.base_delay_ms as f64 * config.backoff_multiplier.powi(attempt as i32 - 1);
				let capped = exponential.min(config.max_delay_ms as f64);
				let jitter_frac = deterministic_jitter_fraction(attempt);
				let jitter = capped * config.jitter_factor * jitter_frac;
				let delay_ms = (capped + jitter).max(0.0).round() as u64;

				tokio::time::sleep(Duration::from_millis(delay_ms)).await;
			}
		}
	}

	Err(last_error.expect("retry loop must have captured an error"))
}

/// Produce a deterministic fraction in [0.0, 1.0) from an attempt number.
///
/// Uses Knuth's multiplicative hash to spread values without requiring an RNG.
fn deterministic_jitter_fraction(attempt: u32) -> f64 {
	let hash = (attempt as u64).wrapping_mul(2_654_435_761);
	(hash % 1_000_000) as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicU32, Ordering};
	use std::sync::Arc;

	#[derive(Debug)]
	enum TestError {
		Transient(String),
		Fatal(String),
	}

	fn is_transient(e: &TestError) -> bool {
		matches!(e, TestError::Transient(_))
	}

	#[tokio::test]
	async fn succeeds_on_first_attempt() {
		let config = RetryConfig::default();
		let count = Arc::new(AtomicU32::new(0));
		let cc = count.clone();

		let result = retry(&config, is_transient, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Ok::<_, TestError>(42)
			}
		})
		.await;

		assert_eq!(result.unwrap(), 42);
		assert_eq!(count.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn retries_transient_and_succeeds() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let count = Arc::new(AtomicU32::new(0));
		let cc = count.clone();

		let result = retry(&config, is_transient, || {
			let cc = cc.clone();
			async move {
				let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
				if attempt < 2 {
					Err(TestError::Transient("down".into()))
				} else {
					Ok(99)
				}
			}
		})
		.await;

		assert_eq!(result.unwrap(), 99);
		assert_eq!(count.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn stops_after_max_attempts() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let count = Arc::new(AtomicU32::new(0));
		let cc = count.clone();

		let result: Result<i32, TestError> = retry(&config, is_transient, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(TestError::Transient("down".into()))
			}
		})
		.await;

		assert!(result.is_err());
		assert_eq!(count.load(Ordering::SeqCst), 3);
	}

	#[tokio::test]
	async fn does_not_retry_fatal_errors() {
		let config = RetryConfig {
			max_attempts: 5,
			base_delay_ms: 1,
			..Default::default()
		};
		let count = Arc::new(AtomicU32::new(0));
		let cc = count.clone();

		let result: Result<i32, TestError> = retry(&config, is_transient, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(TestError::Fatal("nope".into()))
			}
		})
		.await;

		assert!(result.is_err());
		assert_eq!(count.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn default_config_has_sensible_values() {
		let cfg = RetryConfig::default();
		assert_eq!(cfg.max_attempts, 3);
		assert_eq!(cfg.base_delay_ms, 500);
		assert_eq!(cfg.max_delay_ms, 15_000);
		assert!((cfg.backoff_multiplier - 2.0).abs() < f64::EPSILON);
		assert!((cfg.jitter_factor - 0.25).abs() < f64::EPSILON);
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
