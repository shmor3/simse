import { createMiddleware } from 'hono/factory';
import type { ApiSecrets, AppVariables, Env } from '../types';

let cachedSecrets: ApiSecrets | null = null;
let cacheTime = 0;
const CACHE_TTL_MS = 300_000; // 5 minutes

export const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: AppVariables;
}>(async (c, next) => {
	const now = Date.now();

	if (cachedSecrets && now - cacheTime < CACHE_TTL_MS) {
		c.set('secrets', cachedSecrets);
		await next();
		return;
	}

	const [
		authApiUrl,
		authApiSecret,
		paymentsApiUrl,
		paymentsApiSecret,
		mailerApiUrl,
		mailerApiSecret,
		jwtSecret,
	] = await Promise.all([
		c.env.SECRETS.get('AUTH_API_URL'),
		c.env.SECRETS.get('AUTH_API_SECRET'),
		c.env.SECRETS.get('PAYMENTS_API_URL'),
		c.env.SECRETS.get('PAYMENTS_API_SECRET'),
		c.env.SECRETS.get('MAILER_API_URL'),
		c.env.SECRETS.get('MAILER_API_SECRET'),
		c.env.SECRETS.get('JWT_SECRET'),
	]);

	if (
		!authApiUrl ||
		!authApiSecret ||
		!paymentsApiUrl ||
		!paymentsApiSecret ||
		!mailerApiUrl ||
		!mailerApiSecret ||
		!jwtSecret
	) {
		return c.json(
			{ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } },
			500,
		);
	}

	cachedSecrets = {
		authApiUrl,
		authApiSecret,
		paymentsApiUrl,
		paymentsApiSecret,
		mailerApiUrl,
		mailerApiSecret,
		jwtSecret,
	};
	cacheTime = now;

	c.set('secrets', cachedSecrets);
	await next();
});
