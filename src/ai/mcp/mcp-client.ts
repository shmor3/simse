// ---------------------------------------------------------------------------
// MCP Client — thin wrapper over Rust MCP engine
// ---------------------------------------------------------------------------

import { createMCPConnectionError, toError } from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { HealthSnapshot } from '../../utils/health-monitor.js';
import { createMcpEngineClient } from './mcp-engine-client.js';
import type {
	MCPClientConfig,
	MCPCompletionRef,
	MCPCompletionResult,
	MCPLoggingLevel,
	MCPLoggingMessage,
	MCPPromptInfo,
	MCPResourceInfo,
	MCPResourceTemplateInfo,
	MCPRoot,
	MCPToolInfo,
	MCPToolResult,
} from './types.js';

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
	readonly listResourceTemplates: (
		serverName?: string,
	) => Promise<MCPResourceTemplateInfo[]>;
	readonly listPrompts: (serverName?: string) => Promise<MCPPromptInfo[]>;
	readonly getPrompt: (
		serverName: string,
		promptName: string,
		args: Record<string, string>,
	) => Promise<string>;
	readonly setLoggingLevel: (
		serverName: string,
		level: MCPLoggingLevel,
	) => Promise<void>;
	readonly onLoggingMessage: (
		handler: (message: MCPLoggingMessage & { serverName: string }) => void,
	) => () => void;
	readonly onToolsChanged: (
		handler: (serverName: string) => void,
	) => () => void;
	readonly onResourcesChanged: (
		handler: (serverName: string) => void,
	) => () => void;
	readonly onPromptsChanged: (
		handler: (serverName: string) => void,
	) => () => void;
	readonly complete: (
		serverName: string,
		ref: MCPCompletionRef,
		argument: { name: string; value: string },
	) => Promise<MCPCompletionResult>;
	readonly sendRootsListChanged: () => Promise<void>;
	readonly setRoots: (roots: MCPRoot[]) => void;
	readonly roots: readonly MCPRoot[];
	readonly getServerHealth: (serverName: string) => HealthSnapshot | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an MCP client that connects to one or more MCP servers
 * via the Rust MCP engine subprocess.
 *
 * @param config - Server definitions with connection details.
 * @param loggerOpt - Optional logger (defaults to the global logger).
 * @returns A frozen {@link MCPClient} with tool/resource/prompt discovery and invocation.
 * @throws {MCPConnectionError} When a server connection fails.
 */
