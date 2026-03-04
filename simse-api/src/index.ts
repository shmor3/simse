import { Hono } from 'hono';
import { authMiddleware } from './middleware/auth';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import gateway from './routes/gateway';
import notifications from './routes/notifications';
import teams from './routes/teams';
import users from './routes/users';
import type { AuthContext, Env } from './types';

const app = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// Health check
app.get('/health', (c) => c.json({ ok: true }));

// Public auth routes (no middleware)
app.route('/auth', auth);

// Auth middleware for all protected routes
app.use('/users/*', authMiddleware);
app.use('/teams/*', authMiddleware);
app.use('/notifications', authMiddleware);
app.use('/notifications/*', authMiddleware);
app.use('/api-keys', authMiddleware);
app.use('/api-keys/*', authMiddleware);
app.use('/payments/*', authMiddleware);
app.use('/emails/*', authMiddleware);

// Logout needs auth context
app.use('/auth/logout', authMiddleware);
app.use('/auth/me', authMiddleware);

// Protected routes
app.route('/users', users);
app.route('/teams', teams);
app.route('/notifications', notifications);
app.route('/api-keys', apiKeys);

// Gateway
app.route('', gateway);

export default app;
