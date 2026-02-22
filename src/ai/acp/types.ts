// ---------------------------------------------------------------------------
// Agent Communication Protocol (ACP) Types
// ---------------------------------------------------------------------------
//
// All types are strictly readonly to enforce immutability throughout
// the codebase.  No classes — only plain data interfaces.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface ACPServerEntry {
	/** Friendly name for this ACP server connection. */
	readonly name: string;
	/** Base URL of the ACP-compatible server (e.g. "http://localhost:8000"). */
	readonly url: string;
	/** Default agent ID to use when none is specified per-step. */
	readonly defaultAgent?: string;
	/**
	 * Optional API key for authenticated servers.
	 * Prefer setting via environment variables (ACP_API_KEY_<NAME>).
	 */
	readonly apiKey?: string;
	/** Request timeout in milliseconds. Defaults to 30 000. */
	readonly timeoutMs?: number;
}

export interface ACPConfig {
	/** ACP servers to connect to. At least one is required. */
	readonly servers: readonly ACPServerEntry[];
	/** Name of the default server (must match a server entry name). */
	readonly defaultServer?: string;
	/** Default agent ID used when neither the step nor the server specifies one. */
	readonly defaultAgent?: string;
}

// ---------------------------------------------------------------------------
// ACP Protocol — Messages
// ---------------------------------------------------------------------------

/** A single text content part within a message. */
export interface ACPTextPart {
	readonly type: 'text';
	readonly text: string;
}

/** A single data content part within a message. */
export interface ACPDataPart {
	readonly type: 'data';
	readonly data: unknown;
	readonly mimeType?: string;
}

export type ACPMessagePart = ACPTextPart | ACPDataPart;

/** A message exchanged between user and agent. */
export interface ACPMessage {
	readonly role: 'user' | 'agent';
	readonly parts: readonly ACPMessagePart[];
}

// ---------------------------------------------------------------------------
// ACP Protocol — Agents
// ---------------------------------------------------------------------------

export interface ACPAgentInfo {
	/** Unique identifier of the agent. */
	readonly id: string;
	/** Human-readable name. */
	readonly name?: string;
	/** Description of what this agent does. */
	readonly description?: string;
	/** Additional metadata the server may expose. */
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// ACP Protocol — Runs
// ---------------------------------------------------------------------------

export type ACPRunStatus =
	| 'created'
	| 'in_progress'
	| 'awaiting_input'
	| 'completed'
	| 'failed'
	| 'cancelled';

export interface ACPRunError {
	readonly message: string;
	readonly code?: string;
}

export interface ACPRun {
	/** Unique run identifier. */
	readonly run_id: string;
	/** The agent that handled this run. */
	readonly agent_id: string;
	/** Current status. */
	readonly status: ACPRunStatus;
	/** Output messages from the agent (populated when completed). */
	readonly output?: readonly ACPMessage[];
	/** Error details when status is "failed". */
	readonly error?: ACPRunError;
	/** Additional metadata. */
	readonly metadata?: Readonly<Record<string, unknown>>;
	/** ISO-8601 timestamp of creation. */
	readonly created_at?: string;
	/** ISO-8601 timestamp of last update. */
	readonly updated_at?: string;
}

// ---------------------------------------------------------------------------
// ACP Protocol — Requests
// ---------------------------------------------------------------------------

export interface ACPCreateRunRequest {
	/** Agent to run. */
	readonly agent_id: string;
	/** Input messages. */
	readonly input: readonly ACPMessage[];
	/** Optional run-level configuration forwarded to the agent. */
	readonly config?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// ACP Protocol — Streaming Events (SSE)
// ---------------------------------------------------------------------------

export type ACPStreamEventType =
	| 'run.created'
	| 'run.in_progress'
	| 'run.completed'
	| 'run.failed'
	| 'message.delta'
	| 'message.completed'
	| 'generic';

export interface ACPStreamEvent {
	/** Event type. */
	readonly event: ACPStreamEventType;
	/** Event payload. */
	readonly data: ACPRun | ACPMessageDelta | Readonly<Record<string, unknown>>;
}

export interface ACPMessageDelta {
	/** Incremental text content. */
	readonly delta: string;
}

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
	/** The run ID for traceability. */
	readonly runId: string;
}

export interface ACPEmbedResult {
	/** One embedding vector per input string. */
	readonly embeddings: ReadonlyArray<readonly number[]>;
	/** The agent that produced the embeddings. */
	readonly agentId: string;
	/** The server that handled the request. */
	readonly serverName: string;
}

// ---------------------------------------------------------------------------
// Options for generate / chat / embed
// ---------------------------------------------------------------------------

export interface ACPGenerateOptions {
	readonly agentId?: string;
	readonly serverName?: string;
	readonly systemPrompt?: string;
	readonly config?: Readonly<Record<string, unknown>>;
}

export interface ACPChatMessage {
	readonly role: 'system' | 'user' | 'assistant';
	readonly content: string;
}

export interface ACPChatOptions {
	readonly agentId?: string;
	readonly serverName?: string;
	readonly config?: Readonly<Record<string, unknown>>;
}
