export interface Env {
	AUTH_API_URL: string;
	AUTH_API_SECRET: string;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	MAILER_API_URL: string;
	COMMS_QUEUE: Queue;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
