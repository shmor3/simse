// ---------------------------------------------------------------------------
// MCP module — barrel re-export
// ---------------------------------------------------------------------------

export type { MCPClient } from './mcp-client.js';
export { createMCPClient } from './mcp-client.js';

export type {
	McpEngineClient,
	McpEngineClientOptions,
} from './mcp-engine-client.js';
export { createMcpEngineClient } from './mcp-engine-client.js';

export type { MCPServerOptions, SimseMCPServer } from './mcp-server.js';
export { createMCPServer } from './mcp-server.js';

export type {
	MCPClientConfig,
	MCPCompletionRef,
	MCPCompletionRequest,
	MCPCompletionResult,
	MCPLoggingLevel,
	MCPLoggingMessage,
	MCPPromptInfo,
	MCPResourceInfo,
	MCPResourceSubscription,
	MCPResourceTemplateInfo,
	MCPRoot,
	MCPServerConfig,
	MCPServerConnection,
	MCPToolAnnotations,
	MCPToolCallMetrics,
	MCPToolInfo,
	MCPToolResult,
} from './types.js';
