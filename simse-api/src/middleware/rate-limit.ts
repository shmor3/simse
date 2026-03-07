import type { Context } from 'hono';
import { createMiddleware } from 'hono/factory';
import { RateLimiter } from '../lib/rate-limiter';
import type { AppVariables, Env } from '../types';

const limiter = new RateLimiter(60_000);

let lastPrune = Date.now();

interface RateLimitRule {
	pattern: RegExp;
	limit: number;
	keyType: 'ip' | 'user';
}

const rules: RateLimitRule[] = [
	// Public routes — per-IP
	{ pattern: /^\/auth\/(login|register)$/, limit: 10, keyType: 'ip' },
	{
		pattern: /^\/auth\/(reset-password|new-password)$/,
		limit: 5,
		keyType: 'ip',
	},
	{ pattern: /^\/auth\/(2fa|verify-email)$/, limit: 10, keyType: 'ip' },
	{ pattern: /^\/auth\/refresh$/, limit: 30, keyType: 'ip' },
	// Protected routes — per-user
	{ pattern: /^\/(users|teams|api-keys)(\/|$)/, limit: 60, keyType: 'user' },
	{ pattern: /^\/payments(\/|$)/, limit: 30, keyType: 'user' },
	{ pattern: /^\/notifications(\/|$)/, limit: 20, keyType: 'user' },
];

function getClientIp(
	c: Context<{ Bindings: Env; Variables: AppVariables }>,
): string {
	return (
		c.req.header('CF-Connecting-IP') ??
		c.req.header('X-Forwarded-For')?.split(',')[0]?.trim() ??
		'unknown'
	);
}

export const rateLimitMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: AppVariables;
}>(async (c, next) => {
	const now = Date.now();
	if (now - lastPrune > 60_000) {
		limiter.prune();
		lastPrune = now;
	}

	const path = c.req.path;
	const rule = rules.find((r) => r.pattern.test(path));

	if (!rule) {
		await next();
		return;
	}

	const key =
		rule.keyType === 'ip'
			? `ip:${getClientIp(c)}:${rule.pattern.source}`
			: `user:${c.req.header('X-User-Id') ?? getClientIp(c)}:${rule.pattern.source}`;

	const result = limiter.check(key, rule.limit);

	c.header('X-RateLimit-Limit', String(rule.limit));
	c.header('X-RateLimit-Remaining', String(Math.max(0, result.remaining)));
	c.header('X-RateLimit-Reset', String(Math.ceil(result.resetAt / 1000)));

	if (!result.allowed) {
		const retryAfter = Math.ceil((result.resetAt - now) / 1000);
		c.header('Retry-After', String(retryAfter));
		return c.json(
			{ error: { code: 'RATE_LIMITED', message: 'Too many requests' } },
			429,
		);
	}

	await next();
});
