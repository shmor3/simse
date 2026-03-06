// ---------------------------------------------------------------------------
// Resilience — re-exports from simse-resilience + AcpError-specific helpers
// ---------------------------------------------------------------------------

pub use simse_resilience::{
	CircuitBreaker, CircuitBreakerConfig, CircuitState,
	HealthMonitor, HealthSnapshot, HealthStatus,
	RetryConfig,
};

use crate::error::AcpError;

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
	simse_resilience::retry(config, is_transient, f).await
}

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
}
