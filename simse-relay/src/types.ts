export interface Env {
	TUNNEL_SESSION: DurableObjectNamespace;
	SECRETS?: { get(key: string): Promise<string | null> };
	ANALYTICS: AnalyticsEngineDataset;
}

export interface ApiSecrets {
	authApiUrl: string;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
