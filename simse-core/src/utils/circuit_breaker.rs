//! Circuit breaker — prevents cascading failures by short-circuiting
//! requests to unhealthy services.
//!
//! States: Closed -> Open -> HalfOpen -> Closed

use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{ResilienceErrorCode, SimseError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The three states of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Circuit is open — requests are rejected immediately.
    Open,
    /// Probing — a limited number of requests are allowed through.
    HalfOpen,
}

/// Configuration for [`CircuitBreaker`].
pub struct CircuitBreakerOptions {
    /// Identifier for this breaker (used in error messages).
    pub name: String,
    /// Number of consecutive failures before opening the circuit. Default 5.
    pub failure_threshold: u32,
    /// Time to wait before transitioning from Open to HalfOpen. Default 30s.
    pub reset_timeout: Duration,
    /// Max attempts allowed in HalfOpen state before re-opening. Default 1.
    pub half_open_max_attempts: u32,
}

impl Default for CircuitBreakerOptions {
    fn default() -> Self {
        Self {
            name: "default".into(),
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
            half_open_max_attempts: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct Inner {
    name: String,
    state: CircuitState,
    failure_count: u32,
    last_failure_time: Option<Instant>,
    half_open_attempts: u32,
    failure_threshold: u32,
    reset_timeout: Duration,
    half_open_max_attempts: u32,
}

// ---------------------------------------------------------------------------
// CircuitBreaker
// ---------------------------------------------------------------------------

/// A circuit breaker that wraps async operations and short-circuits when
/// the failure threshold is exceeded.
pub struct CircuitBreaker {
    inner: Mutex<Inner>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given options.
    pub fn new(opts: CircuitBreakerOptions) -> Self {
        Self {
            inner: Mutex::new(Inner {
                name: opts.name,
                state: CircuitState::Closed,
                failure_count: 0,
                last_failure_time: None,
                half_open_attempts: 0,
                failure_threshold: opts.failure_threshold,
                reset_timeout: opts.reset_timeout,
                half_open_max_attempts: opts.half_open_max_attempts,
            }),
        }
    }

    /// Execute an async closure through the circuit breaker.
    ///
    /// - In **Closed** state, the closure is called normally.
    /// - In **Open** state, the call is rejected with `RESILIENCE_CIRCUIT_OPEN`
    ///   unless the reset timeout has elapsed, in which case the circuit
    ///   transitions to **HalfOpen**.
    /// - In **HalfOpen** state, a limited number of probe requests are allowed.
    ///   A success closes the circuit; a failure re-opens it.
    pub async fn execute<F, Fut, T>(&self, f: F) -> Result<T, SimseError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, SimseError>>,
    {
        // Pre-call checks (under lock)
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

            if inner.state == CircuitState::Open {
                if let Some(last) = inner.last_failure_time {
                    if last.elapsed() >= inner.reset_timeout {
                        inner.state = CircuitState::HalfOpen;
                        inner.half_open_attempts = 0;
                    } else {
                        return Err(SimseError::resilience(
                            ResilienceErrorCode::CircuitOpen,
                            format!(
                                "Circuit breaker '{}' is open",
                                inner.name
                            ),
                        ));
                    }
                } else {
                    return Err(SimseError::resilience(
                        ResilienceErrorCode::CircuitOpen,
                        format!(
                            "Circuit breaker '{}' is open",
                            inner.name
                        ),
                    ));
                }
            }

            if inner.state == CircuitState::HalfOpen
                && inner.half_open_attempts >= inner.half_open_max_attempts
            {
                return Err(SimseError::resilience(
                    ResilienceErrorCode::CircuitOpen,
                    format!(
                        "Circuit breaker '{}' is open (half-open limit reached)",
                        inner.name
                    ),
                ));
            }

            if inner.state == CircuitState::HalfOpen {
                inner.half_open_attempts += 1;
            }
        }

        // Execute outside the lock
        match f().await {
            Ok(value) => {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                if inner.state == CircuitState::HalfOpen {
                    inner.state = CircuitState::Closed;
                }
                inner.failure_count = 0;
                Ok(value)
            }
            Err(err) => {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.failure_count += 1;
                inner.last_failure_time = Some(Instant::now());

                if inner.state == CircuitState::HalfOpen {
                    // Any failure in half-open → re-open
                    inner.state = CircuitState::Open;
                } else if inner.failure_count >= inner.failure_threshold {
                    inner.state = CircuitState::Open;
                }

                Err(err)
            }
        }
    }

    /// Returns the current circuit state with lazy Open->HalfOpen transition.
    pub fn state(&self) -> CircuitState {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.state == CircuitState::Open {
            if let Some(last) = inner.last_failure_time {
                if last.elapsed() >= inner.reset_timeout {
                    inner.state = CircuitState::HalfOpen;
                    inner.half_open_attempts = 0;
                }
            }
        }
        inner.state
    }

    /// Returns the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).failure_count
    }

    /// Reset the circuit breaker to Closed state.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.last_failure_time = None;
        inner.half_open_attempts = 0;
    }
}
