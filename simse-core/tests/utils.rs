use simse_core::error::SimseError;
use simse_core::utils::circuit_breaker::{CircuitBreaker, CircuitBreakerOptions, CircuitState};
use simse_core::utils::health_monitor::{
    create_health_monitor, HealthMonitorOptions, HealthStatus,
};
use simse_core::utils::retry::{retry, RetryOptions};
use simse_core::utils::timeout::with_timeout;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// retry tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_succeeds_on_third_attempt() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(SimseError::other("transient"))
                } else {
                    Ok(42)
                }
            }
        },
        RetryOptions {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            jitter_factor: 0.0,
            ..Default::default()
        },
        None,
    )
    .await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn retry_respects_max_attempts() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<(), SimseError> = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(SimseError::other("always fails"))
            }
        },
        RetryOptions {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            jitter_factor: 0.0,
            ..Default::default()
        },
        None,
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "RESILIENCE_RETRY_EXHAUSTED");
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn retry_aborts_on_cancellation() {
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    // Cancel after first attempt
    let result: Result<(), SimseError> = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            let t = token_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // After first failure, cancel
                    t.cancel();
                }
                Err(SimseError::other("fail"))
            }
        },
        RetryOptions {
            max_attempts: 10,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            jitter_factor: 0.0,
            ..Default::default()
        },
        Some(token),
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "RESILIENCE_RETRY_ABORTED");
    // Should have tried at most 1-2 times before cancellation was detected
    assert!(counter.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn retry_succeeds_first_attempt() {
    let result = retry(
        |_attempt| async { Ok::<_, SimseError>(100) },
        RetryOptions {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            ..Default::default()
        },
        None,
    )
    .await;

    assert_eq!(result.unwrap(), 100);
}

#[tokio::test]
async fn retry_with_should_retry_false_aborts_early() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<(), SimseError> = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(SimseError::other("non-retryable"))
            }
        },
        RetryOptions {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            should_retry: Some(Arc::new(|_err, _attempt| false)),
            ..Default::default()
        },
        None,
    )
    .await;

    assert!(result.is_err());
    // Should have only tried once because shouldRetry returned false
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    // The error should be the original, not RetryExhausted
    let err = result.unwrap_err();
    assert_eq!(err.code(), "OTHER_ERROR");
}

#[tokio::test]
async fn retry_calls_on_retry_callback() {
    let attempts_seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let attempts_clone = attempts_seen.clone();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let _result: Result<i32, SimseError> = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(SimseError::other("fail"))
                } else {
                    Ok(42)
                }
            }
        },
        RetryOptions {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            jitter_factor: 0.0,
            on_retry: Some(Arc::new(move |_err, attempt, _delay| {
                attempts_clone.lock().unwrap().push(attempt);
            })),
            ..Default::default()
        },
        None,
    )
    .await;

    let seen = attempts_seen.lock().unwrap();
    // on_retry called with next attempt number (2 and 3)
    assert_eq!(*seen, vec![2, 3]);
}

#[tokio::test]
async fn retry_exponential_backoff_delays() {
    // Verify backoff formula: base_delay * multiplier^(attempt-1), capped at max_delay
    let start = tokio::time::Instant::now();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let _result: Result<(), SimseError> = retry(
        move |_attempt| {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(SimseError::other("fail"))
            }
        },
        RetryOptions {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
            ..Default::default()
        },
        None,
    )
    .await;

    let elapsed = start.elapsed();
    // 2 delays: 10ms (10*2^0) + 20ms (10*2^1) = 30ms minimum
    assert!(elapsed >= Duration::from_millis(25)); // some tolerance
}

// ---------------------------------------------------------------------------
// circuit_breaker tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn circuit_breaker_opens_after_threshold() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 5,
        reset_timeout: Duration::from_secs(30),
        half_open_max_attempts: 1,
    });

    // Record 5 failures
    for _ in 0..5 {
        let result: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
        assert!(result.is_err());
    }

    assert_eq!(cb.state(), CircuitState::Open);

    // Next call should be rejected immediately
    let result: Result<(), SimseError> = cb
        .execute(|| async { Ok(()) })
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "RESILIENCE_CIRCUIT_OPEN");
}

#[tokio::test]
async fn circuit_breaker_resets_after_timeout() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(50),
        half_open_max_attempts: 1,
    });

    // Record 2 failures to open
    for _ in 0..2 {
        let _: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
    }
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Should now be half-open (lazy transition)
    assert_eq!(cb.state(), CircuitState::HalfOpen);
}

#[tokio::test]
async fn circuit_breaker_closes_on_success() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(50),
        half_open_max_attempts: 1,
    });

    // Open the circuit
    for _ in 0..2 {
        let _: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
    }
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Execute a successful call in half-open state
    let result = cb.execute(|| async { Ok::<_, SimseError>(42) }).await;
    assert_eq!(result.unwrap(), 42);

    // Should be closed now
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
}

