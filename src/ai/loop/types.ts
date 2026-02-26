// ---------------------------------------------------------------------------
// Agentic Loop Types
// ---------------------------------------------------------------------------

import type { EventBus } from '../../events/types.js';
import type { Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { ACPToolCall, ACPToolCallUpdate } from '../acp/types.js';
import type { ContextPruner } from '../conversation/context-pruner.js';
import type { Conversation } from '../conversation/types.js';
import type { MemoryMiddleware } from '../memory/middleware.js';
import type { TextGenerationProvider } from '../memory/types.js';
import type { SystemPromptBuilder } from '../prompts/types.js';
import type {
	ToolCallRequest,
	ToolCallResult,
	ToolRegistry,
} from '../tools/types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface AgenticLoopOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly conversation: Conversation;
	readonly logger?: Logger;
	readonly maxTurns?: number;
	readonly serverName?: string;
	readonly agentId?: string;
	readonly systemPrompt?: string;
	readonly signal?: AbortSignal;
	/** Enable auto-compaction when conversation exceeds threshold. */
	readonly autoCompact?: boolean;
	/** Provider for summarizing conversation during auto-compaction. */
	readonly compactionProvider?: TextGenerationProvider;
	/** Retry config for the generateStream call within each turn. */
	readonly streamRetry?: {
		readonly maxAttempts?: number;
		readonly baseDelayMs?: number;
	};
	/** Retry config for tool execution when results look transient. */
	readonly toolRetry?: {
		readonly maxAttempts?: number;
		readonly baseDelayMs?: number;
	};
	/** Optional event bus for publishing loop lifecycle events. */
	readonly eventBus?: EventBus;
	/** Optional memory middleware for per-turn context enrichment and response storage. */
	readonly memoryMiddleware?: MemoryMiddleware;
	/**
	 * When true, the ACP agent manages its own tool calling (e.g. Claude Code).
	 * Skips injecting `<tool_use>` XML into the system prompt and skips
	 * parsing `<tool_use>` tags from responses. Instead, tool activity is
	 * reported via `onToolCall`/`onToolCallUpdate` callbacks from ACP
	 * `session/update` notifications.
	 */
	readonly agentManagesTools?: boolean;
	/**
	 * When provided, the builder constructs the base system prompt instead of
	 * simple concatenation of tool definitions and user system prompt.
	 */
	readonly systemPromptBuilder?: SystemPromptBuilder;
	/**
	 * Context pruner for lightweight pre-compaction pass. When provided,
	 * prunes old tool outputs before falling through to full summarization.
	 */
	readonly contextPruner?: ContextPruner;
}

// ---------------------------------------------------------------------------
// Turn
// ---------------------------------------------------------------------------

export interface LoopTurn {
	readonly turn: number;
	readonly type: 'text' | 'tool_use';
	readonly text?: string;
	readonly toolCalls?: readonly ToolCallRequest[];
	readonly toolResults?: readonly ToolCallResult[];
	readonly durationMs: number;
}

// ---------------------------------------------------------------------------
// Subagent types
// ---------------------------------------------------------------------------

export interface SubagentInfo {
	readonly id: string;
	readonly description: string;
	readonly mode: 'spawn' | 'delegate';
}

export interface SubagentResult {
	readonly text: string;
	readonly turns: number;
	readonly durationMs: number;
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

export interface LoopCallbacks {
	readonly onStreamDelta?: (text: string) => void;
	readonly onStreamStart?: () => void;
	readonly onToolCallStart?: (call: ToolCallRequest) => void;
	readonly onToolCallEnd?: (result: ToolCallResult) => void;
	readonly onTurnComplete?: (turn: LoopTurn) => void;
	readonly onCompaction?: (summary: string) => void;
	readonly onError?: (error: Error) => void;
	/** Called when the ACP agent starts a tool call (agentManagesTools mode). */
	readonly onAgentToolCall?: (toolCall: ACPToolCall) => void;
	/** Called when the ACP agent updates a tool call (agentManagesTools mode). */
	readonly onAgentToolCallUpdate?: (update: ACPToolCallUpdate) => void;
	readonly onSubagentStart?: (info: SubagentInfo) => void;
	readonly onSubagentStreamDelta?: (id: string, text: string) => void;
	readonly onSubagentToolCallStart?: (
		id: string,
		call: ToolCallRequest,
	) => void;
	readonly onSubagentToolCallEnd?: (id: string, result: ToolCallResult) => void;
	readonly onSubagentComplete?: (id: string, result: SubagentResult) => void;
	readonly onSubagentError?: (id: string, error: Error) => void;
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

export interface AgenticLoopResult {
	readonly finalText: string;
	readonly turns: readonly LoopTurn[];
	readonly totalTurns: number;
	readonly hitTurnLimit: boolean;
	readonly aborted: boolean;
	readonly totalDurationMs: number;
}

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface AgenticLoop {
	readonly run: (
		userInput: string,
		callbacks?: LoopCallbacks,
	) => Promise<AgenticLoopResult>;
}