export function createMCPClient(
	config: MCPClientConfig,
	loggerOpt?: Logger,
): MCPClient {
	const logger = (loggerOpt ?? getDefaultLogger()).child('mcp-client');

	// Track connected server names locally for isAvailable / connectionCount
	const connectedServers = new Set<string>();

	// Notification handler registries (local)
	const loggingHandlers = new Set<
		(message: MCPLoggingMessage & { serverName: string }) => void
	>();
	const toolsChangedHandlers = new Set<(serverName: string) => void>();
	const resourcesChangedHandlers = new Set<(serverName: string) => void>();
	const promptsChangedHandlers = new Set<(serverName: string) => void>();
	let currentRoots: MCPRoot[] = [];

	// -----------------------------------------------------------------------
	// Engine client
	// -----------------------------------------------------------------------

	const enginePath = process.env.SIMSE_MCP_ENGINE_PATH ?? 'simse-mcp-engine';
	const engineClient = createMcpEngineClient({
		enginePath,
		logger,
	});

	// Initialize the engine with client config
	const initPromise = engineClient
		.request<void>('mcp/initialize', {
			clientConfig: {
				servers: config.servers.map((s) => ({
					name: s.name,
					transport: s.transport,
					command: s.command,
					args: s.args ? [...s.args] : undefined,
					env: s.env,
					url: s.url,
				})),
				clientName: config.clientName,
				clientVersion: config.clientVersion,
				circuitBreaker: config.circuitBreaker,
			},
		})
		.catch((err) => {
			logger.warn('MCP engine initialization failed', {
				error: toError(err).message,
			});
		});

	// -----------------------------------------------------------------------
	// Subscribe to engine notifications
	// -----------------------------------------------------------------------

	engineClient.onNotification('mcp/loggingMessage', (params) => {
		const p = params as {
			serverName: string;
			level: MCPLoggingLevel;
			logger?: string;
			data: unknown;
		};
		for (const handler of loggingHandlers) {
			handler({
				level: p.level,
				logger: p.logger,
				data: p.data,
				serverName: p.serverName,
			});
		}
	});

	engineClient.onNotification('mcp/toolsChanged', (params) => {
		const p = params as { serverName: string };
		for (const handler of toolsChangedHandlers) handler(p.serverName);
	});

	engineClient.onNotification('mcp/resourcesChanged', (params) => {
		const p = params as { serverName: string };
		for (const handler of resourcesChangedHandlers) handler(p.serverName);
	});

	engineClient.onNotification('mcp/promptsChanged', (params) => {
		const p = params as { serverName: string };
		for (const handler of promptsChangedHandlers) handler(p.serverName);
	});

	// -----------------------------------------------------------------------
	// Connection lifecycle
	// -----------------------------------------------------------------------

	const connect = async (serverName: string): Promise<void> => {
		await initPromise;

		const serverConfig = config.servers.find((s) => s.name === serverName);
		if (!serverConfig) {
			throw createMCPConnectionError(
				serverName,
				`No MCP server configured with name "${serverName}"`,
			);
		}

		await engineClient.request<void>('mcp/connect', { serverName });
		connectedServers.add(serverName);
		logger.info(`Connected to MCP server "${serverName}"`);
	};

	const connectAll = async (): Promise<string[]> => {
		await initPromise;

		const result = await engineClient.request<{ connected: string[] }>(
			'mcp/connectAll',
			{},
		);
		for (const name of result.connected) {
			connectedServers.add(name);
		}
		logger.info(`Connected to ${result.connected.length} MCP servers`);
		return result.connected;
	};

	const disconnect = async (serverName: string): Promise<void> => {
		await engineClient.request<void>('mcp/disconnect', { serverName });
		connectedServers.delete(serverName);
		logger.info(`Disconnected from MCP server "${serverName}"`);
	};

	const disconnectAll = async (): Promise<void> => {
		await engineClient.request<void>('mcp/disconnectAll', {});
		connectedServers.clear();
		logger.info('Disconnected from all MCP servers');
	};

	const isAvailable = (serverName?: string): boolean => {
		if (serverName) {
			return connectedServers.has(serverName);
		}
		return connectedServers.size > 0;
	};

	// -----------------------------------------------------------------------
	// Tools
	// -----------------------------------------------------------------------

	const listTools = async (serverName?: string): Promise<MCPToolInfo[]> => {
		return engineClient.request<MCPToolInfo[]>('mcp/listTools', {
			serverName,
		});
	};

	const callTool = async (
		serverName: string,
		toolName: string,
		args: Record<string, unknown>,
	): Promise<MCPToolResult> => {
		return engineClient.request<MCPToolResult>('mcp/callTool', {
			serverName,
			toolName,
			args,
		});
	};

	// -----------------------------------------------------------------------
	// Resources
	// -----------------------------------------------------------------------

	const listResources = async (
		serverName?: string,
	): Promise<MCPResourceInfo[]> => {
		return engineClient.request<MCPResourceInfo[]>('mcp/listResources', {
			serverName,
		});
	};

	const readResource = async (
		serverName: string,
		uri: string,
	): Promise<string> => {
		return engineClient.request<string>('mcp/readResource', {
			serverName,
			uri,
		});
	};

	const listResourceTemplates = async (
		serverName?: string,
	): Promise<MCPResourceTemplateInfo[]> => {
		return engineClient.request<MCPResourceTemplateInfo[]>(
			'mcp/listResourceTemplates',
			{ serverName },
		);
	};

	// -----------------------------------------------------------------------
	// Prompts
	// -----------------------------------------------------------------------

	const listPrompts = async (serverName?: string): Promise<MCPPromptInfo[]> => {
		return engineClient.request<MCPPromptInfo[]>('mcp/listPrompts', {
			serverName,
		});
	};

	const getPrompt = async (
		serverName: string,
		promptName: string,
		args: Record<string, string>,
	): Promise<string> => {
		return engineClient.request<string>('mcp/getPrompt', {
			serverName,
			promptName,
			args,
		});
	};

	// -----------------------------------------------------------------------
	// Logging
	// -----------------------------------------------------------------------

	const setLoggingLevel = async (
		serverName: string,
		level: MCPLoggingLevel,
	): Promise<void> => {
		await engineClient.request<void>('mcp/setLoggingLevel', {
			serverName,
			level,
		});
	};

	const onLoggingMessage = (
		handler: (message: MCPLoggingMessage & { serverName: string }) => void,
	): (() => void) => {
		loggingHandlers.add(handler);
		return () => {
			loggingHandlers.delete(handler);
		};
	};

	// -----------------------------------------------------------------------
	// List-changed notifications
	// -----------------------------------------------------------------------

	const onToolsChanged = (
		handler: (serverName: string) => void,
	): (() => void) => {
		toolsChangedHandlers.add(handler);
		return () => {
			toolsChangedHandlers.delete(handler);
		};
	};

	const onResourcesChanged = (
		handler: (serverName: string) => void,
	): (() => void) => {
		resourcesChangedHandlers.add(handler);
		return () => {
			resourcesChangedHandlers.delete(handler);
		};
	};

	const onPromptsChanged = (
		handler: (serverName: string) => void,
	): (() => void) => {
		promptsChangedHandlers.add(handler);
		return () => {
			promptsChangedHandlers.delete(handler);
		};
	};

	// -----------------------------------------------------------------------
	// Completions
	// -----------------------------------------------------------------------

	const complete = async (
		serverName: string,
		ref: MCPCompletionRef,
		argument: { name: string; value: string },
	): Promise<MCPCompletionResult> => {
		return engineClient.request<MCPCompletionResult>('mcp/complete', {
			serverName,
			ref,
			argument,
		});
	};

	// -----------------------------------------------------------------------
	// Roots
	// -----------------------------------------------------------------------

	const getServerHealth = (_serverName: string): HealthSnapshot | undefined => {
		// Health is now managed in the Rust engine; delegate
		// For now return undefined since health snapshots are engine-internal
		return undefined;
	};

	const setRoots = (roots: MCPRoot[]): void => {
		currentRoots = [...roots];
		// Fire and forget — inform the engine of new roots
		engineClient.request<void>('mcp/setRoots', { roots }).catch(() => {
			// ignore errors
		});
	};

	const sendRootsListChanged = async (): Promise<void> => {
		await engineClient.request<void>('mcp/sendRootsListChanged', {});
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
			return connectedServers.size;
		},
		get connectedServerNames() {
			return [...connectedServers];
		},
		listTools,
		callTool,
		listResources,
		readResource,
		listResourceTemplates,
		listPrompts,
		getPrompt,
		setLoggingLevel,
		onLoggingMessage,
		onToolsChanged,
		onResourcesChanged,
		onPromptsChanged,
		complete,
		getServerHealth,
		sendRootsListChanged,
		setRoots,
		get roots() {
			return [...currentRoots];
		},
	});
}
