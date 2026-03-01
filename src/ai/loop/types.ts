// ---------------------------------------------------------------------------
// Agentic Loop Types
// ---------------------------------------------------------------------------

import type { LibraryServices } from '../library/library-services.js';
import type { TextGenerationProvider } from '../library/types.js';
import type { EventBus } from '../../events/types.js';
import type { Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type {
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
} from '../acp/types.js';
import type { ContextPruner } from '../conversation/context-pruner.js';
import type { Conversation } from '../conversation/types.js';
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
	/** Optional library services for per-turn context enrichment and response storage. */
	readonly libraryServices?: LibraryServices;
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
	/** Max consecutive identical tool calls before doom loop fires. Default: 3. */
	readonly maxIdenticalToolCalls?: number;
	/** Custom compaction prompt override. */
	readonly compactionPrompt?: string;
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
	/** Token usage for this turn, if reported by the server. */
	readonly usage?: ACPTokenUsage;
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
	/** Called after each turn with the accumulated token usage across all turns. */
	readonly onUsageUpdate?: (accumulated: ACPTokenUsage) => void;
	/** Called when a doom loop is detected (same tool + args called N times consecutively). */
	readonly onDoomLoop?: (toolName: string, callCount: number) => void;
	/** Called before compaction — return a string to append to conversation for context preservation. */
	readonly onPreCompaction?: (conversation: string) => string | undefined;
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
	/** Accumulated token usage across all turns, if reported by the server. */
	readonly totalUsage?: ACPTokenUsage;
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
