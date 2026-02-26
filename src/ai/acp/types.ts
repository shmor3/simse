// ---------------------------------------------------------------------------
// Agent Client Protocol (ACP) Types — JSON-RPC 2.0 over stdio
// ---------------------------------------------------------------------------
//
// Native ACP types following the Agent Client Protocol specification
// (agentclientprotocol.com). All types are strictly readonly.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 framing
// ---------------------------------------------------------------------------

export interface JsonRpcRequest {
	readonly jsonrpc: '2.0';
	readonly id: number;
	readonly method: string;
	readonly params?: unknown;
}

export interface JsonRpcResponse {
	readonly jsonrpc: '2.0';
	readonly id: number;
	readonly result?: unknown;
	readonly error?: JsonRpcError;
}

export interface JsonRpcNotification {
	readonly jsonrpc: '2.0';
	readonly method: string;
	readonly params?: unknown;
}

export interface JsonRpcError {
	readonly code: number;
	readonly message: string;
	readonly data?: unknown;
}

export type JsonRpcMessage =
	| JsonRpcRequest
	| JsonRpcResponse
	| JsonRpcNotification;

// ---------------------------------------------------------------------------
// ACP protocol — initialize
// ---------------------------------------------------------------------------

export interface ACPClientInfo {
	readonly name: string;
	readonly version: string;
}

export interface ACPInitializeParams {
	readonly client_info: ACPClientInfo;
	readonly capabilities?: Readonly<Record<string, unknown>>;
}

export interface ACPServerInfo {
	readonly name: string;
	readonly version: string;
}

export interface ACPAgentCapabilities {
	readonly loadSession?: boolean;
	readonly promptCapabilities?: Readonly<Record<string, unknown>>;
	readonly sessionCapabilities?: Readonly<Record<string, unknown>>;
	readonly mcpCapabilities?: Readonly<Record<string, unknown>>;
}

export interface ACPInitializeResult {
	readonly protocolVersion: number;
	readonly agentInfo: ACPServerInfo;
	readonly agentCapabilities?: ACPAgentCapabilities;
	readonly authMethods?: readonly Readonly<Record<string, unknown>>[];
}

// ---------------------------------------------------------------------------
// ACP protocol — sessions
// ---------------------------------------------------------------------------

export interface ACPSessionNewParams {
	readonly cwd: string;
	readonly mcpServers: readonly unknown[];
}

export type ACPSessionNewResult = ACPSessionInfo;

// ---------------------------------------------------------------------------
// ACP protocol — content blocks
// ---------------------------------------------------------------------------

export interface ACPTextContent {
	readonly type: 'text';
	readonly text: string;
}

export interface ACPResourceLinkContent {
	readonly type: 'resource_link';
	readonly uri: string;
	readonly name: string;
	readonly mimeType?: string;
	readonly title?: string;
	readonly description?: string;
}

export interface ACPResourceContent {
	readonly type: 'resource';
	readonly resource: {
		readonly uri: string;
		readonly mimeType?: string;
		readonly text?: string;
		readonly blob?: string;
	};
}

/** @deprecated Non-standard content block. Use ACPTextContent or ACPResourceContent instead. */
export interface ACPDataContent {
	readonly type: 'data';
	readonly data: unknown;
	readonly mimeType?: string;
}

export type ACPContentBlock =
	| ACPTextContent
	| ACPResourceLinkContent
	| ACPResourceContent
	| ACPDataContent;

// ---------------------------------------------------------------------------
// ACP protocol — prompt (generation)
// ---------------------------------------------------------------------------

export type ACPStopReason =
	| 'end_turn'
	| 'max_tokens'
	| 'max_turn_requests'
	| 'refusal'
	| 'cancelled'
	| 'stop_sequence'
	| 'tool_use';

export interface ACPSessionPromptParams {
	readonly sessionId: string;
	readonly prompt: readonly ACPContentBlock[];
}

