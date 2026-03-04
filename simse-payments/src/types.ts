export interface Env {
	DB: D1Database;
	STRIPE_SECRET_KEY: string;
	STRIPE_WEBHOOK_SECRET: string;
	API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
}
