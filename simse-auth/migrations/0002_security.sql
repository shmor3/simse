-- Rate limiting
CREATE TABLE rate_limits (
  key TEXT NOT NULL,
  window TEXT NOT NULL,
  count INTEGER DEFAULT 1,
  PRIMARY KEY (key, window)
);

CREATE INDEX idx_rate_limits_window ON rate_limits(window);
