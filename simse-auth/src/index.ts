import { Hono } from 'hono';
import auth from './routes/auth';
import teams from './routes/teams';
import users from './routes/users';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.route('/auth', auth);
app.route('/users', users);
app.route('/teams', teams);

export default app;
