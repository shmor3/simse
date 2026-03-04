export interface Env {
	DB: D1Database;
	SESSION_SECRET: string;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
