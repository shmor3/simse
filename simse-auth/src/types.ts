/**
 * Secrets Store binding — not yet in @cloudflare/workers-types.
 * A Secrets Store namespace exposes `.get(name)` to retrieve individual secrets.
 */
interface SecretsStoreNamespace {
	get(name: string): Promise<string>;
}

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