export interface ACPSessionPromptResult {
	readonly content?: readonly ACPContentBlock[];
	readonly stopReason?: ACPStopReason;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// ACP protocol — session update notifications (streaming)
// ---------------------------------------------------------------------------

export interface ACPSessionUpdate {
	readonly sessionUpdate: string;
	readonly content?: unknown;
	readonly metadata?: unknown;
	readonly [key: string]: unknown;
}

export interface ACPSessionUpdateParams {
	readonly sessionId: string;
	readonly update?: ACPSessionUpdate;
}

// ---------------------------------------------------------------------------
// ACP protocol — permission requests
// ---------------------------------------------------------------------------

export interface ACPPermissionRequestParams {
	readonly sessionId: string;
	readonly description: string;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

export interface ACPPermissionRequestResult {
	readonly allowed: boolean;
}

export type ACPPermissionPolicy = 'auto-approve' | 'prompt' | 'deny';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface ACPServerEntry {
	/** Friendly name for this ACP server connection. */
	readonly name: string;
	/** Command to spawn this server (required — ACP uses stdio). */
	readonly command: string;
	/** Arguments for the command. */
	readonly args?: readonly string[];
	/** Working directory for the spawned command. */
	readonly cwd?: string;
	/** Environment variables to pass to the spawned process. */
	readonly env?: Readonly<Record<string, string>>;
	/** Default agent ID to use when none is specified per-step. */
	readonly defaultAgent?: string;
	/** Request timeout in milliseconds. Defaults to 30 000. */
	readonly timeoutMs?: number;
	/** Permission policy for tool use requests from the agent. Defaults to 'prompt'. */
	readonly permissionPolicy?: ACPPermissionPolicy;
}

export interface ACPConfig {
	/** ACP servers to connect to. At least one is required. */
	readonly servers: readonly ACPServerEntry[];
	/** Name of the default server (must match a server entry name). */
	readonly defaultServer?: string;
	/** Default agent ID used when neither the step nor the server specifies one. */
	readonly defaultAgent?: string;
	/** MCP server configs to pass to ACP agents during session creation. */
	readonly mcpServers?: readonly ACPMCPServerConfig[];
}

/** MCP server config passed to ACP agents so they can discover tools. */
export interface ACPMCPServerConfig {
	readonly name: string;
	readonly command: string;
	readonly args?: readonly string[];
	readonly env?: Readonly<Record<string, string>>;
}

// ---------------------------------------------------------------------------
// Token usage tracking
// ---------------------------------------------------------------------------

export interface ACPTokenUsage {
	/** Number of tokens in the prompt / input. */
	readonly promptTokens: number;
	/** Number of tokens in the completion / output. */
	readonly completionTokens: number;
	/** Total tokens consumed (prompt + completion). */
	readonly totalTokens: number;
}

// ---------------------------------------------------------------------------
// Agent info (synthetic — derived from config, not from protocol)
// ---------------------------------------------------------------------------

export interface ACPAgentInfo {
	/** Unique identifier of the agent. */
	readonly id: string;
	/** Human-readable name. */
	readonly name?: string;
	/** Description of what this agent does. */
	readonly description?: string;
	/** Additional metadata. */
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Streaming chunk types
// ---------------------------------------------------------------------------

/** An incremental text delta from a streaming response. */
export interface ACPStreamDelta {
	readonly type: 'delta';
	readonly text: string;
}

/** Final event emitted when a stream completes, carrying optional usage. */
export interface ACPStreamComplete {
	readonly type: 'complete';
	readonly usage?: ACPTokenUsage;
}

/** A tool call event from a streaming response. */
export interface ACPStreamToolCall {
	readonly type: 'tool_call';
	readonly toolCall: ACPToolCall;
}

/** A tool call progress update from a streaming response. */
export interface ACPStreamToolCallUpdate {
	readonly type: 'tool_call_update';
	readonly update: ACPToolCallUpdate;
}

/** Discriminated union yielded by `generateStream()`. */
export type ACPStreamChunk =
	| ACPStreamDelta
	| ACPStreamComplete
	| ACPStreamToolCall
	| ACPStreamToolCallUpdate;

// ---------------------------------------------------------------------------
// Client result types
// ---------------------------------------------------------------------------

export interface ACPGenerateResult {
	/** The generated text content. */
	readonly content: string;
	/** The agent that produced the response. */
	readonly agentId: string;
	/** The server that handled the request. */
	readonly serverName: string;
	/** The session ID for traceability. */
	readonly sessionId: string;
	/** Token usage reported by the server, if available. */
	readonly usage?: ACPTokenUsage;
	/** Stop reason from the agent. */
	readonly stopReason?: ACPStopReason;
}

export interface ACPEmbedResult {
	/** One embedding vector per input string. */
	readonly embeddings: ReadonlyArray<readonly number[]>;
	/** The agent that produced the embeddings. */
	readonly agentId: string;
	/** The server that handled the request. */
	readonly serverName: string;
	/** Token usage reported by the server, if available. */
	readonly usage?: ACPTokenUsage;
}

// ---------------------------------------------------------------------------
// Options for generate / chat / embed
// ---------------------------------------------------------------------------

export interface ACPGenerateOptions {
	readonly agentId?: string;
	readonly serverName?: string;
	readonly systemPrompt?: string;
	readonly config?: Readonly<Record<string, unknown>>;
	readonly sampling?: ACPSamplingParams;
}

export interface ACPChatMessage {
	readonly role: 'system' | 'user' | 'assistant';
	readonly content: string;
}

export interface ACPChatOptions {
	readonly agentId?: string;
	readonly serverName?: string;
	readonly config?: Readonly<Record<string, unknown>>;
	readonly sampling?: ACPSamplingParams;
}

// ---------------------------------------------------------------------------
// Sampling parameters for generation
// ---------------------------------------------------------------------------

export interface ACPSamplingParams {
	readonly temperature?: number;
	readonly maxTokens?: number;
	readonly topP?: number;
	readonly topK?: number;
	readonly stopSequences?: readonly string[];
}

// ---------------------------------------------------------------------------
// Tool call types from session/update notifications
// ---------------------------------------------------------------------------

export interface ACPToolCall {
	readonly toolCallId: string;
	readonly title: string;
	readonly kind:
		| 'read'
		| 'edit'
		| 'delete'
		| 'move'
		| 'search'
		| 'execute'
		| 'think'
		| 'fetch'
		| 'other';
	readonly status:
		| 'pending'
		| 'in_progress'
		| 'completed'
		| 'failed'
		| 'cancelled';
}

export interface ACPToolCallUpdate {
	readonly toolCallId: string;
	readonly status:
		| 'pending'
		| 'in_progress'
		| 'completed'
		| 'failed'
		| 'cancelled';
	readonly content?: unknown;
}

// ---------------------------------------------------------------------------
// Model info from session/new response
// ---------------------------------------------------------------------------

export interface ACPModelInfo {
	readonly modelId: string;
	readonly name: string;
	readonly description?: string;
}

export interface ACPModelsInfo {
	readonly availableModels: readonly ACPModelInfo[];
	readonly currentModelId: string;
}

// ---------------------------------------------------------------------------
// Mode info from session/new response
// ---------------------------------------------------------------------------

export interface ACPModeInfo {
	readonly id: string;
	readonly name: string;
	readonly description?: string;
}

export interface ACPModesInfo {
	readonly currentModeId: string;
	readonly availableModes: readonly ACPModeInfo[];
}

// ---------------------------------------------------------------------------
// Extended session info with models and modes
// ---------------------------------------------------------------------------

export interface ACPSessionInfo {
	readonly sessionId: string;
	readonly models?: ACPModelsInfo;
	readonly modes?: ACPModesInfo;
}

// ---------------------------------------------------------------------------
// Session list/load types
// ---------------------------------------------------------------------------

export interface ACPSessionListEntry {
	readonly sessionId: string;
	readonly createdAt?: string;
	readonly lastActiveAt?: string;
}

// ---------------------------------------------------------------------------
// Client capabilities for init
// ---------------------------------------------------------------------------

export interface ACPClientCapabilities {
	readonly permissions?: boolean;
	readonly streaming?: boolean;
}
