// ---------------------------------------------------------------------------
// Tool Registry Types
// ---------------------------------------------------------------------------

import type { EventBus } from '../../events/types.js';
import type { Logger } from '../../logger.js';
import type { MCPClient } from '../mcp/mcp-client.js';
import type { MemoryManager } from '../memory/memory.js';
import type { VirtualFS } from '../vfs/index.js';

// ---------------------------------------------------------------------------
// Tool Definitions
// ---------------------------------------------------------------------------

export type ToolCategory =
	| 'read'
	| 'edit'
	| 'search'
	| 'execute'
	| 'memory'
	| 'vfs'
	| 'task'
	| 'subagent'
	| 'other';

export interface ToolParameter {
	readonly type: string;
	readonly description: string;
	readonly required?: boolean;
}

export interface ToolAnnotations {
	readonly title?: string;
	/** Whether the tool performs destructive operations. */
	readonly destructive?: boolean;
	/** Whether the tool is read-only. */
	readonly readOnly?: boolean;
}

export interface ToolDefinition {
	readonly name: string;
	readonly description: string;
	readonly parameters: Readonly<Record<string, ToolParameter>>;
	readonly category?: ToolCategory;
	readonly annotations?: ToolAnnotations;
	/** Per-tool execution timeout in milliseconds. Overrides `defaultToolTimeoutMs`. */
	readonly timeoutMs?: number;
	/** Per-tool output character limit. Overrides `maxOutputChars` from registry options. */
	readonly maxOutputChars?: number;
}

// ---------------------------------------------------------------------------
// Tool Calls
// ---------------------------------------------------------------------------

export interface ToolCallRequest {
	readonly id: string;
	readonly name: string;
	readonly arguments: Record<string, unknown>;
}

export interface ToolCallResult {
	readonly id: string;
	readonly name: string;
	readonly output: string;
	readonly isError: boolean;
	readonly durationMs?: number;
}

// ---------------------------------------------------------------------------
// Handler & Registration
// ---------------------------------------------------------------------------

export type ToolHandler = (args: Record<string, unknown>) => Promise<string>;

export interface RegisteredTool {
	readonly definition: ToolDefinition;
	readonly handler: ToolHandler;
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

export interface ToolMetrics {
	readonly name: string;
	readonly callCount: number;
	readonly errorCount: number;
	readonly totalDurationMs: number;
	readonly avgDurationMs: number;
	readonly lastCalledAt: number;
}

// ---------------------------------------------------------------------------
// Permission Resolver
// ---------------------------------------------------------------------------

export interface ToolPermissionResolver {
	readonly check: (
		request: ToolCallRequest,
		definition?: ToolDefinition,
	) => Promise<boolean>;
}

// ---------------------------------------------------------------------------
// Registry Options & Interface
// ---------------------------------------------------------------------------

export interface ToolRegistryOptions {
	readonly mcpClient?: MCPClient;
	readonly memoryManager?: MemoryManager;
	readonly vfs?: VirtualFS;
	readonly permissionResolver?: ToolPermissionResolver;
	readonly logger?: Logger;
	/** Default timeout for tool execution in milliseconds. Per-tool `timeoutMs` overrides this. */
	readonly defaultToolTimeoutMs?: number;
	/** Maximum characters for tool output before truncation. Per-tool `maxOutputChars` overrides this. Default: 50_000. */
	readonly maxOutputChars?: number;
	/** Optional event bus for publishing tool lifecycle events. */
	readonly eventBus?: EventBus;
}

export interface ToolRegistry {
	readonly discover: () => Promise<void>;
	readonly register: (definition: ToolDefinition, handler: ToolHandler) => void;
	readonly unregister: (name: string) => boolean;
	readonly getToolDefinitions: () => readonly ToolDefinition[];
	readonly getToolDefinition: (name: string) => ToolDefinition | undefined;
	readonly formatForSystemPrompt: () => string;
	readonly execute: (call: ToolCallRequest) => Promise<ToolCallResult>;
	readonly batchExecute: (
		calls: readonly ToolCallRequest[],
		options?: { readonly maxConcurrency?: number },
	) => Promise<readonly ToolCallResult[]>;
	readonly parseToolCalls: (response: string) => {
		readonly text: string;
		readonly toolCalls: readonly ToolCallRequest[];
	};
	readonly getToolMetrics: (name: string) => ToolMetrics | undefined;
	readonly getAllToolMetrics: () => readonly ToolMetrics[];
	readonly clearMetrics: () => void;
	readonly isRegistered: (name: string) => boolean;
	readonly toolCount: number;
	readonly toolNames: readonly string[];
}
