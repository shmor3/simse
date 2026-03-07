-- Drop unused two_factor_secret column (2FA uses email codes, not TOTP)
ALTER TABLE users DROP COLUMN two_factor_secret;
