export async function checkRateLimit(
	db: D1Database,
	key: string,
	windowSeconds: number,
	maxAttempts: number,
): Promise<{ allowed: boolean; remaining: number }> {
	const window = String(Math.floor(Date.now() / (windowSeconds * 1000)));

	const row = await db
		.prepare('SELECT count FROM rate_limits WHERE key = ? AND window = ?')
		.bind(key, window)
		.first<{ count: number }>();

	if (row && row.count >= maxAttempts) {
		return { allowed: false, remaining: 0 };
	}

	await db
		.prepare(
			'INSERT INTO rate_limits (key, window, count) VALUES (?, ?, 1) ON CONFLICT(key, window) DO UPDATE SET count = count + 1',
		)
		.bind(key, window)
		.run();

	const count = (row?.count ?? 0) + 1;
	return {
		allowed: count <= maxAttempts,
		remaining: Math.max(0, maxAttempts - count),
	};
}