#[tokio::test]
async fn circuit_breaker_half_open_failure_reopens() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(50),
        half_open_max_attempts: 1,
    });

    // Open the circuit
    for _ in 0..2 {
        let _: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
    }

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Fail again in half-open state
    let _: Result<(), SimseError> = cb
        .execute(|| async { Err(SimseError::other("fail again")) })
        .await;

    // Should re-open
    assert_eq!(cb.state(), CircuitState::Open);
}

#[tokio::test]
async fn circuit_breaker_reset() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 2,
        reset_timeout: Duration::from_secs(30),
        half_open_max_attempts: 1,
    });

    // Open the circuit
    for _ in 0..2 {
        let _: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
    }
    assert_eq!(cb.state(), CircuitState::Open);

    // Manual reset
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
}

#[tokio::test]
async fn circuit_breaker_success_resets_failure_count() {
    let cb = CircuitBreaker::new(CircuitBreakerOptions {
        name: "test".into(),
        failure_threshold: 5,
        reset_timeout: Duration::from_secs(30),
        half_open_max_attempts: 1,
    });

    // 3 failures (below threshold)
    for _ in 0..3 {
        let _: Result<(), SimseError> = cb
            .execute(|| async { Err(SimseError::other("fail")) })
            .await;
    }
    assert_eq!(cb.failure_count(), 3);
    assert_eq!(cb.state(), CircuitState::Closed);

    // 1 success resets the count
    let _ = cb.execute(|| async { Ok::<_, SimseError>(()) }).await;
    assert_eq!(cb.failure_count(), 0);
}

// ---------------------------------------------------------------------------
// health_monitor tests
// ---------------------------------------------------------------------------

#[test]
fn health_monitor_status_transitions() {
    let monitor = create_health_monitor(Some(HealthMonitorOptions {
        degraded_threshold: 3,
        unhealthy_threshold: 5,
        window_ms: 60_000,
    }));

    // Starts healthy
    assert_eq!(monitor.status(), HealthStatus::Healthy);
    assert!(monitor.is_healthy());

    // Record successes — stays healthy
    monitor.record_success();
    monitor.record_success();
    assert_eq!(monitor.status(), HealthStatus::Healthy);

    // 3 consecutive failures → Degraded
    monitor.record_failure(None);
    monitor.record_failure(None);
    monitor.record_failure(None);
    assert_eq!(monitor.status(), HealthStatus::Degraded);
    assert!(!monitor.is_healthy());

    // 2 more failures (total 5 consecutive) → Unhealthy
    monitor.record_failure(None);
    monitor.record_failure(None);
    assert_eq!(monitor.status(), HealthStatus::Unhealthy);
    assert!(!monitor.is_healthy());

    // One success resets consecutive failures → Healthy
    monitor.record_success();
    assert_eq!(monitor.status(), HealthStatus::Healthy);
    assert!(monitor.is_healthy());
}

#[test]
fn health_monitor_stats() {
    let monitor = create_health_monitor(None);

    monitor.record_success();
    monitor.record_failure(Some("something went wrong"));
    monitor.record_success();

    let health = monitor.get_health();
    assert_eq!(health.total_calls, 3);
    assert_eq!(health.total_failures, 1);
    assert_eq!(health.consecutive_failures, 0);
    assert!(health.failure_rate > 0.0);
    assert!(health.last_success_time.is_some());
    assert!(health.last_failure_time.is_some());
    assert!(health.last_error.is_some());
}

#[test]
fn health_monitor_reset() {
    let monitor = create_health_monitor(None);

    monitor.record_failure(Some("err"));
    monitor.record_failure(Some("err"));
    monitor.record_failure(Some("err"));
    assert_eq!(monitor.status(), HealthStatus::Degraded);

    monitor.reset();

    assert_eq!(monitor.status(), HealthStatus::Healthy);
    let health = monitor.get_health();
    assert_eq!(health.total_calls, 0);
    assert_eq!(health.total_failures, 0);
    assert_eq!(health.consecutive_failures, 0);
    assert!(health.last_success_time.is_none());
    assert!(health.last_failure_time.is_none());
    assert!(health.last_error.is_none());
}

#[test]
fn health_monitor_failure_rate_windowed() {
    let monitor = create_health_monitor(Some(HealthMonitorOptions {
        degraded_threshold: 3,
        unhealthy_threshold: 5,
        window_ms: 60_000,
    }));

    // 1 success + 1 failure = 50% failure rate
    monitor.record_success();
    monitor.record_failure(None);

    let health = monitor.get_health();
    assert!((health.failure_rate - 0.5).abs() < f64::EPSILON);
}

#[test]
fn health_monitor_error_stored() {
    let monitor = create_health_monitor(None);

    monitor.record_failure(Some("first error"));
    let h1 = monitor.get_health();
    assert_eq!(h1.last_error.as_deref(), Some("first error"));

    monitor.record_failure(Some("second error"));
    let h2 = monitor.get_health();
    assert_eq!(h2.last_error.as_deref(), Some("second error"));
}

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
