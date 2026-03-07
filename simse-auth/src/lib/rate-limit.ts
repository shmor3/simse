export async function checkRateLimit(
	db: D1Database,
	key: string,
	windowSeconds: number,
	maxAttempts: number,
): Promise<{ allowed: boolean; remaining: number }> {
	// Store window start as unix seconds so cleanup can compare against timestamps
	const windowStart =
		Math.floor(Date.now() / 1000 / windowSeconds) * windowSeconds;
	const window = String(windowStart);

	// Atomic upsert — increment and return count in one operation
	const row = await db
		.prepare(
			'INSERT INTO rate_limits (key, window, count) VALUES (?, ?, 1) ON CONFLICT(key, window) DO UPDATE SET count = count + 1 RETURNING count',
		)
		.bind(key, window)
		.first<{ count: number }>();

	const count = row?.count ?? 1;
	return {
		allowed: count <= maxAttempts,
		remaining: Math.max(0, maxAttempts - count),
	};
}
