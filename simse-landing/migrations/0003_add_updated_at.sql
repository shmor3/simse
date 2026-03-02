ALTER TABLE waitlist ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'));
