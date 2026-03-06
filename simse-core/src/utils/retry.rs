//! Retry utility with exponential backoff and jitter.
//!
//! Uses `tokio_util::sync::CancellationToken` for cooperative cancellation.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rand::RngExt;
use tokio_util::sync::CancellationToken;

use crate::error::{ResilienceErrorCode, SimseError};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Callback invoked before each retry with the error, upcoming attempt number,
/// and computed delay.
pub type OnRetryFn = dyn Fn(&SimseError, u32, Duration) + Send + Sync;

/// Predicate that decides whether the operation should be retried for a given
/// error and attempt number. Return `false` to abort immediately.
pub type ShouldRetryFn = dyn Fn(&SimseError, u32) -> bool + Send + Sync;

/// Configuration knobs for [`retry`].
pub struct RetryOptions {
    /// Maximum number of attempts (including the first). Minimum 1.
    pub max_attempts: u32,
    /// Base delay before the first retry.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each retry.
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0..=1.0). Adds randomness to prevent thundering herd.
    pub jitter_factor: f64,
    /// Optional predicate — return `false` to abort immediately for
    /// non-retryable errors. Defaults to always retry.
    pub should_retry: Option<Arc<ShouldRetryFn>>,
    /// Called before each retry with the error and upcoming attempt number.
    pub on_retry: Option<Arc<OnRetryFn>>,
}

impl Default for RetryOptions {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(15),
            backoff_multiplier: 2.0,
            jitter_factor: 0.25,
            should_retry: None,
            on_retry: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Core retry function
// ---------------------------------------------------------------------------

/// Execute an async closure with automatic retries and exponential backoff.
///
/// The closure receives the current attempt number (1-based). On every failure
/// the delay is computed as `base_delay * multiplier^(attempt-1)`, capped at
/// `max_delay`, then jittered by `±jitter_factor * delay`.
///
/// Pass an optional [`CancellationToken`] for cooperative cancellation. When
/// cancelled, the function returns a `RESILIENCE_RETRY_ABORTED` error.
pub async fn retry<F, Fut, T>(
    f: F,
    opts: RetryOptions,
    cancel: Option<CancellationToken>,
) -> Result<T, SimseError>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = Result<T, SimseError>>,
{
    let max_attempts = opts.max_attempts.max(1);
    let base_delay = opts.base_delay;
    let max_delay = opts.max_delay;
    let multiplier = opts.backoff_multiplier;
    let jitter_factor = opts.jitter_factor.clamp(0.0, 1.0);
    let should_retry = opts.should_retry;
    let on_retry = opts.on_retry;

    let mut last_error: Option<SimseError> = None;

    for attempt in 1..=max_attempts {
        // Check cancellation before each attempt
        if let Some(ref token) = cancel
            && token.is_cancelled() {
                return Err(SimseError::resilience(
                    ResilienceErrorCode::RetryAborted,
                    "Retry aborted by cancellation token",
                ));
            }

        match f(attempt).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_error = Some(err);

                // Don't retry on the last attempt
                if attempt >= max_attempts {
                    break;
                }

                // Check whether this error is retryable
                if let (Some(predicate), Some(err)) = (&should_retry, &last_error)
                    && !predicate(err, attempt) {
                        return Err(last_error.expect("checked above"));
                    }

                // Calculate delay with exponential backoff and jitter
                let exp_delay_secs =
                    base_delay.as_secs_f64() * multiplier.powi((attempt - 1) as i32);
                let capped_secs = exp_delay_secs.min(max_delay.as_secs_f64());

                let jitter = if jitter_factor > 0.0 {
                    let mut rng = rand::rng();
                    let j: f64 = rng.random_range(-1.0..1.0);
                    capped_secs * jitter_factor * j
                } else {
                    0.0
                };

                let final_secs = (capped_secs + jitter).max(0.0);
                let final_delay = Duration::from_secs_f64(final_secs);

                if let (Some(cb), Some(err)) = (&on_retry, &last_error) {
                    cb(err, attempt + 1, final_delay);
                }

                // Sleep, but respect cancellation
                if let Some(ref token) = cancel {
                    tokio::select! {
                        () = tokio::time::sleep(final_delay) => {}
                        () = token.cancelled() => {
                            return Err(SimseError::resilience(
                                ResilienceErrorCode::RetryAborted,
                                "Retry aborted by cancellation token",
                            ));
                        }
                    }
                } else {
                    tokio::time::sleep(final_delay).await;
                }
            }
        }
    }

    Err(SimseError::resilience(
        ResilienceErrorCode::RetryExhausted,
        format!(
            "All {} retry attempts exhausted: {}",
            max_attempts,
            last_error
                .as_ref()
                .map(|e| e.to_string())
                .unwrap_or_default()
        ),
    ))
}
