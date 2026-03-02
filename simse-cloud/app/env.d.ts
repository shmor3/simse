interface Env {
	DB: D1Database;
	SESSIONS: KVNamespace;
	STRIPE_SECRET_KEY: string;
	STRIPE_WEBHOOK_SECRET: string;
	RESEND_API_KEY: string;
	SESSION_SECRET: string;
	APP_URL: string;
}
