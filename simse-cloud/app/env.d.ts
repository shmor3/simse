interface Env {
	DB: D1Database;
	SESSIONS: KVNamespace;
	STRIPE_SECRET_KEY: string;
	STRIPE_WEBHOOK_SECRET: string;
	EMAIL_API_URL: string;
	EMAIL_API_SECRET: string;
	SESSION_SECRET: string;
	APP_URL: string;
}
