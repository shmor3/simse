import { Hono } from 'hono';
import { authMiddleware } from './middleware/auth';
import checkout from './routes/checkout';
import credits from './routes/credits';
import customers from './routes/customers';
import portal from './routes/portal';
import subscriptions from './routes/subscriptions';
import webhooks from './routes/webhooks';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

// Webhooks — no auth (Stripe signature verification instead)
app.route('/webhooks', webhooks);

// Authenticated routes
app.use('/customers', authMiddleware);
app.use('/customers/*', authMiddleware);
app.use('/checkout', authMiddleware);
app.use('/portal', authMiddleware);
app.use('/subscriptions/*', authMiddleware);
app.use('/credits', authMiddleware);
app.use('/credits/*', authMiddleware);

app.route('/customers', customers);
app.route('/checkout', checkout);
app.route('/portal', portal);
app.route('/subscriptions', subscriptions);
app.route('/credits', credits);

export default app;
