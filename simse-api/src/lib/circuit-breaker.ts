type CircuitState = 'closed' | 'open' | 'half-open';

export class CircuitBreaker {
	readonly name: string;
	private state: CircuitState = 'closed';
	private failureCount = 0;
	private lastFailureTime = 0;
	private readonly failureThreshold: number;
	private readonly resetTimeoutMs: number;
	private readonly windowMs: number;

	constructor(
		name: string,
		options?: {
			failureThreshold?: number;
			resetTimeoutMs?: number;
			windowMs?: number;
		},
	) {
		this.name = name;
		this.failureThreshold = options?.failureThreshold ?? 5;
		this.resetTimeoutMs = options?.resetTimeoutMs ?? 30_000;
		this.windowMs = options?.windowMs ?? 60_000;
	}

	canRequest(): boolean {
		if (this.state === 'closed') return true;

		if (this.state === 'open') {
			if (Date.now() - this.lastFailureTime >= this.resetTimeoutMs) {
				this.state = 'half-open';
				return true;
			}
			return false;
		}

		// half-open: allow one probe
		return true;
	}

	recordSuccess(): void {
		this.state = 'closed';
		this.failureCount = 0;
	}

	recordFailure(): void {
		const now = Date.now();

		if (now - this.lastFailureTime > this.windowMs) {
			this.failureCount = 0;
		}

		this.failureCount++;
		this.lastFailureTime = now;

		if (this.state === 'half-open') {
			this.state = 'open';
			return;
		}

		if (this.failureCount >= this.failureThreshold) {
			this.state = 'open';
		}
	}

	getState(): CircuitState {
		if (
			this.state === 'open' &&
			Date.now() - this.lastFailureTime >= this.resetTimeoutMs
		) {
			this.state = 'half-open';
		}
		return this.state;
	}

	getStatus(): 'healthy' | 'degraded' | 'open' {
		const s = this.getState();
		if (s === 'closed') return 'healthy';
		if (s === 'half-open') return 'degraded';
		return 'open';
	}
}
