import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const cleanupMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	await next();

	// Run cleanup ~1% of requests
	if (Math.random() > 0.01) return;

	const db = c.env.DB;
	const now = Math.floor(Date.now() / 1000);

	c.executionCtx.waitUntil(
		Promise.all([
			db
				.prepare("DELETE FROM sessions WHERE expires_at < datetime('now')")
				.run(),
			db.prepare("DELETE FROM tokens WHERE expires_at < datetime('now')").run(),
			db
				.prepare("DELETE FROM team_invites WHERE expires_at < datetime('now')")
				.run(),
			db
				.prepare(
					"DELETE FROM refresh_tokens WHERE revoked = 1 AND created_at < datetime('now', '-7 days')",
				)
				.run(),
			db
				.prepare(
					"DELETE FROM refresh_tokens WHERE expires_at < datetime('now')",
				)
				.run(),
			// Rate limits: delete all entries (they're cheap to recreate, and stale ones waste space)
			// Using a conservative approach: delete rows that are definitely from a past window
			// The smallest window used is 60s, so any row older than 1 hour is definitely stale
			db
				.prepare('DELETE FROM rate_limits WHERE CAST(window AS INTEGER) < ?')
				.bind(now - 3600)
				.run(),
		]).catch((err) => {
			console.error('Cleanup failed', err);
		}),
	);
});
