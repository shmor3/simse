export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
