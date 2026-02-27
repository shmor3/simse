// ---------------------------------------------------------------------------
// Subagent Tool Registration
//
// Registers subagent_spawn and subagent_delegate tools with a ToolRegistry.
// Follows the same pattern as registerLibraryTools / registerTaskTools.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { Library } from '../library/library.js';
import { createConversation } from '../conversation/conversation.js';
import { createAgenticLoop } from '../loop/agentic-loop.js';
import type { SubagentInfo, SubagentResult } from '../loop/types.js';
import { createToolRegistry } from './tool-registry.js';
import type {
	ToolCallRequest,
	ToolCallResult,
	ToolDefinition,
	ToolHandler,
	ToolRegistry,
} from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SubagentCallbacks {
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

export interface SubagentToolsOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly callbacks?: SubagentCallbacks;
	readonly defaultMaxTurns?: number;
	readonly maxDepth?: number;
	readonly serverName?: string;
	readonly agentId?: string;
	readonly systemPrompt?: string;
	readonly library?: Library;
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

let subagentCounter = 0;

const nextSubagentId = (): string => `sub_${++subagentCounter}`;

/** Reset counter — exposed for tests only. */
export const _resetSubagentCounter = (): void => {
	subagentCounter = 0;
};

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Child registry construction
// ---------------------------------------------------------------------------

function createChildRegistry(
	parentRegistry: ToolRegistry,
	options: SubagentToolsOptions,
	childDepth: number,
): ToolRegistry {
	const childRegistry = createToolRegistry({});

	// Copy all tools from parent except subagent tools
	for (const def of parentRegistry.getToolDefinitions()) {
		if (def.name === 'subagent_spawn' || def.name === 'subagent_delegate') {
			continue;
		}
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

	// Register subagent tools at the next depth level
	registerSubagentTools(childRegistry, options, childDepth);

	return childRegistry;
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerSubagentTools(
	registry: ToolRegistry,
	options: SubagentToolsOptions,
	depth = 0,
): void {
	const {
		acpClient,
		callbacks,
		defaultMaxTurns = 10,
		maxDepth = 2,
		serverName,
		agentId,
		systemPrompt,
	} = options;

	// If we've hit max depth, don't register subagent tools
	if (depth >= maxDepth) return;

	// subagent_spawn — nested agentic loop
	registerTool(
		registry,
		{
			name: 'subagent_spawn',
			description:
				'Spawn a subagent to handle a complex, multi-step task autonomously. The subagent runs in its own conversation context with access to all tools and returns its final result.',
			parameters: {
				task: {
					type: 'string',
					description: 'The task/prompt for the subagent to work on',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing what the subagent will do (e.g. "Researching API endpoints")',
					required: true,
				},
				maxTurns: {
					type: 'number',
					description: `Maximum turns the subagent can take (default: ${defaultMaxTurns})`,
				},
				systemPrompt: {
					type: 'string',
					description: 'Optional system prompt override for the subagent',
				},
			},
			category: 'subagent',
		},
		async (args) => {
			const id = nextSubagentId();
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Subagent task');
			const turns =
				typeof args.maxTurns === 'number' ? args.maxTurns : defaultMaxTurns;
			const childSystemPrompt =
				typeof args.systemPrompt === 'string'
					? args.systemPrompt
					: systemPrompt;

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'spawn' as const,
			});

			callbacks?.onSubagentStart?.(info);

			try {
				const childRegistry = createChildRegistry(
					options.toolRegistry,
					options,
					depth + 1,
				);

				if (options.library) {
					const shelfName = desc.replace(/\s+/g, '-').toLowerCase();
					const shelf = options.library.shelf(shelfName);

					// Override library tools with shelf-scoped versions
					childRegistry.register(
						{
							name: 'library_search',
							description:
								"Search the library for relevant volumes. Results are scoped to this agent's shelf.",
							parameters: {
								query: {
									type: 'string',
									description: 'The search query',
									required: true,
								},
								maxResults: {
									type: 'number',
									description: 'Max results (default: 5)',
								},
							},
							category: 'library',
							annotations: { readOnly: true },
						},
						async (args) => {
							const query = String(args.query ?? '');
							const maxResults =
								typeof args.maxResults === 'number'
									? args.maxResults
									: 5;
							const results = await shelf.search(query, maxResults);
							if (results.length === 0)
								return 'No matching volumes found.';
							return results
								.map(
									(r, i) =>
										`${i + 1}. [${r.volume.metadata.topic ?? 'uncategorized'}] (score: ${r.score.toFixed(2)})\n   ${r.volume.text}`,
								)
								.join('\n\n');
						},
					);

					childRegistry.register(
						{
							name: 'library_shelve',
							description: "Shelve a volume in this agent's shelf.",
							parameters: {
								text: {
									type: 'string',
									description: 'The text content to shelve',
									required: true,
								},
								topic: {
									type: 'string',
									description: 'Topic category',
									required: true,
								},
							},
							category: 'library',
						},
						async (args) => {
							const text = String(args.text ?? '');
							const topic = String(args.topic ?? 'general');
							const id = await shelf.add(text, { topic });
							return `Shelved volume with ID: ${id}`;
						},
					);

					childRegistry.register(
						{
							name: 'library_search_global',
							description:
								'Search the entire library across all shelves.',
							parameters: {
								query: {
									type: 'string',
									description: 'The search query',
									required: true,
								},
								maxResults: {
									type: 'number',
									description: 'Max results (default: 5)',
								},
							},
							category: 'library',
							annotations: { readOnly: true },
						},
						async (args) => {
							const query = String(args.query ?? '');
							const maxResults =
								typeof args.maxResults === 'number'
									? args.maxResults
									: 5;
							const results = await shelf.searchGlobal(
								query,
								maxResults,
							);
							if (results.length === 0)
								return 'No matching volumes found.';
							return results
								.map(
									(r, i) =>
										`${i + 1}. [${r.volume.metadata.topic ?? 'uncategorized'}] (score: ${r.score.toFixed(2)})\n   ${r.volume.text}`,
								)
								.join('\n\n');
						},
					);
				}

				const childConversation = createConversation();
				const childLoop = createAgenticLoop({
					acpClient,
					toolRegistry: childRegistry,
					conversation: childConversation,
					maxTurns: turns,
					serverName,
					agentId,
					systemPrompt: childSystemPrompt,
				});

				const start = Date.now();
				const result = await childLoop.run(task, {
					onStreamDelta: (text) => {
						callbacks?.onSubagentStreamDelta?.(id, text);
					},
					onToolCallStart: (call) => {
						callbacks?.onSubagentToolCallStart?.(id, call);
					},
					onToolCallEnd: (toolResult) => {
						callbacks?.onSubagentToolCallEnd?.(id, toolResult);
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

	// subagent_delegate — single-shot ACP generation
	registerTool(
		registry,
		{
			name: 'subagent_delegate',
			description:
				'Delegate a simple task to an ACP agent for a single-shot response. Use for tasks that do not require multi-step tool use.',
			parameters: {
				task: {
					type: 'string',
					description: 'The task/prompt to delegate',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing the delegation (e.g. "Summarizing document")',
					required: true,
				},
				serverName: {
					type: 'string',
					description: 'Target ACP server name (optional)',
				},
				agentId: {
					type: 'string',
					description: 'Target agent ID (optional)',
				},
			},
			category: 'subagent',
		},
		async (args) => {
			const id = nextSubagentId();
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Delegated task');
			const targetServer =
				typeof args.serverName === 'string' ? args.serverName : serverName;
			const targetAgent =
				typeof args.agentId === 'string' ? args.agentId : agentId;

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'delegate' as const,
			});

			callbacks?.onSubagentStart?.(info);

			try {
				const start = Date.now();
				const result = await acpClient.generate(task, {
					serverName: targetServer,
					agentId: targetAgent,
				});

				const subResult: SubagentResult = Object.freeze({
					text: result.content,
					turns: 1,
					durationMs: Date.now() - start,
				});

				callbacks?.onSubagentComplete?.(id, subResult);
				return result.content;
			} catch (err) {
				const error = toError(err);
				callbacks?.onSubagentError?.(id, error);
				throw error;
			}
		},
	);
}
