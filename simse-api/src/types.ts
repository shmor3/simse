export interface Env {
	COMMS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
}

export interface ApiSecrets {
	authApiUrl: string;
	authApiSecret: string;
	paymentsApiUrl: string;
	paymentsApiSecret: string;
	mailerApiUrl: string;
	mailerApiSecret: string;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
