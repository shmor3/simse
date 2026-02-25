/**
 * SimSE — Tool Service
 *
 * Wraps MCP client and server with convenience methods for tool
 * discovery, invocation, resource reading, and prompt expansion.
 */

import {
	type ACPClient,
	createMCPClient,
	createMCPServer,
	type Logger,
	type MCPClient,
	type MCPClientConfig,
	type MCPPromptInfo,
	type MCPResourceInfo,
	type MCPServerConfig,
	type MCPToolInfo,
	type MCPToolResult,
	type SimseMCPServer,
} from 'simse';

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface ToolServiceOptions {
	readonly mcpClientConfig: MCPClientConfig;
	readonly mcpServerConfig: MCPServerConfig;
	readonly acpClient: ACPClient;
	readonly logger: Logger;
}

export interface ToolService {
	/** The underlying MCP client. */
	readonly mcpClient: MCPClient;
	/** Connect to a specific MCP server by name. */
	readonly connect: (serverName: string) => Promise<void>;
	/** Connect to all configured MCP servers. Returns names of those connected. */
	readonly connectAll: () => Promise<string[]>;
	/** Disconnect all MCP client connections. */
	readonly disconnect: () => Promise<void>;
	/** List available tools, optionally filtered by server. */
	readonly listTools: (serverName?: string) => Promise<MCPToolInfo[]>;
	/** Call a tool on a specific server. */
	readonly callTool: (
		serverName: string,
		toolName: string,
		args: Record<string, unknown>,
	) => Promise<MCPToolResult>;
	/** List available resources, optionally filtered by server. */
	readonly listResources: (serverName?: string) => Promise<MCPResourceInfo[]>;
	/** Read a resource from a specific server. */
	readonly readResource: (serverName: string, uri: string) => Promise<string>;
	/** List available prompts, optionally filtered by server. */
	readonly listPrompts: (serverName?: string) => Promise<MCPPromptInfo[]>;
	/** Expand a prompt template with arguments. */
	readonly getPrompt: (
		serverName: string,
		promptName: string,
		args: Record<string, string>,
	) => Promise<string>;
	/** Create the built-in MCP server (exposes simse tools over stdio). */
	readonly createServer: () => SimseMCPServer;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createToolService(options: ToolServiceOptions): ToolService {
	const mcpClient = createMCPClient(options.mcpClientConfig, options.logger);

	return Object.freeze({
		mcpClient,
		connect: (serverName: string) => mcpClient.connect(serverName),
		connectAll: () => mcpClient.connectAll(),
		disconnect: () => mcpClient.disconnectAll(),
		listTools: (serverName?: string) => mcpClient.listTools(serverName),
		callTool: (
			serverName: string,
			toolName: string,
			args: Record<string, unknown>,
		) => mcpClient.callTool(serverName, toolName, args),
		listResources: (serverName?: string) => mcpClient.listResources(serverName),
		readResource: (serverName: string, uri: string) =>
			mcpClient.readResource(serverName, uri),
		listPrompts: (serverName?: string) => mcpClient.listPrompts(serverName),
		getPrompt: (
			serverName: string,
			promptName: string,
			args: Record<string, string>,
		) => mcpClient.getPrompt(serverName, promptName, args),
		createServer: () =>
			createMCPServer(options.mcpServerConfig, {
				acpClient: options.acpClient,
			}),
	});
}
