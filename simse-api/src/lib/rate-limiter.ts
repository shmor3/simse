interface WindowEntry {
	count: number;
	resetAt: number;
}

export class RateLimiter {
	private windows = new Map<string, WindowEntry>();
	private readonly windowMs: number;

	constructor(windowMs = 60_000) {
		this.windowMs = windowMs;
	}

	check(
		key: string,
		limit: number,
	): { allowed: boolean; remaining: number; resetAt: number } {
		const now = Date.now();
		const entry = this.windows.get(key);

		if (!entry || now >= entry.resetAt) {
			const resetAt = now + this.windowMs;
			this.windows.set(key, { count: 1, resetAt });
			return { allowed: true, remaining: limit - 1, resetAt };
		}

		entry.count++;

		if (entry.count > limit) {
			return {
				allowed: false,
				remaining: 0,
				resetAt: entry.resetAt,
			};
		}

		return {
			allowed: true,
			remaining: limit - entry.count,
			resetAt: entry.resetAt,
		};
	}

	prune(): void {
		const now = Date.now();
		for (const [key, entry] of this.windows) {
			if (now >= entry.resetAt) {
				this.windows.delete(key);
			}
		}
	}
}
