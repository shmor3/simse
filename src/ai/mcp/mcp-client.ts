import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import {
	createMCPConnectionError,
	createMCPError,
	createMCPServerNotConnectedError,
	createMCPToolError,
	createMCPTransportConfigError,
	isMCPServerNotConnectedError,
	isMCPTransportConfigError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type {
	MCPClientConfig,
	MCPPromptInfo,
	MCPResourceInfo,
	MCPServerConnection,
	MCPToolInfo,
	MCPToolResult,
} from './types.js';

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

interface ConnectedServer {
	config: MCPServerConnection;
	client: Client;
	transport: StdioClientTransport | StreamableHTTPClientTransport;
}

// ---------------------------------------------------------------------------
// MCPClient interface
// ---------------------------------------------------------------------------

export interface MCPClient {
	readonly connect: (serverName: string) => Promise<void>;
	readonly connectAll: () => Promise<string[]>;
	readonly disconnect: (serverName: string) => Promise<void>;
	readonly disconnectAll: () => Promise<void>;
	readonly isAvailable: (serverName?: string) => boolean;
	readonly connectionCount: number;
	readonly connectedServerNames: string[];
	readonly listTools: (serverName?: string) => Promise<MCPToolInfo[]>;
	readonly callTool: (
		serverName: string,
		toolName: string,
		args: Record<string, unknown>,
	) => Promise<MCPToolResult>;
	readonly listResources: (serverName?: string) => Promise<MCPResourceInfo[]>;
	readonly readResource: (serverName: string, uri: string) => Promise<string>;
	readonly listPrompts: (serverName?: string) => Promise<MCPPromptInfo[]>;
	readonly getPrompt: (
		serverName: string,
		promptName: string,
		args: Record<string, string>,
	) => Promise<string>;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createMCPClient(
	config: MCPClientConfig,
	loggerOpt?: Logger,
): MCPClient {
	const logger = (loggerOpt ?? getDefaultLogger()).child('mcp-client');
	const connections = new Map<string, ConnectedServer>();

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	const createTransport = (
		serverConfig: MCPServerConnection,
	): StdioClientTransport | StreamableHTTPClientTransport => {
		if (serverConfig.transport === 'stdio') {
			if (!serverConfig.command) {
				throw createMCPTransportConfigError(
					serverConfig.name,
					'stdio transport requires a "command" field',
				);
			}
			return new StdioClientTransport({
				command: serverConfig.command,
				args: serverConfig.args ? [...serverConfig.args] : [],
			});
		}

		if (!serverConfig.url) {
			throw createMCPTransportConfigError(
				serverConfig.name,
				'http transport requires a "url" field',
			);
		}

		let parsedUrl: URL;
		try {
			parsedUrl = new URL(serverConfig.url);
		} catch (error) {
			throw createMCPTransportConfigError(
				serverConfig.name,
				`Invalid URL "${serverConfig.url}": ${toError(error).message}`,
			);
		}

		if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
			throw createMCPTransportConfigError(
				serverConfig.name,
				`URL scheme "${parsedUrl.protocol}" is not allowed; only http and https are permitted`,
			);
		}

		return new StreamableHTTPClientTransport(parsedUrl);
	};

	const getConnection = (serverName: string): ConnectedServer => {
		const conn = connections.get(serverName);
		if (!conn) {
			throw createMCPServerNotConnectedError(serverName);
		}
		return conn;
	};

	const getTargetServers = (
		serverName?: string,
	): [string, ConnectedServer][] => {
		if (serverName) {
			return [[serverName, getConnection(serverName)]];
		}
		return [...connections.entries()];
	};

	// -----------------------------------------------------------------------
	// Connection lifecycle
	// -----------------------------------------------------------------------

	// Track in-flight connection attempts to prevent concurrent connect() races
	const connectingPromises = new Map<string, Promise<void>>();

	const connect = (serverName: string): Promise<void> => {
		// If a connection attempt is already in flight for this server, join it
		const inflight = connectingPromises.get(serverName);
		if (inflight) return inflight;

		const promise = doConnect(serverName).finally(() => {
			connectingPromises.delete(serverName);
		});
		connectingPromises.set(serverName, promise);
		return promise;
	};

	const doConnect = async (serverName: string): Promise<void> => {
		const serverConfig = config.servers.find((s) => s.name === serverName);
		if (!serverConfig) {
			throw createMCPConnectionError(
				serverName,
				`No MCP server configured with name "${serverName}"`,
			);
		}

		// Disconnect existing connection to prevent resource leaks
		if (connections.has(serverName)) {
			await disconnect(serverName);
		}

		logger.debug(`Connecting to MCP server "${serverName}"`, {
			transport: serverConfig.transport,
		});

		let transport: StdioClientTransport | StreamableHTTPClientTransport;
		try {
			transport = createTransport(serverConfig);
		} catch (error) {
			if (isMCPTransportConfigError(error)) throw error;
			throw createMCPConnectionError(
				serverName,
				`Failed to create transport: ${toError(error).message}`,
				{
					cause: error,
				},
			);
		}

		const client = new Client({
			name: config.clientName ?? 'simse-mcp-client',
			version: config.clientVersion ?? '1.0.0',
		});

		try {
			await client.connect(transport);
		} catch (error) {
			// Close the transport to prevent resource leak (orphaned child process, etc.)
			try {
				await transport.close?.();
			} catch (closeError) {
				logger.debug(`Error closing transport for "${serverName}"`, {
					error: toError(closeError).message,
				});
			}
			throw createMCPConnectionError(
				serverName,
				`Connection failed: ${toError(error).message}`,
				{ cause: error },
			);
		}

		connections.set(serverName, {
			config: serverConfig,
			client,
			transport,
		});
		logger.info(`Connected to MCP server "${serverName}"`);
	};

	const connectAll = async (): Promise<string[]> => {
		const results = await Promise.allSettled(
			config.servers.map(async (server) => {
				try {
					await connect(server.name);
				} catch (err) {
					logger.warn(`Failed to connect to MCP server "${server.name}"`, {
						error: toError(err).message,
					});
					throw err;
				}
			}),
		);

		return config.servers
			.filter((_, i) => results[i].status === 'fulfilled')
			.map((s) => s.name);
	};

	const disconnect = async (serverName: string): Promise<void> => {
		// If a connect is in-flight, wait for it to finish (or fail) first
		const inflight = connectingPromises.get(serverName);
		if (inflight) {
			try {
				await inflight;
			} catch {
				// connect failed — nothing to disconnect
			}
		}

		const conn = connections.get(serverName);
		if (!conn) return;

		logger.debug(`Disconnecting from MCP server "${serverName}"`);

		try {
			await conn.client.close();
		} catch (error) {
			logger.warn(`Error disconnecting from "${serverName}"`, {
				error: toError(error).message,
			});
		}

		connections.delete(serverName);
		logger.info(`Disconnected from MCP server "${serverName}"`);
	};

	const disconnectAll = async (): Promise<void> => {
		const names = [...connections.keys()];
		await Promise.all(names.map((name) => disconnect(name)));
	};

	const isAvailable = (serverName?: string): boolean => {
		if (serverName) {
			return connections.has(serverName);
		}
		return connections.size > 0;
	};

	// -----------------------------------------------------------------------
	// Tools
	// -----------------------------------------------------------------------

	const listTools = async (serverName?: string): Promise<MCPToolInfo[]> => {
		const servers = getTargetServers(serverName);
		const results: MCPToolInfo[] = [];

		for (const [name, conn] of servers) {
			try {
				const response = await conn.client.listTools();
				for (const tool of response.tools) {
					results.push({
						serverName: name,
						name: tool.name,
						description: tool.description,
						inputSchema: tool.inputSchema as Record<string, unknown>,
					});
				}
			} catch (error) {
				if (isMCPServerNotConnectedError(error)) throw error;
				throw createMCPError(
					`Failed to list tools from server "${name}": ${toError(error).message}`,
					{
						code: 'MCP_LIST_TOOLS_FAILED',
						cause: error,
						metadata: { serverName: name },
					},
				);
			}
		}

		return results;
	};

	const callTool = async (
		serverName: string,
		toolName: string,
		args: Record<string, unknown>,
	): Promise<MCPToolResult> => {
		const conn = getConnection(serverName);

		logger.debug(`Calling tool "${toolName}" on server "${serverName}"`, {
			argKeys: Object.keys(args),
		});

		let response: Awaited<ReturnType<Client['callTool']>>;
		try {
			response = await conn.client.callTool({
				name: toolName,
				arguments: args,
			});
		} catch (error) {
			throw createMCPToolError(
				serverName,
				toolName,
				`Tool call failed: ${toError(error).message}`,
				{ cause: error },
			);
		}

		const rawContent = (response.content ?? []) as Array<{
			type: string;
			text?: string;
			[key: string]: unknown;
		}>;

		const textParts = rawContent
			.filter(
				(item): item is typeof item & { text: string } =>
					item.type === 'text' &&
					typeof item.text === 'string' &&
					item.text.length > 0,
			)
			.map((item) => item.text);

		const result: MCPToolResult = {
			content: textParts.join('\n'),
			isError: response.isError === true,
			rawContent,
		};

		if (result.isError) {
			logger.warn(`Tool "${toolName}" on "${serverName}" returned an error`, {
				content: result.content.slice(0, 200),
			});
		} else {
			logger.debug(`Tool "${toolName}" completed successfully`, {
				contentLength: result.content.length,
			});
		}

		return result;
	};

	// -----------------------------------------------------------------------
	// Resources
	// -----------------------------------------------------------------------

	const listResources = async (
		serverName?: string,
	): Promise<MCPResourceInfo[]> => {
		const servers = getTargetServers(serverName);
		const results: MCPResourceInfo[] = [];

		for (const [name, conn] of servers) {
			try {
				const response = await conn.client.listResources();
				for (const res of response.resources) {
					results.push({
						serverName: name,
						uri: res.uri,
						name: res.name,
						description: res.description,
						mimeType: res.mimeType,
					});
				}
			} catch (error) {
				if (isMCPServerNotConnectedError(error)) throw error;
				throw createMCPError(
					`Failed to list resources from server "${name}": ${toError(error).message}`,
					{
						code: 'MCP_LIST_RESOURCES_FAILED',
						cause: error,
						metadata: { serverName: name },
					},
				);
			}
		}

		return results;
	};

	const readResource = async (
		serverName: string,
		uri: string,
	): Promise<string> => {
		const conn = getConnection(serverName);

		logger.debug(`Reading resource "${uri}" from server "${serverName}"`);

		let response: Awaited<ReturnType<Client['readResource']>>;
		try {
			response = await conn.client.readResource({ uri });
		} catch (error) {
			throw createMCPError(
				`Failed to read resource "${uri}" from server "${serverName}": ${toError(error).message}`,
				{
					code: 'MCP_READ_RESOURCE_FAILED',
					cause: error,
					metadata: { serverName, uri },
				},
			);
		}

		const first = response.contents[0];
		if (!first) return '';
		if ('text' in first) return first.text as string;
		return JSON.stringify(first);
	};

	// -----------------------------------------------------------------------
	// Prompts
	// -----------------------------------------------------------------------

	const listPrompts = async (serverName?: string): Promise<MCPPromptInfo[]> => {
		const servers = getTargetServers(serverName);
		const results: MCPPromptInfo[] = [];

		for (const [name, conn] of servers) {
			try {
				const response = await conn.client.listPrompts();
				for (const prompt of response.prompts) {
					results.push({
						serverName: name,
						name: prompt.name,
						description: prompt.description,
						arguments: prompt.arguments,
					});
				}
			} catch (error) {
				if (isMCPServerNotConnectedError(error)) throw error;
				throw createMCPError(
					`Failed to list prompts from server "${name}": ${toError(error).message}`,
					{
						code: 'MCP_LIST_PROMPTS_FAILED',
						cause: error,
						metadata: { serverName: name },
					},
				);
			}
		}

		return results;
	};

	const getPrompt = async (
		serverName: string,
		promptName: string,
		args: Record<string, string>,
	): Promise<string> => {
		const conn = getConnection(serverName);

		logger.debug(`Getting prompt "${promptName}" from server "${serverName}"`);

		let response: Awaited<ReturnType<Client['getPrompt']>>;
		try {
			response = await conn.client.getPrompt({
				name: promptName,
				arguments: args,
			});
		} catch (error) {
			throw createMCPError(
				`Failed to get prompt "${promptName}" from server "${serverName}": ${toError(error).message}`,
				{
					code: 'MCP_GET_PROMPT_FAILED',
					cause: error,
					metadata: { serverName, promptName },
				},
			);
		}

		return response.messages
			.map((msg) => {
				if (msg.content.type === 'text') return msg.content.text;
				return JSON.stringify(msg.content);
			})
			.join('\n');
	};

	// -----------------------------------------------------------------------
	// Return the record
	// -----------------------------------------------------------------------

	return Object.freeze({
		connect,
		connectAll,
		disconnect,
		disconnectAll,
		isAvailable,
		get connectionCount() {
			return connections.size;
		},
		get connectedServerNames() {
			return [...connections.keys()];
		},
		listTools,
		callTool,
		listResources,
		readResource,
		listPrompts,
		getPrompt,
	});
}
