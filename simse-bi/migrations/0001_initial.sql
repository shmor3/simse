CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  action TEXT NOT NULL,
  user_id TEXT NOT NULL,
  metadata TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_audit_user ON audit_events(user_id);
CREATE INDEX idx_audit_action ON audit_events(action);
CREATE INDEX idx_audit_created ON audit_events(created_at);
