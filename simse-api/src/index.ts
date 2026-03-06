import { Hono } from 'hono';
import { analyticsMiddleware } from './middleware/analytics';
import { secretsMiddleware } from './middleware/secrets';
import gateway from './routes/gateway';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.use('*', analyticsMiddleware);
app.get('/health', (c) => c.json({ ok: true }));
app.use('*', secretsMiddleware);
app.route('', gateway);

export default app;
