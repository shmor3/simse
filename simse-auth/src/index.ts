import { Hono } from 'hono';
import { analyticsMiddleware } from './middleware/analytics';
import { cleanupMiddleware } from './middleware/cleanup';
import { securityHeadersMiddleware } from './middleware/security';
import { serviceAuthMiddleware } from './middleware/service-auth';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import teams from './routes/teams';
import users from './routes/users';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

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
app.use('*', securityHeadersMiddleware);
app.use('*', cleanupMiddleware);
app.get('/health', (c) => c.json({ ok: true }));

// Auth routes (mostly public — gateway forwards without auth check)
// These specific auth routes require auth and must be gated
app.use('/auth/me', serviceAuthMiddleware);
app.use('/auth/logout', serviceAuthMiddleware);
app.route('/auth', auth);

// Protected routes — require gateway service auth to prevent direct access
app.use('/users/*', serviceAuthMiddleware);
app.use('/teams/*', serviceAuthMiddleware);
app.use('/api-keys/*', serviceAuthMiddleware);
app.route('/users', users);
app.route('/teams', teams);
app.route('/api-keys', apiKeys);

export default app;
