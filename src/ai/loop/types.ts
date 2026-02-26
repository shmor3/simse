// ---------------------------------------------------------------------------
// Agentic Loop Types
// ---------------------------------------------------------------------------

import type { EventBus } from '../../events/types.js';
import type { Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { Conversation } from '../conversation/types.js';
import type { TextGenerationProvider } from '../memory/types.js';
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
