// ---------------------------------------------------------------------------
// Server — Type definitions
// ---------------------------------------------------------------------------

import type { ACPServerEntry } from '../ai/acp/types.js';

/**
 * Configuration for creating a headless simse server.
 */
export interface SimseServerConfig {
	/** Port to listen on. Use 0 for random assignment. */
	readonly port?: number;
	/** Hostname to bind to. Defaults to '127.0.0.1'. */
	readonly host?: string;
	/** ACP server entries the server can connect to. */
	readonly acpServers: readonly ACPServerEntry[];
	/** Optional MCP server configurations. */
	readonly mcpServers?: readonly unknown[];
	/** Working directory for spawned processes. */
	readonly workingDirectory: string;
}

/**
 * A running headless simse server instance.
 */
export interface SimseServer {
	/** Start accepting connections. */
	readonly start: () => Promise<void>;
	/** Gracefully shut down the server. */
	readonly stop: () => Promise<void>;
	/** The port the server is listening on (resolved after start). */
	readonly port: number;
	/** The full URL the server is listening on (resolved after start). */
	readonly url: string;
}
