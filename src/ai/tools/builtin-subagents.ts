// ---------------------------------------------------------------------------
// Built-in Subagent Registration
//
// Registers specialized subagent tools (subagent_explore, subagent_plan)
// with a ToolRegistry. These provide pre-configured, read-only child loops
// with focused system prompts and restricted tool access.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import { createConversation } from '../conversation/conversation.js';
import { createAgenticLoop } from '../loop/agentic-loop.js';
import type { SubagentInfo, SubagentResult } from '../loop/types.js';
import { createToolRegistry } from './tool-registry.js';
import type { ToolDefinition, ToolHandler, ToolRegistry } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface BuiltinSubagentCallbacks {
	readonly onSubagentStart?: (info: SubagentInfo) => void;
	readonly onSubagentStreamDelta?: (id: string, text: string) => void;
	readonly onSubagentComplete?: (id: string, result: SubagentResult) => void;
	readonly onSubagentError?: (id: string, error: Error) => void;
}

export interface BuiltinSubagentOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly callbacks?: BuiltinSubagentCallbacks;
	readonly serverName?: string;
	readonly agentId?: string;
	/** Max turns for the explore subagent. Default: 5. */
	readonly exploreMaxTurns?: number;
	/** Max turns for the plan subagent. Default: 10. */
	readonly planMaxTurns?: number;
}

// ---------------------------------------------------------------------------
// Read-only tool categories
// ---------------------------------------------------------------------------

const READ_ONLY_CATEGORIES = new Set(['read', 'search', 'memory']);

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

let builtinSubagentCounter = 0;

const nextId = (prefix: string): string =>
	`${prefix}_${++builtinSubagentCounter}`;

/** Reset counter — exposed for tests only. */
export const _resetBuiltinSubagentCounter = (): void => {
	builtinSubagentCounter = 0;
};

/**
 * Create a child registry containing only read-only tools from the parent.
 */
function createFilteredChildRegistry(
	parentRegistry: ToolRegistry,
): ToolRegistry {
	const childRegistry = createToolRegistry({});

	for (const def of parentRegistry.getToolDefinitions()) {
		// Include tools that are explicitly read-only via annotations
		const isReadOnly = def.annotations?.readOnly === true;
		// Include tools in read-only categories
		const isReadCategory =
			def.category !== undefined && READ_ONLY_CATEGORIES.has(def.category);

		if (!isReadOnly && !isReadCategory) continue;

		// Delegate execution to the parent registry
		childRegistry.register(def, async (args) => {
			const result = await parentRegistry.execute({
				id: `child_call_${Date.now()}`,
				name: def.name,
				arguments: args,
			});
			if (result.isError) throw new Error(result.output);
			return result.output;
		});
	}

	return childRegistry;
}

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// System prompts
// ---------------------------------------------------------------------------

const EXPLORE_SYSTEM_PROMPT = `You are a fast, read-only codebase exploration agent.

Your task is to quickly find information in the codebase and return a concise answer.

Guidelines:
- Use search tools (grep, glob) efficiently to locate relevant code.
- Read only the files necessary to answer the question.
- Be concise — answer directly without elaboration.
- You have read-only access. Do not attempt to modify files.
- Return your findings as a clear, structured summary.`;

const PLAN_SYSTEM_PROMPT = `You are a research and planning agent.

Your task is to analyze the codebase, understand existing patterns, and produce a structured implementation plan.

Guidelines:
- Explore the codebase thoroughly to understand architecture and conventions.
- Identify the files that need to be created or modified.
- Consider edge cases, error handling, and testing.
- Output a numbered implementation plan with specific file paths and descriptions.
- You have read-only access. Do not attempt to modify files.`;

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerBuiltinSubagents(
	registry: ToolRegistry,
	options: BuiltinSubagentOptions,
): void {
	const {
		acpClient,
		callbacks,
		serverName,
		agentId,
		exploreMaxTurns = 5,
		planMaxTurns = 10,
	} = options;

	// subagent_explore — fast, read-only codebase exploration
	registerTool(
		registry,
		{
			name: 'subagent_explore',
			description:
				'Fast, read-only codebase exploration agent. Use for finding files, searching code, understanding architecture, or answering questions about the codebase. Returns concise answers.',
			parameters: {
				task: {
					type: 'string',
					description:
						'What to explore or find in the codebase (e.g. "find all API endpoint handlers", "how does authentication work?")',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing the exploration (e.g. "Finding API endpoints")',
					required: true,
				},
			},
			category: 'subagent',
			annotations: { readOnly: true },
		},
		async (args) => {
			const id = nextId('explore');
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Exploring codebase');

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'spawn' as const,
			});
			callbacks?.onSubagentStart?.(info);

			try {
				const childRegistry = createFilteredChildRegistry(options.toolRegistry);
				const childConversation = createConversation();
				const childLoop = createAgenticLoop({
					acpClient,
					toolRegistry: childRegistry,
					conversation: childConversation,
					maxTurns: exploreMaxTurns,
					serverName,
					agentId,
					systemPrompt: EXPLORE_SYSTEM_PROMPT,
				});

				const start = Date.now();
				const result = await childLoop.run(task, {
					onStreamDelta: (text) => {
						callbacks?.onSubagentStreamDelta?.(id, text);
					},
					onError: (error) => {
						callbacks?.onSubagentError?.(id, error);
					},
				});

				const subResult: SubagentResult = Object.freeze({
					text: result.finalText,
					turns: result.totalTurns,
					durationMs: Date.now() - start,
				});

				callbacks?.onSubagentComplete?.(id, subResult);
				return result.finalText;
			} catch (err) {
				const error = toError(err);
				callbacks?.onSubagentError?.(id, error);
				throw error;
			}
		},
	);

	// subagent_plan — research-only planning agent
	registerTool(
		registry,
		{
			name: 'subagent_plan',
			description:
				'Research and planning agent with read-only access. Analyzes the codebase, understands patterns and conventions, and produces structured implementation plans with specific files to create/modify.',
			parameters: {
				task: {
					type: 'string',
					description:
						'What to plan (e.g. "plan implementation of user authentication", "design the caching layer")',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing the planning task (e.g. "Planning auth system")',
					required: true,
				},
			},
			category: 'subagent',
			annotations: { readOnly: true },
		},
		async (args) => {
			const id = nextId('plan');
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Planning implementation');

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'spawn' as const,
			});
			callbacks?.onSubagentStart?.(info);

			try {
				const childRegistry = createFilteredChildRegistry(options.toolRegistry);
				const childConversation = createConversation();
				const childLoop = createAgenticLoop({
					acpClient,
					toolRegistry: childRegistry,
					conversation: childConversation,
					maxTurns: planMaxTurns,
					serverName,
					agentId,
					systemPrompt: PLAN_SYSTEM_PROMPT,
				});

				const start = Date.now();
				const result = await childLoop.run(task, {
					onStreamDelta: (text) => {
						callbacks?.onSubagentStreamDelta?.(id, text);
					},
					onError: (error) => {
						callbacks?.onSubagentError?.(id, error);
					},
				});

				const subResult: SubagentResult = Object.freeze({
					text: result.finalText,
					turns: result.totalTurns,
					durationMs: Date.now() - start,
				});

				callbacks?.onSubagentComplete?.(id, subResult);
				return result.finalText;
			} catch (err) {
				const error = toError(err);
				callbacks?.onSubagentError?.(id, error);
				throw error;
			}
		},
	);
}
