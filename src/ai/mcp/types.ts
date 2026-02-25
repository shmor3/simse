// ---------------------------------------------------------------------------
// MCP Server Connection Configuration
// ---------------------------------------------------------------------------
//
// All types are strictly readonly to enforce immutability throughout
// the codebase.  No classes — only plain data interfaces.
// ---------------------------------------------------------------------------

export interface MCPServerConnection {
	/** Unique name identifying this MCP server connection. */
	readonly name: string;
	/** Transport type: stdio spawns a child process, http connects over HTTP. */
	readonly transport: 'stdio' | 'http';
	/** For stdio: the command to execute (e.g. "node"). */
	readonly command?: string;
	/** For stdio: arguments to the command (e.g. ["server.js"]). */
	readonly args?: readonly string[];
	/** For stdio: environment variables passed to the child process. */
	readonly env?: Readonly<Record<string, string>>;
	/** For http: the server URL (e.g. "http://localhost:3000/mcp"). */
	readonly url?: string;
}

export interface MCPClientConfig {
	/** External MCP servers to connect to. */
	readonly servers: readonly MCPServerConnection[];
	/** Client name advertised during MCP handshake. Required when connecting to servers. */
	readonly clientName?: string;
	/** Client version advertised during MCP handshake. Required when connecting to servers. */
	readonly clientVersion?: string;
}

export interface MCPServerConfig {
	/** Whether to start the built-in MCP server. */
	readonly enabled: boolean;
	/** Transport type for the server. */
	readonly transport: 'stdio';
	/** Server name advertised in MCP handshake. */
	readonly name: string;
	/** Server version advertised in MCP handshake. */
	readonly version: string;
}

// ---------------------------------------------------------------------------
// MCP Discovery / Result Types
// ---------------------------------------------------------------------------

export interface MCPToolCallMetrics {
	/** Wall-clock duration of the tool call in milliseconds. */
	readonly durationMs: number;
	/** Name of the MCP server that handled the call. */
	readonly serverName: string;
	/** Name of the tool that was called. */
	readonly toolName: string;
	/** ISO-8601 timestamp when the call started. */
	readonly startedAt: string;
}

export interface MCPToolResult {
	/** Concatenated text content from the tool response. */
	readonly content: string;
	/** Whether the tool call resulted in an error. */
	readonly isError: boolean;
	/** Raw content items from the MCP response. */
	readonly rawContent: ReadonlyArray<
		Readonly<{ type: string; text?: string; [key: string]: unknown }>
	>;
	/** Timing and identification metrics for this tool call. */
	readonly metrics: MCPToolCallMetrics;
}

export interface MCPToolAnnotations {
	readonly title?: string;
	readonly readOnlyHint?: boolean;
	readonly destructiveHint?: boolean;
	readonly idempotentHint?: boolean;
	readonly openWorldHint?: boolean;
}

export interface MCPToolInfo {
	readonly serverName: string;
	readonly name: string;
	readonly description?: string;
	readonly inputSchema?: Readonly<Record<string, unknown>>;
	readonly annotations?: MCPToolAnnotations;
}

export interface MCPResourceInfo {
	readonly serverName: string;
	readonly uri: string;
	readonly name?: string;
	readonly description?: string;
	readonly mimeType?: string;
}

export interface MCPPromptInfo {
	readonly serverName: string;
	readonly name: string;
	readonly description?: string;
	readonly arguments?: ReadonlyArray<
		Readonly<{ name: string; description?: string; required?: boolean }>
	>;
}

// ---------------------------------------------------------------------------
// Logging types
// ---------------------------------------------------------------------------

export type MCPLoggingLevel =
	| 'debug'
	| 'info'
	| 'notice'
	| 'warning'
	| 'error'
	| 'critical'
	| 'alert'
	| 'emergency';

export interface MCPLoggingMessage {
	readonly level: MCPLoggingLevel;
	readonly logger?: string;
	readonly data: unknown;
}

// ---------------------------------------------------------------------------
// Completion types
// ---------------------------------------------------------------------------

export interface MCPCompletionRequest {
	readonly ref: MCPCompletionRef;
	readonly argument: {
		readonly name: string;
		readonly value: string;
	};
}

export type MCPCompletionRef =
	| { readonly type: 'ref/resource'; readonly uri: string }
	| { readonly type: 'ref/prompt'; readonly name: string };

export interface MCPCompletionResult {
	readonly values: readonly string[];
	readonly hasMore?: boolean;
	readonly total?: number;
}

// ---------------------------------------------------------------------------
// Root types
// ---------------------------------------------------------------------------

export interface MCPRoot {
	readonly uri: string;
	readonly name?: string;
}

// ---------------------------------------------------------------------------
// Resource subscription types
// ---------------------------------------------------------------------------

export interface MCPResourceSubscription {
	readonly uri: string;
	readonly serverName: string;
}

// ---------------------------------------------------------------------------
// Resource template types
// ---------------------------------------------------------------------------

export interface MCPResourceTemplateInfo {
	readonly serverName: string;
	readonly uriTemplate: string;
	readonly name?: string;
	readonly description?: string;
	readonly mimeType?: string;
}
