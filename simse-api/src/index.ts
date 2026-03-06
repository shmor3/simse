import { Hono } from 'hono';
import { CircuitBreaker } from './lib/circuit-breaker';
import { analyticsMiddleware } from './middleware/analytics';
import { rateLimitMiddleware } from './middleware/rate-limit';
import { secretsMiddleware } from './middleware/secrets';
import { requestValidationMiddleware } from './middleware/security';
import gateway from './routes/gateway';
import type { AppVariables, Env } from './types';

// Per-backend circuit breakers (shared across requests within worker instance)
export const breakers = {
	auth: new CircuitBreaker('auth'),
	payments: new CircuitBreaker('payments'),
	mailer: new CircuitBreaker('mailer'),
};

const app = new Hono<{ Bindings: Env; Variables: AppVariables }>();

app.onError((err, c) => {
	console.error('Unhandled error', err);
	return c.json(
		{
			error: {
				code: 'INTERNAL_ERROR',
				message: 'An unexpected error occurred',
			},
		},
		500,
	);
});

app.notFound((c) => {
	return c.json(
		{ error: { code: 'NOT_FOUND', message: 'Route not found' } },
		404,
	);
});

app.use('*', analyticsMiddleware);
app.use('*', requestValidationMiddleware);
app.use('*', rateLimitMiddleware);

app.get('/health', (c) => {
	return c.json({
		ok: true,
		services: {
			auth: breakers.auth.getStatus(),
			payments: breakers.payments.getStatus(),
			mailer: breakers.mailer.getStatus(),
		},
	});
});

app.use('*', secretsMiddleware);
app.route('', gateway);

export default app;
