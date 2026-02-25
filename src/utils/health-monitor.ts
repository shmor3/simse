// ---------------------------------------------------------------------------
// Health Monitor — tracks success/failure rates and reports service health
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type HealthStatus = 'healthy' | 'degraded' | 'unhealthy';

export interface HealthMonitorOptions {
	/** Consecutive failures required to enter "degraded" state. Default 3. */
	readonly degradedThreshold?: number;
	/** Consecutive failures required to enter "unhealthy" state. Default 5. */
	readonly unhealthyThreshold?: number;
	/** Sliding window in ms for failure rate calculation. Default 60_000. */
	readonly windowMs?: number;
}

export interface HealthSnapshot {
	readonly status: HealthStatus;
	readonly consecutiveFailures: number;
	readonly totalCalls: number;
	readonly totalFailures: number;
	readonly failureRate: number;
	readonly lastSuccessTime?: number;
	readonly lastFailureTime?: number;
	readonly lastError?: Error;
}

export interface HealthMonitor {
	readonly recordSuccess: () => void;
	readonly recordFailure: (error?: unknown) => void;
	readonly getHealth: () => HealthSnapshot;
	readonly isHealthy: () => boolean;
	readonly reset: () => void;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createHealthMonitor(
	options?: HealthMonitorOptions,
): HealthMonitor {
	const degradedThreshold = options?.degradedThreshold ?? 3;
	const unhealthyThreshold = options?.unhealthyThreshold ?? 5;
	const windowMs = options?.windowMs ?? 60_000;

	let consecutiveFailures = 0;
	let totalCalls = 0;
	let totalFailures = 0;
	let lastSuccessTime: number | undefined;
	let lastFailureTime: number | undefined;
	let lastError: Error | undefined;

	// Windowed events for failure rate calculation
	const events: Array<{ time: number; success: boolean }> = [];

	const pruneWindow = (): void => {
		const cutoff = Date.now() - windowMs;
		while (events.length > 0 && events[0].time < cutoff) {
			events.shift();
		}
	};

	const recordSuccess = (): void => {
		totalCalls++;
		consecutiveFailures = 0;
		lastSuccessTime = Date.now();
		events.push({ time: Date.now(), success: true });
	};

	const recordFailure = (error?: unknown): void => {
		totalCalls++;
		totalFailures++;
		consecutiveFailures++;
		lastFailureTime = Date.now();
		if (error instanceof Error) {
			lastError = error;
		} else if (error !== undefined) {
			lastError = new Error(String(error));
		}
		events.push({ time: Date.now(), success: false });
	};

	const computeStatus = (): HealthStatus => {
		if (consecutiveFailures >= unhealthyThreshold) return 'unhealthy';
		if (consecutiveFailures >= degradedThreshold) return 'degraded';
		return 'healthy';
	};

	const getHealth = (): HealthSnapshot => {
		pruneWindow();

		const windowedTotal = events.length;
		const windowedFailures = events.filter((e) => !e.success).length;
		const failureRate =
			windowedTotal > 0 ? windowedFailures / windowedTotal : 0;

		return Object.freeze({
			status: computeStatus(),
			consecutiveFailures,
			totalCalls,
			totalFailures,
			failureRate,
			lastSuccessTime,
			lastFailureTime,
			lastError,
		});
	};

	const isHealthy = (): boolean => computeStatus() === 'healthy';

	const reset = (): void => {
		consecutiveFailures = 0;
		totalCalls = 0;
		totalFailures = 0;
		lastSuccessTime = undefined;
		lastFailureTime = undefined;
		lastError = undefined;
		events.length = 0;
	};

	return Object.freeze({
		recordSuccess,
		recordFailure,
		getHealth,
		isHealthy,
		reset,
	});
}
