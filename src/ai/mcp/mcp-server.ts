import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import { createChain } from '../chain/chain.js';
import { createPromptTemplate } from '../chain/prompt-template.js';
import type { Conversation } from '../conversation/types.js';
import type { MemoryManager } from '../memory/memory.js';
import type { TaskList } from '../tasks/types.js';
import type { ToolRegistry } from '../tools/types.js';
import type { VirtualFS } from '../vfs/index.js';
import type { MCPServerConfig } from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface MCPServerOptions {
	/** ACP client used for generate, run-chain, and list-agents tools. */
	readonly acpClient: ACPClient;
	/** Optional memory manager for memory-search and memory-add tools. */
	readonly memoryManager?: MemoryManager;
	/** Optional VFS for vfs-read, vfs-write, vfs-list, vfs-tree tools. */
	readonly vfs?: VirtualFS;
	/** Optional task list for task-create, task-get, task-update, task-list tools. */
	readonly taskList?: TaskList;
	/** Optional tool registry for tool discovery. */
	readonly toolRegistry?: ToolRegistry;
	/** Optional conversation for conversation state access. */
	readonly conversation?: Conversation;
}

// ---------------------------------------------------------------------------
// SimseMCPServer interface
// ---------------------------------------------------------------------------

export interface SimseMCPServer {
	readonly start: () => Promise<void>;
	readonly stop: () => Promise<void>;
	readonly sendToolListChanged: () => void;
	readonly sendResourceListChanged: () => void;
	readonly sendPromptListChanged: () => void;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an MCP server that exposes simse capabilities (generate, run-chain,
 * list-agents, memory, VFS, and task tools) over the Model Context Protocol.
 *
 * @param config - Server name and version.
 * @param options - ACP client (required), plus optional memory manager, VFS, task list.
 * @returns A frozen {@link SimseMCPServer}. Call `start()` to begin serving over stdio.
 */
export function createMCPServer(
	config: MCPServerConfig,
	options: MCPServerOptions,
): SimseMCPServer {
	const acpClient = options.acpClient;
	const memoryManager = options.memoryManager;
	const vfs = options.vfs;
	const taskList = options.taskList;
	// options.toolRegistry and options.conversation are reserved for future use

	const server = new McpServer({
		name: config.name,
		version: config.version,
	});

	// -----------------------------------------------------------------------
	// Logging helper
	// -----------------------------------------------------------------------

	const sendLog = (level: string, data: unknown, loggerName?: string): void => {
		try {
			server.sendLoggingMessage({
				level: level as 'info',
				logger: loggerName,
				data,
			});
		} catch {
			// Ignore if not connected
		}
	};

	// -----------------------------------------------------------------------
	// List-changed notification helpers
	// -----------------------------------------------------------------------

	const sendToolListChanged = (): void => {
		try {
			server.sendToolListChanged();
		} catch {
			/* ignore */
		}
	};

	const sendResourceListChanged = (): void => {
		try {
			server.sendResourceListChanged();
		} catch {
			/* ignore */
		}
	};

	const sendPromptListChanged = (): void => {
		try {
			server.sendPromptListChanged();
		} catch {
			/* ignore */
		}
	};

	// -----------------------------------------------------------------------
	// Tools
	// -----------------------------------------------------------------------

	const registerTools = (): void => {
		// Tool: generate — single LLM prompt via ACP
		server.registerTool(
			'generate',
			{
				title: 'Generate Text',
				description: 'Generate text from a prompt using an ACP agent',
				inputSchema: {
					prompt: z.string().describe('The prompt to send to the agent'),
					agentId: z
						.string()
						.optional()
						.describe('ACP agent ID to use (defaults to configured default)'),
					serverName: z
						.string()
						.optional()
						.describe(
							'ACP server name to use (defaults to configured default)',
						),
					systemPrompt: z.string().optional().describe('System prompt'),
				},
			},
			async ({ prompt, agentId, serverName, systemPrompt }) => {
				try {
					sendLog(
						'info',
						`Generating text with prompt length ${(prompt as string).length}`,
						'generate',
					);
					const res = await acpClient.generate(prompt, {
						agentId,
						serverName,
						systemPrompt,
					});

					return { content: [{ type: 'text' as const, text: res.content }] };
				} catch (error) {
					const message = toError(error).message;
					sendLog('error', `Generate failed: ${message}`, 'generate');
					return {
						content: [{ type: 'text' as const, text: `Error: ${message}` }],
						isError: true,
					};
				}
			},
		);

		// Tool: run-chain — execute a multi-step chain
		server.registerTool(
			'run-chain',
			{
				title: 'Run Chain',
				description:
					'Execute a multi-step LangChain pipeline. Provide steps as a JSON array and initial values as a JSON object.',
				inputSchema: {
					steps: z
						.string()
						.describe(
							'JSON array of step objects: [{"name":"step1","template":"...","agentId":"my-agent"}]',
						),
					values: z
						.string()
						.describe('JSON object of initial template variable values'),
				},
			},
			async ({ steps, values }) => {
				let parsedSteps: Array<{
					name: string;
					template: string;
					agentId?: string;
					serverName?: string;
					systemPrompt?: string;
				}>;
				let parsedValues: Record<string, string>;

				try {
					parsedSteps = JSON.parse(steps);
				} catch {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "steps" is not valid JSON',
							},
						],
						isError: true,
					};
				}

				if (
					!Array.isArray(parsedSteps) ||
					!parsedSteps.every(
						(s) =>
							typeof s === 'object' &&
							s !== null &&
							typeof s.name === 'string' &&
							typeof s.template === 'string',
					)
				) {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "steps" must be a JSON array of objects with "name" and "template" string fields',
							},
						],
						isError: true,
					};
				}

				try {
					parsedValues = JSON.parse(values);
				} catch {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "values" is not valid JSON',
							},
						],
						isError: true,
					};
				}

				if (
					typeof parsedValues !== 'object' ||
					parsedValues === null ||
					Array.isArray(parsedValues)
				) {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "values" must be a JSON object',
							},
						],
						isError: true,
					};
				}

				const nonStringKeys = Object.entries(parsedValues)
					.filter(([, v]) => typeof v !== 'string')
					.map(([k]) => k);
				if (nonStringKeys.length > 0) {
					return {
						content: [
							{
								type: 'text' as const,
								text: `Error: "values" entries must all be strings. Non-string keys: ${nonStringKeys.join(', ')}`,
							},
						],
						isError: true,
					};
				}

				try {
					const chain = createChain({ acpClient });

					for (const s of parsedSteps) {
						chain.addStep({
							name: s.name,
							template: createPromptTemplate(s.template),
							agentId: s.agentId,
							serverName: s.serverName,
							systemPrompt: s.systemPrompt,
						});
					}

					const results = await chain.run(parsedValues);
					const formatted = results
						.map(
							(r) =>
								`[${r.stepName}] (${r.provider}/${r.model}, ${r.durationMs}ms)\n${r.output}`,
						)
						.join('\n\n');

					return { content: [{ type: 'text' as const, text: formatted }] };
				} catch (error) {
					const message = toError(error).message;
					return {
						content: [{ type: 'text' as const, text: `Error: ${message}` }],
						isError: true,
					};
				}
			},
		);

		// Tool: list-agents — check which ACP agents are available
		server.registerTool(
			'list-agents',
			{
				title: 'List Agents',
				description: 'List available ACP agents across configured servers',
				inputSchema: {
					serverName: z
						.string()
						.optional()
						.describe(
							'Specific ACP server to query (defaults to all configured)',
						),
				},
			},
			async ({ serverName }) => {
				const lines: string[] = [];

				if (serverName) {
					const available = await acpClient.isAvailable(serverName);
					lines.push(
						`${serverName}: ${available ? 'available' : 'unavailable'}`,
					);
					if (available) {
						try {
							const agents = await acpClient.listAgents(serverName);
							for (const agent of agents) {
								lines.push(
									`  - ${agent.id}${agent.name ? ` (${agent.name})` : ''}${agent.description ? `: ${agent.description}` : ''}`,
								);
							}
							if (agents.length === 0) {
								lines.push('  (no agents found)');
							}
						} catch (err) {
							const errMsg = toError(err).message;
							lines.push(`  (could not list agents: ${errMsg})`);
							sendLog(
								'error',
								`Failed to list agents: ${errMsg}`,
								'list-agents',
							);
						}
					}
				} else {
					for (const name of acpClient.serverNames) {
						const available = await acpClient.isAvailable(name);
						lines.push(`${name}: ${available ? 'available' : 'unavailable'}`);
						if (available) {
							try {
								const agents = await acpClient.listAgents(name);
								for (const agent of agents) {
									lines.push(
										`  - ${agent.id}${agent.name ? ` (${agent.name})` : ''}${agent.description ? `: ${agent.description}` : ''}`,
									);
								}
								if (agents.length === 0) {
									lines.push('  (no agents found)');
								}
							} catch {
								lines.push('  (could not list agents)');
							}
						}
					}
				}

				if (lines.length === 0) {
					lines.push('No ACP servers configured.');
				}

				return {
					content: [{ type: 'text' as const, text: lines.join('\n') }],
				};
			},
		);

		// Tool: memory-search — search the vector memory store
		if (memoryManager) {
			server.registerTool(
				'memory-search',
				{
					title: 'Memory Search',
					description: 'Search the vector memory store',
					inputSchema: {
						query: z.string().describe('Search query'),
						maxResults: z
							.number()
							.optional()
							.describe('Max results (default 10)'),
					},
				},
				async ({ query, maxResults }) => {
					try {
						sendLog('info', `Searching memory for: ${query}`, 'memory-search');
						const results = await memoryManager.search(
							query as string,
							(maxResults as number | undefined) ?? 10,
						);
						return {
							content: [
								{
									type: 'text' as const,
									text: JSON.stringify(results, null, 2),
								},
							],
						};
					} catch (error) {
						const message = toError(error).message;
						sendLog(
							'error',
							`Memory search failed: ${message}`,
							'memory-search',
						);
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			// Tool: memory-add — add an entry to the vector memory store
			server.registerTool(
				'memory-add',
				{
					title: 'Memory Add',
					description: 'Add an entry to the vector memory store',
					inputSchema: {
						text: z.string().describe('Text to store'),
						metadata: z.string().optional().describe('Optional JSON metadata'),
					},
				},
				async ({ text, metadata }) => {
					try {
						const meta = metadata ? JSON.parse(metadata as string) : undefined;
						sendLog(
							'info',
							`Adding memory entry (${(text as string).length} chars)`,
							'memory-add',
						);
						const id = await memoryManager.add(text as string, meta);
						return {
							content: [{ type: 'text' as const, text: `Added entry: ${id}` }],
						};
					} catch (error) {
						const message = toError(error).message;
						sendLog('error', `Memory add failed: ${message}`, 'memory-add');
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);
			// Tool: memory-delete — remove an entry from the vector memory store
			server.registerTool(
				'memory-delete',
				{
					title: 'Memory Delete',
					description: 'Delete an entry from the vector memory store',
					inputSchema: {
						id: z.string().describe('The memory entry ID to delete'),
					},
				},
				async ({ id }) => {
					try {
						const deleted = await memoryManager.delete(id as string);
						if (!deleted) {
							return {
								content: [
									{
										type: 'text' as const,
										text: `Memory entry not found: ${id}`,
									},
								],
								isError: true,
							};
						}
						return {
							content: [
								{
									type: 'text' as const,
									text: `Deleted memory entry: ${id}`,
								},
							],
						};
					} catch (error) {
						const message = toError(error).message;
						sendLog(
							'error',
							`Memory delete failed: ${message}`,
							'memory-delete',
						);
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);
		}

		// -- VFS tools --
		if (vfs) {
			server.registerTool(
				'vfs-read',
				{
					title: 'VFS Read',
					description: 'Read a file from the virtual filesystem sandbox',
					inputSchema: {
						path: z.string().describe('The file path to read'),
					},
				},
				async ({ path }) => {
					try {
						const result = vfs.readFile(path as string);
						if (result.contentType === 'binary') {
							return {
								content: [
									{
										type: 'text' as const,
										text: `[Binary file: ${result.size} bytes]`,
									},
								],
							};
						}
						return {
							content: [{ type: 'text' as const, text: result.text }],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			server.registerTool(
				'vfs-write',
				{
					title: 'VFS Write',
					description: 'Write a file to the virtual filesystem sandbox',
					inputSchema: {
						path: z.string().describe('The file path to write'),
						content: z.string().describe('The file content'),
					},
				},
				async ({ path, content: fileContent }) => {
					try {
						vfs.writeFile(path as string, fileContent as string, {
							createParents: true,
						});
						return {
							content: [
								{
									type: 'text' as const,
									text: `Wrote ${Buffer.byteLength(fileContent as string, 'utf-8')} bytes to ${path}`,
								},
							],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			server.registerTool(
				'vfs-list',
				{
					title: 'VFS List',
					description:
						'List files and directories in the virtual filesystem sandbox',
					inputSchema: {
						path: z
							.string()
							.optional()
							.describe('The directory path to list (default: /)'),
					},
				},
				async ({ path }) => {
					try {
						const entries = vfs.readdir((path as string) ?? '/');
						if (entries.length === 0) {
							return {
								content: [
									{ type: 'text' as const, text: 'Directory is empty.' },
								],
							};
						}
						const text = entries
							.map((e) => `${e.type === 'directory' ? 'd' : 'f'} ${e.name}`)
							.join('\n');
						return {
							content: [{ type: 'text' as const, text }],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			server.registerTool(
				'vfs-tree',
				{
					title: 'VFS Tree',
					description: 'Show a tree view of the virtual filesystem sandbox',
					inputSchema: {
						path: z
							.string()
							.optional()
							.describe('The root path for the tree (default: /)'),
					},
				},
				async ({ path }) => {
					try {
						const tree = vfs.tree((path as string) ?? '/');
						return {
							content: [{ type: 'text' as const, text: tree }],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);
		}

		// -- Task tools --
		if (taskList) {
			server.registerTool(
				'task-create',
				{
					title: 'Task Create',
					description: 'Create a new task to track work',
					inputSchema: {
						subject: z.string().describe('Brief task title'),
						description: z.string().describe('Detailed task description'),
						activeForm: z
							.string()
							.optional()
							.describe('Present continuous form for spinner'),
					},
				},
				async ({ subject, description, activeForm }) => {
					try {
						const task = taskList.create({
							subject: subject as string,
							description: description as string,
							activeForm: activeForm as string | undefined,
						});
						return {
							content: [
								{
									type: 'text' as const,
									text: `Created task #${task.id}: ${task.subject}`,
								},
							],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			server.registerTool(
				'task-get',
				{
					title: 'Task Get',
					description: 'Get full details of a task by ID',
					inputSchema: {
						id: z.string().describe('The task ID'),
					},
				},
				async ({ id }) => {
					const task = taskList.get(id as string);
					if (!task) {
						return {
							content: [
								{
									type: 'text' as const,
									text: `Task not found: ${id}`,
								},
							],
							isError: true,
						};
					}
					return {
						content: [
							{
								type: 'text' as const,
								text: JSON.stringify(task, null, 2),
							},
						],
					};
				},
			);

			server.registerTool(
				'task-update',
				{
					title: 'Task Update',
					description: 'Update a task status or fields',
					inputSchema: {
						id: z.string().describe('The task ID'),
						status: z
							.string()
							.optional()
							.describe('New status: "pending", "in_progress", or "completed"'),
						subject: z.string().optional().describe('New subject'),
						description: z.string().optional().describe('New description'),
					},
				},
				async ({ id, status, subject, description }) => {
					try {
						const task = taskList.update(id as string, {
							...(status && {
								status: status as 'pending' | 'in_progress' | 'completed',
							}),
							...(subject && { subject: subject as string }),
							...(description && {
								description: description as string,
							}),
						});
						if (!task) {
							return {
								content: [
									{
										type: 'text' as const,
										text: `Task not found: ${id}`,
									},
								],
								isError: true,
							};
						}
						return {
							content: [
								{
									type: 'text' as const,
									text: `Updated task #${task.id}: ${task.subject} [${task.status}]`,
								},
							],
						};
					} catch (error) {
						const message = toError(error).message;
						return {
							content: [{ type: 'text' as const, text: `Error: ${message}` }],
							isError: true,
						};
					}
				},
			);

			server.registerTool(
				'task-delete',
				{
					title: 'Task Delete',
					description: 'Delete a task by ID',
					inputSchema: {
						id: z.string().describe('The task ID to delete'),
					},
				},
				async ({ id }) => {
					const deleted = taskList.delete(id as string);
					if (!deleted) {
						return {
							content: [
								{
									type: 'text' as const,
									text: `Task not found: ${id}`,
								},
							],
							isError: true,
						};
					}
					return {
						content: [
							{
								type: 'text' as const,
								text: `Deleted task #${id}`,
							},
						],
					};
				},
			);

			server.registerTool(
				'task-list',
				{
					title: 'Task List',
					description: 'List all tasks with status and dependencies',
					inputSchema: {},
				},
				async () => {
					const tasks = taskList.list();
					if (tasks.length === 0) {
						return {
							content: [{ type: 'text' as const, text: 'No tasks.' }],
						};
					}
					const text = tasks
						.map((t) => {
							let line = `#${t.id} [${t.status}] ${t.subject}`;
							if (t.blockedBy.length > 0)
								line += ` (blocked by: ${t.blockedBy.join(', ')})`;
							return line;
						})
						.join('\n');
					return {
						content: [{ type: 'text' as const, text }],
					};
				},
			);
		}
	};

	// -----------------------------------------------------------------------
	// Resources
	// -----------------------------------------------------------------------

	const registerResources = (): void => {
		server.registerResource(
			'acp-agents',
			'agents://acp',
			{
				description: 'List of available ACP agents across all servers',
				mimeType: 'application/json',
			},
			async (uri) => {
				const allAgents: Array<{
					server: string;
					id: string;
					name?: string;
					description?: string;
				}> = [];

				for (const serverName of acpClient.serverNames) {
					try {
						const agents = await acpClient.listAgents(serverName);
						for (const agent of agents) {
							allAgents.push({
								server: serverName,
								id: agent.id,
								name: agent.name,
								description: agent.description,
							});
						}
					} catch {
						// Server not available, skip
					}
				}

				return {
					contents: [
						{
							uri: uri.href,
							text: JSON.stringify(allAgents, null, 2),
						},
					],
				};
			},
		);

		// Resource: tasks://list
		if (taskList) {
			server.registerResource(
				'tasks-list',
				'tasks://list',
				{
					description: 'Current task list with statuses and dependencies',
					mimeType: 'application/json',
				},
				async (uri) => {
					const tasks = taskList.list();
					return {
						contents: [
							{
								uri: uri.href,
								text: JSON.stringify(tasks, null, 2),
							},
						],
					};
				},
			);
		}

		// Resource: vfs://tree
		if (vfs) {
			server.registerResource(
				'vfs-tree',
				'vfs://tree',
				{
					description: 'Full tree view of the virtual filesystem',
					mimeType: 'text/plain',
				},
				async (uri) => {
					const tree = vfs.tree('/');
					return {
						contents: [
							{
								uri: uri.href,
								text: tree,
							},
						],
					};
				},
			);
		}
	};

	// -----------------------------------------------------------------------
	// Prompts
	// -----------------------------------------------------------------------

	const registerPrompts = (): void => {
		server.registerPrompt(
			'single-prompt',
			{
				title: 'Single Prompt',
				description: 'A reusable prompt template with {variable} substitution',
				argsSchema: {
					template: z
						.string()
						.describe(
							'Prompt template with {variable} placeholders, e.g. "Translate {text} to {language}"',
						),
					variables: z
						.string()
						.describe(
							'JSON object of variable values, e.g. {"text":"hello","language":"French"}',
						),
				},
			},
			({ template, variables }) => {
				let parsedVars: Record<string, string>;
				try {
					parsedVars = JSON.parse(variables);
				} catch {
					return {
						messages: [
							{
								role: 'user' as const,
								content: {
									type: 'text' as const,
									text: 'Error: "variables" is not valid JSON',
								},
							},
						],
					};
				}

				let formatted: string;
				try {
					const pt = createPromptTemplate(template);
					formatted = pt.format(parsedVars);
				} catch (error) {
					const message = toError(error).message;
					return {
						messages: [
							{
								role: 'user' as const,
								content: {
									type: 'text' as const,
									text: `Error: ${message}`,
								},
							},
						],
					};
				}
				return {
					messages: [
						{
							role: 'user' as const,
							content: { type: 'text' as const, text: formatted },
						},
					],
				};
			},
		);
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	let started = false;
	let registered = false;
	let startPromise: Promise<void> | null = null;

	const start = async (): Promise<void> => {
		if (started) return;
		if (startPromise) return startPromise;

		startPromise = (async () => {
			if (!registered) {
				registerTools();
				registerResources();
				registerPrompts();
				registered = true;
			}

			const transport = new StdioServerTransport();
			await server.connect(transport);
			started = true;
		})().finally(() => {
			startPromise = null;
		});

		return startPromise;
	};

	const stop = async (): Promise<void> => {
		await server.close();
	};

	return Object.freeze({
		start,
		stop,
		sendToolListChanged,
		sendResourceListChanged,
		sendPromptListChanged,
	});
}
