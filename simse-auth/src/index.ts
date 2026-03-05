import { Hono } from 'hono';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import teams from './routes/teams';
import users from './routes/users';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

// Auth routes (public — gateway forwards without auth check)
app.route('/auth', auth);

// Protected routes (gateway validates token first, passes X-User-Id)
app.route('/users', users);
app.route('/teams', teams);
app.route('/api-keys', apiKeys);

export default app;
