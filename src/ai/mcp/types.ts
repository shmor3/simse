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
	/** For http: the server URL (e.g. "http://localhost:3000/mcp"). */
	readonly url?: string;
}

export interface MCPClientConfig {
	/** External MCP servers to connect to. */
	readonly servers: readonly MCPServerConnection[];
	/** Client name advertised during MCP handshake. Defaults to `'simse-mcp-client'`. */
	readonly clientName?: string;
	/** Client version advertised during MCP handshake. Defaults to `'1.0.0'`. */
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

export interface MCPToolResult {
	/** Concatenated text content from the tool response. */
	readonly content: string;
	/** Whether the tool call resulted in an error. */
	readonly isError: boolean;
	/** Raw content items from the MCP response. */
	readonly rawContent: ReadonlyArray<
		Readonly<{ type: string; text?: string; [key: string]: unknown }>
	>;
}

export interface MCPToolInfo {
	readonly serverName: string;
	readonly name: string;
	readonly description?: string;
	readonly inputSchema?: Readonly<Record<string, unknown>>;
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
