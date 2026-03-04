interface Env {
	DB: D1Database;
	SESSIONS: KVNamespace;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	EMAIL_API_URL: string;
	EMAIL_API_SECRET: string;
	SESSION_SECRET: string;
	APP_URL: string;
}
