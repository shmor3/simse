import { env } from 'cloudflare:test';

await env.DB.exec(
	"CREATE TABLE IF NOT EXISTS notifications (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, type TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, read INTEGER DEFAULT 0, link TEXT, created_at TEXT DEFAULT (datetime('now')))",
);
await env.DB.exec(
	'CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_id, read)',
);
