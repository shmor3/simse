//! Timeout utility — wraps a future with a deadline that rejects with a
//! structured `RESILIENCE_TIMEOUT` error.
//!
//! Ports the TypeScript `src/utils/timeout.ts` to Rust using
//! `tokio::time::timeout`.

use std::future::Future;
use std::time::Duration;

use crate::error::{ResilienceErrorCode, SimseError};

/// Run a future with a timeout. Returns a `RESILIENCE_TIMEOUT` error if the
/// future does not complete within `duration`.
///
/// The optional `operation` label is included in the error message.
pub async fn with_timeout<F, T>(
    future: F,
    duration: Duration,
    operation: Option<&str>,
) -> Result<T, SimseError>
where
    F: Future<Output = Result<T, SimseError>>,
{
    let label = operation.unwrap_or("unknown");

    match tokio::time::timeout(duration, future).await {
        Ok(result) => result,
        Err(_elapsed) => Err(SimseError::resilience(
            ResilienceErrorCode::Timeout,
            format!(
                "Operation '{}' timed out after {}ms",
                label,
                duration.as_millis()
            ),
        )),
    }
}
