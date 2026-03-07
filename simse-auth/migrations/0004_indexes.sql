-- Expiry indexes for cleanup queries
CREATE INDEX idx_tokens_expires ON tokens(expires_at);
CREATE INDEX idx_team_invites_expires ON team_invites(expires_at);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);

-- Composite index for cleanup of revoked/expired refresh tokens
CREATE INDEX idx_refresh_tokens_revoked_created ON refresh_tokens(revoked, created_at);

-- Team invite lookups by team (used in orphan cleanup and team deletion)
CREATE INDEX idx_team_invites_team ON team_invites(team_id);
