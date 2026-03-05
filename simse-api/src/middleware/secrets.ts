import { createMiddleware } from 'hono/factory';
import type { Env, ApiSecrets } from '../types';

export const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
	const [authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl, mailerApiSecret] =
		await Promise.all([
			c.env.SECRETS.get('AUTH_API_URL'),
			c.env.SECRETS.get('AUTH_API_SECRET'),
			c.env.SECRETS.get('PAYMENTS_API_URL'),
			c.env.SECRETS.get('PAYMENTS_API_SECRET'),
			c.env.SECRETS.get('MAILER_API_URL'),
			c.env.SECRETS.get('MAILER_API_SECRET'),
		]);

	if (!authApiUrl || !authApiSecret || !paymentsApiUrl || !paymentsApiSecret || !mailerApiUrl || !mailerApiSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	c.set('secrets', { authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl, mailerApiSecret });
	await next();
});
