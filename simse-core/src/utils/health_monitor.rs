//! Health monitor — tracks success/failure rates and reports service health
//! using a sliding time window.
//!
//! Ports the TypeScript `src/utils/health-monitor.ts` to Rust.

use std::sync::Mutex;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Health status of a monitored service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Configuration for [`HealthMonitor`].
pub struct HealthMonitorOptions {
    /// Consecutive failures required to enter "Degraded" state. Default 3.
    pub degraded_threshold: u32,
    /// Consecutive failures required to enter "Unhealthy" state. Default 5.
    pub unhealthy_threshold: u32,
    /// Sliding window in milliseconds for failure rate calculation. Default 60_000.
    pub window_ms: u64,
}

impl Default for HealthMonitorOptions {
    fn default() -> Self {
        Self {
            degraded_threshold: 3,
            unhealthy_threshold: 5,
            window_ms: 60_000,
        }
    }
}

/// A snapshot of health statistics at a point in time.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    pub status: HealthStatus,
    pub consecutive_failures: u32,
    pub total_calls: u64,
    pub total_failures: u64,
    pub failure_rate: f64,
    pub last_success_time: Option<Instant>,
    pub last_failure_time: Option<Instant>,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct Event {
    time: Instant,
    success: bool,
}

struct Inner {
    degraded_threshold: u32,
    unhealthy_threshold: u32,
    window_ms: u64,
    consecutive_failures: u32,
    total_calls: u64,
    total_failures: u64,
    last_success_time: Option<Instant>,
    last_failure_time: Option<Instant>,
    last_error: Option<String>,
    events: Vec<Event>,
}

impl Inner {
    fn prune_window(&mut self) {
        let cutoff = Instant::now()
            .checked_sub(std::time::Duration::from_millis(self.window_ms))
            .unwrap_or_else(Instant::now);
        self.events.retain(|e| e.time >= cutoff);
    }

    fn compute_status(&self) -> HealthStatus {
        if self.consecutive_failures >= self.unhealthy_threshold {
            HealthStatus::Unhealthy
        } else if self.consecutive_failures >= self.degraded_threshold {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

// ---------------------------------------------------------------------------
// HealthMonitor
// ---------------------------------------------------------------------------

/// Tracks success/failure of operations and computes health status.
pub struct HealthMonitor {
    inner: Mutex<Inner>,
}

/// Create a new health monitor with the given options.
pub fn create_health_monitor(options: Option<HealthMonitorOptions>) -> HealthMonitor {
    let opts = options.unwrap_or_default();
    HealthMonitor {
        inner: Mutex::new(Inner {
            degraded_threshold: opts.degraded_threshold,
            unhealthy_threshold: opts.unhealthy_threshold,
            window_ms: opts.window_ms,
            consecutive_failures: 0,
            total_calls: 0,
            total_failures: 0,
            last_success_time: None,
            last_failure_time: None,
            last_error: None,
            events: Vec::new(),
        }),
    }
}

impl HealthMonitor {
    /// Record a successful operation.
    pub fn record_success(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.prune_window();
        inner.total_calls += 1;
        inner.consecutive_failures = 0;
        let now = Instant::now();
        inner.last_success_time = Some(now);
        inner.events.push(Event {
            time: now,
            success: true,
        });
    }

    /// Record a failed operation with an optional error message.
    pub fn record_failure(&self, error: Option<&str>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.prune_window();
        inner.total_calls += 1;
        inner.total_failures += 1;
        inner.consecutive_failures += 1;
        let now = Instant::now();
        inner.last_failure_time = Some(now);
        if let Some(msg) = error {
            inner.last_error = Some(msg.to_string());
        }
        inner.events.push(Event {
            time: now,
            success: false,
        });
    }

    /// Get a snapshot of the current health statistics.
    pub fn get_health(&self) -> HealthSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.prune_window();

        let windowed_total = inner.events.len();
        let windowed_failures = inner.events.iter().filter(|e| !e.success).count();
        let failure_rate = if windowed_total > 0 {
            windowed_failures as f64 / windowed_total as f64
        } else {
            0.0
        };

        HealthSnapshot {
            status: inner.compute_status(),
            consecutive_failures: inner.consecutive_failures,
            total_calls: inner.total_calls,
            total_failures: inner.total_failures,
            failure_rate,
            last_success_time: inner.last_success_time,
            last_failure_time: inner.last_failure_time,
            last_error: inner.last_error.clone(),
        }
    }

    /// Returns the current health status.
    pub fn status(&self) -> HealthStatus {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).compute_status()
    }

    /// Returns `true` if the current status is `Healthy`.
    pub fn is_healthy(&self) -> bool {
        self.status() == HealthStatus::Healthy
    }

    /// Reset all counters and state.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.consecutive_failures = 0;
        inner.total_calls = 0;
        inner.total_failures = 0;
        inner.last_success_time = None;
        inner.last_failure_time = None;
        inner.last_error = None;
        inner.events.clear();
    }
}
