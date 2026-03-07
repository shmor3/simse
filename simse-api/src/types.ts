/**
 * Cloudflare Secrets Store binding — not yet in @cloudflare/workers-types.
 * A secrets_store binding exposes a `.get(name)` method that resolves the
 * named secret from the configured store.
 */
export interface SecretsStoreNamespace {
	get(name: string): Promise<string>;
}

export interface Env {
	COMMS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
	CLOUD_SERVICE: Fetcher;
}

export interface AppVariables {
	secrets: ApiSecrets;
	requestId: string;
}

export interface ApiSecrets {
	authApiUrl: string;
	authApiSecret: string;
	paymentsApiUrl: string;
	paymentsApiSecret: string;
	mailerApiUrl: string;
	mailerApiSecret: string;
	jwtSecret: string;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
