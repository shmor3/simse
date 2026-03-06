use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn starts_healthy() {
		let hm = HealthMonitor::default();
		assert_eq!(hm.status(), HealthStatus::Healthy);
		let snap = hm.snapshot();
		assert_eq!(snap.consecutive_failures, 0);
		assert_eq!(snap.total_calls, 0);
	}

	#[test]
	fn degraded_after_3_failures() {
		let hm = HealthMonitor::default();
		hm.record_failure("err1");
		hm.record_failure("err2");
		assert_eq!(hm.status(), HealthStatus::Healthy);

		hm.record_failure("err3");
		assert_eq!(hm.status(), HealthStatus::Degraded);
	}

	#[test]
	fn unhealthy_after_5_failures() {
		let hm = HealthMonitor::default();
		for i in 0..5 {
			hm.record_failure(&format!("err{i}"));
		}
		assert_eq!(hm.status(), HealthStatus::Unhealthy);
	}

	#[test]
	fn success_resets_consecutive_failures() {
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
	fn window_prunes_old_entries() {
		let hm = HealthMonitor::new(Duration::from_millis(0));
		hm.record_failure("err1");
		hm.record_failure("err2");

		let snap = hm.snapshot();
		assert_eq!(snap.window_failure_rate, 0.0);
		assert_eq!(snap.total_calls, 2);
		assert_eq!(snap.total_failures, 2);
	}

	#[test]
	fn snapshot_computes_failure_rate() {
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
	fn reset_clears_everything() {
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

	#[test]
	fn default_uses_60s_window() {
		let hm = HealthMonitor::default();
		assert_eq!(hm.window_duration, Duration::from_secs(60));
	}
}
