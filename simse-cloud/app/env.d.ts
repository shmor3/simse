interface Env {
	APP_URL: string;
	ANALYTICS: AnalyticsEngineDataset;
	TUNNEL_SESSION: DurableObjectNamespace;
	SECRETS?: { get(key: string): Promise<string | null> };
}
