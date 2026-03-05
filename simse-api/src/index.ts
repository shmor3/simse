import { Hono } from 'hono';
import gateway from './routes/gateway';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.route('', gateway);

export default app;
