import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const cleanupMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	await next();

	// Run cleanup ~1% of requests
	if (Math.random() > 0.01) return;

	const db = c.env.DB;
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
				.prepare('DELETE FROM rate_limits WHERE window < ?')
				.bind(String(Math.floor(Date.now() / 1000) - 3600))
				.run(),
		]).catch(() => {}),
	);
});
