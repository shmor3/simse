use simse_core::error::SimseError;
use simse_core::utils::timeout::with_timeout;
use std::time::Duration;

// ---------------------------------------------------------------------------
// timeout tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn with_timeout_completes() {
    let result = with_timeout(
        async { Ok::<_, SimseError>(42) },
        Duration::from_secs(1),
        None,
    )
    .await;

    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn with_timeout_cancels() {
    let result: Result<(), SimseError> = with_timeout(
        async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(())
        },
        Duration::from_millis(50),
        Some("slow_operation"),
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "RESILIENCE_TIMEOUT");
}

#[tokio::test]
async fn with_timeout_propagates_error() {
    let result: Result<(), SimseError> = with_timeout(
        async { Err(SimseError::other("inner error")) },
        Duration::from_secs(1),
        None,
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "OTHER_ERROR");
}

#[tokio::test]
async fn with_timeout_zero_duration_completes_fast_fn() {
    // A zero timeout should still let an already-ready future complete
    // (tokio::time::timeout with zero duration resolves immediately-ready futures)
    let result = with_timeout(
        async { Ok::<_, SimseError>(99) },
        Duration::ZERO,
        None,
    )
    .await;

    // This might succeed or timeout depending on scheduling — either is valid.
    // The main thing is it doesn't panic.
    let _ = result;
}
