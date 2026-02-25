// ---------------------------------------------------------------------------
// Tool Registry Types
// ---------------------------------------------------------------------------

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
// Permission Resolver
// ---------------------------------------------------------------------------

export interface ToolPermissionResolver {
	readonly check: (request: ToolCallRequest) => Promise<boolean>;
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
}

export interface ToolRegistry {
	readonly discover: () => Promise<void>;
	readonly register: (definition: ToolDefinition, handler: ToolHandler) => void;
	readonly unregister: (name: string) => boolean;
	readonly getToolDefinitions: () => readonly ToolDefinition[];
	readonly formatForSystemPrompt: () => string;
	readonly execute: (call: ToolCallRequest) => Promise<ToolCallResult>;
	readonly parseToolCalls: (response: string) => {
		readonly text: string;
		readonly toolCalls: readonly ToolCallRequest[];
	};
	readonly toolCount: number;
	readonly toolNames: readonly string[];
}
