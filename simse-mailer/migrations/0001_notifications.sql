CREATE TABLE notifications (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  type TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  read INTEGER DEFAULT 0,
  link TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_notifications_user ON notifications(user_id, read);
