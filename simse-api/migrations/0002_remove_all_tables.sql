-- Only run after data is migrated to simse-auth-db and simse-mailer-db
DROP TABLE IF EXISTS api_keys;
DROP TABLE IF EXISTS team_invites;
DROP TABLE IF EXISTS team_members;
DROP TABLE IF EXISTS teams;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS users;
