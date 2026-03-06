export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS: AnalyticsEngineDataset;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
