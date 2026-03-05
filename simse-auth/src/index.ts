import { Hono } from 'hono';
import auth from './routes/auth';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.route('/auth', auth);

export default app;
