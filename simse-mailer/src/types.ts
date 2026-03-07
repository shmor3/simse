/**
 * Cloudflare Secrets Store binding — not yet in @cloudflare/workers-types.
 * A secrets_store binding exposes a `.get(name)` method that resolves the
 * named secret from the configured store.
 */
export interface SecretsStoreNamespace {
	get(name: string): Promise<string>;
}
