export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
