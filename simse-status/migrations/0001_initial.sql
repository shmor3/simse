-- Initial schema for simse-status
CREATE TABLE checks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  service_id TEXT NOT NULL,
  status TEXT NOT NULL,
  response_time_ms INTEGER,
  status_code INTEGER,
  error TEXT,
  checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_checks_service_time ON checks(service_id, checked_at);
CREATE INDEX idx_checks_time ON checks(checked_at);
