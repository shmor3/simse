export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS: AnalyticsEngineDataset;
	SECRETS: SecretsStoreNamespace;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
