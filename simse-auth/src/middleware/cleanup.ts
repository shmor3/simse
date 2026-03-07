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

	// LIMIT prevents runaway deletes from overwhelming D1 on large backlogs
	c.executionCtx.waitUntil(
		Promise.all([
			db
				.prepare(
					"DELETE FROM sessions WHERE id IN (SELECT id FROM sessions WHERE expires_at < datetime('now') LIMIT 500)",
				)
				.run(),
			db
				.prepare(
					"DELETE FROM tokens WHERE id IN (SELECT id FROM tokens WHERE expires_at < datetime('now') LIMIT 500)",
				)
				.run(),
			db
				.prepare(
					"DELETE FROM team_invites WHERE id IN (SELECT id FROM team_invites WHERE expires_at < datetime('now') LIMIT 500)",
				)
				.run(),
			db
				.prepare(
					"DELETE FROM refresh_tokens WHERE id IN (SELECT id FROM refresh_tokens WHERE revoked = 1 AND created_at < datetime('now', '-7 days') LIMIT 500)",
				)
				.run(),
			db
				.prepare(
					"DELETE FROM refresh_tokens WHERE id IN (SELECT id FROM refresh_tokens WHERE expires_at < datetime('now') LIMIT 500)",
				)
				.run(),
			db
				.prepare(
					'DELETE FROM rate_limits WHERE rowid IN (SELECT rowid FROM rate_limits WHERE CAST(window AS INTEGER) < ? LIMIT 1000)',
				)
				.bind(now - 3600)
				.run(),
		]).catch((err) => {
			console.error('Cleanup failed', err);
		}),
	);
});
