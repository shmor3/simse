// ---------------------------------------------------------------------------
// MCP Server — thin wrapper over Rust MCP engine
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import { createChain } from '../chain/chain.js';
import { createPromptTemplate } from '../chain/prompt-template.js';
import type { Conversation } from '../conversation/types.js';
import type { Library } from '../library/library.js';
import type { TaskList } from '../tasks/types.js';
import type { ToolRegistry } from '../tools/types.js';
import type { VirtualFS } from '../vfs/vfs.js';
import {
	createMcpEngineClient,
	type McpEngineClient,
} from './mcp-engine-client.js';
import type { MCPServerConfig } from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface MCPServerOptions {
	/** ACP client used for generate, run-chain, and list-agents tools. */
	readonly acpClient: ACPClient;
	/** Optional library for library-search and library-shelve tools. */
	readonly library?: Library;
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
// Tool execution request / response types (internal)
// ---------------------------------------------------------------------------

interface ToolExecuteRequest {
	readonly requestId: string;
	readonly toolName: string;
	readonly args: Record<string, unknown>;
}

interface ToolExecuteResult {
	readonly requestId: string;
	readonly content: ReadonlyArray<{
		readonly type: string;
		readonly text: string;
	}>;
	readonly isError?: boolean;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an MCP server that exposes simse capabilities (generate, run-chain,
 * list-agents, library, VFS, and task tools) over the Model Context Protocol.
 *
 * @param config - Server name and version.
 * @param options - ACP client (required), plus optional library, VFS, task list.
 * @returns A frozen {@link SimseMCPServer}. Call `start()` to begin serving over stdio.
 */
export function createMCPServer(
	config: MCPServerConfig,
	options: MCPServerOptions,
): SimseMCPServer {
	const acpClient = options.acpClient;
	const library = options.library;
	const vfs = options.vfs;
	const taskList = options.taskList;
	// options.toolRegistry and options.conversation are reserved for future use

	let engineClient: McpEngineClient | undefined;
	let started = false;
	let startPromise: Promise<void> | null = null;

	// -----------------------------------------------------------------------
	// Tool handler dispatch
	// -----------------------------------------------------------------------

	const handleToolExecute = async (
		request: ToolExecuteRequest,
	): Promise<ToolExecuteResult> => {
		const { requestId, toolName, args } = request;

		try {
			const result = await executeToolHandler(toolName, args);
			return { requestId, ...result };
		} catch (error) {
			const message = toError(error).message;
			return {
				requestId,
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -----------------------------------------------------------------------
	// Tool handlers (same logic as before, just extracted)
	// -----------------------------------------------------------------------

	const executeToolHandler = async (
		toolName: string,
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		switch (toolName) {
			case 'generate':
				return handleGenerate(args);
			case 'run-chain':
				return handleRunChain(args);
			case 'list-agents':
				return handleListAgents(args);
			case 'library-search':
				return handleLibrarySearch(args);
			case 'library-shelve':
				return handleLibraryShelve(args);
			case 'library-withdraw':
				return handleLibraryWithdraw(args);
			case 'vfs-read':
				return handleVfsRead(args);
			case 'vfs-write':
				return handleVfsWrite(args);
			case 'vfs-list':
				return handleVfsList(args);
			case 'vfs-tree':
				return handleVfsTree(args);
			case 'task-create':
				return handleTaskCreate(args);
			case 'task-get':
				return handleTaskGet(args);
			case 'task-update':
				return handleTaskUpdate(args);
			case 'task-delete':
				return handleTaskDelete(args);
			case 'task-list':
				return handleTaskList();
			default:
				return {
					content: [{ type: 'text', text: `Unknown tool: ${toolName}` }],
					isError: true,
				};
		}
	};

	// -- generate --
	const handleGenerate = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		try {
			const res = await acpClient.generate(args.prompt as string, {
				agentId: args.agentId as string | undefined,
				serverName: args.serverName as string | undefined,
				systemPrompt: args.systemPrompt as string | undefined,
			});
			return { content: [{ type: 'text', text: res.content }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- run-chain --
	const handleRunChain = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		let parsedSteps: Array<{
			name: string;
			template: string;
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
		}>;
		let parsedValues: Record<string, string>;

		try {
			parsedSteps = JSON.parse(args.steps as string);
		} catch {
			return {
				content: [{ type: 'text', text: 'Error: "steps" is not valid JSON' }],
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
						type: 'text',
						text: 'Error: "steps" must be a JSON array of objects with "name" and "template" string fields',
					},
				],
				isError: true,
			};
		}

		try {
			parsedValues = JSON.parse(args.values as string);
		} catch {
			return {
				content: [{ type: 'text', text: 'Error: "values" is not valid JSON' }],
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
					{ type: 'text', text: 'Error: "values" must be a JSON object' },
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
						type: 'text',
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

			return { content: [{ type: 'text', text: formatted }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- list-agents --
	const handleListAgents = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		const lines: string[] = [];
		const serverName = args.serverName as string | undefined;

		if (serverName) {
			const available = await acpClient.isAvailable(serverName);
			lines.push(`${serverName}: ${available ? 'available' : 'unavailable'}`);
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
					lines.push(`  (could not list agents: ${toError(err).message})`);
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

		return { content: [{ type: 'text', text: lines.join('\n') }] };
	};

	// -- library-search --
	const handleLibrarySearch = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!library) {
			return {
				content: [{ type: 'text', text: 'Error: Library not configured' }],
				isError: true,
			};
		}
		try {
			const results = await library.search(
				args.query as string,
				(args.maxResults as number | undefined) ?? 10,
			);
			return {
				content: [{ type: 'text', text: JSON.stringify(results, null, 2) }],
			};
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- library-shelve --
	const handleLibraryShelve = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!library) {
			return {
				content: [{ type: 'text', text: 'Error: Library not configured' }],
				isError: true,
			};
		}
		try {
			const meta = args.metadata
				? JSON.parse(args.metadata as string)
				: undefined;
			const id = await library.add(args.text as string, meta);
			return { content: [{ type: 'text', text: `Shelved volume: ${id}` }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- library-withdraw --
	const handleLibraryWithdraw = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!library) {
			return {
				content: [{ type: 'text', text: 'Error: Library not configured' }],
				isError: true,
			};
		}
		try {
			const deleted = await library.delete(args.id as string);
			if (!deleted) {
				return {
					content: [{ type: 'text', text: `Volume not found: ${args.id}` }],
					isError: true,
				};
			}
			return {
				content: [{ type: 'text', text: `Withdrew volume: ${args.id}` }],
			};
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- vfs-read --
	const handleVfsRead = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!vfs) {
			return {
				content: [{ type: 'text', text: 'Error: VFS not configured' }],
				isError: true,
			};
		}
		try {
			const result = await vfs.readFile(args.path as string);
			if (result.contentType === 'binary') {
				return {
					content: [
						{ type: 'text', text: `[Binary file: ${result.size} bytes]` },
					],
				};
			}
			return { content: [{ type: 'text', text: result.text }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- vfs-write --
	const handleVfsWrite = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!vfs) {
			return {
				content: [{ type: 'text', text: 'Error: VFS not configured' }],
				isError: true,
			};
		}
		try {
			const fileContent = args.content as string;
			await vfs.writeFile(args.path as string, fileContent, {
				createParents: true,
			});
			return {
				content: [
					{
						type: 'text',
						text: `Wrote ${Buffer.byteLength(fileContent, 'utf-8')} bytes to ${args.path}`,
					},
				],
			};
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- vfs-list --
	const handleVfsList = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!vfs) {
			return {
				content: [{ type: 'text', text: 'Error: VFS not configured' }],
				isError: true,
			};
		}
		try {
			const entries = await vfs.readdir((args.path as string) ?? 'vfs:///');
			if (entries.length === 0) {
				return {
					content: [{ type: 'text', text: 'Directory is empty.' }],
				};
			}
			const text = entries
				.map((e) => `${e.type === 'directory' ? 'd' : 'f'} ${e.name}`)
				.join('\n');
			return { content: [{ type: 'text', text }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- vfs-tree --
	const handleVfsTree = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!vfs) {
			return {
				content: [{ type: 'text', text: 'Error: VFS not configured' }],
				isError: true,
			};
		}
		try {
			const tree = await vfs.tree((args.path as string) ?? 'vfs:///');
			return { content: [{ type: 'text', text: tree }] };
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- task-create --
	const handleTaskCreate = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!taskList) {
			return {
				content: [{ type: 'text', text: 'Error: Task list not configured' }],
				isError: true,
			};
		}
		try {
			const task = taskList.create({
				subject: args.subject as string,
				description: args.description as string,
				activeForm: args.activeForm as string | undefined,
			});
			return {
				content: [
					{ type: 'text', text: `Created task #${task.id}: ${task.subject}` },
				],
			};
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- task-get --
	const handleTaskGet = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!taskList) {
			return {
				content: [{ type: 'text', text: 'Error: Task list not configured' }],
				isError: true,
			};
		}
		const task = taskList.get(args.id as string);
		if (!task) {
			return {
				content: [{ type: 'text', text: `Task not found: ${args.id}` }],
				isError: true,
			};
		}
		return {
			content: [{ type: 'text', text: JSON.stringify(task, null, 2) }],
		};
	};

	// -- task-update --
	const handleTaskUpdate = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!taskList) {
			return {
				content: [{ type: 'text', text: 'Error: Task list not configured' }],
				isError: true,
			};
		}
		try {
			const updateInput: {
				status?: 'pending' | 'in_progress' | 'completed';
				subject?: string;
				description?: string;
			} = {};
			if (args.status) {
				updateInput.status = args.status as
					| 'pending'
					| 'in_progress'
					| 'completed';
			}
			if (args.subject) {
				updateInput.subject = args.subject as string;
			}
			if (args.description) {
				updateInput.description = args.description as string;
			}
			const task = taskList.update(args.id as string, updateInput);
			if (!task) {
				return {
					content: [{ type: 'text', text: `Task not found: ${args.id}` }],
					isError: true,
				};
			}
			return {
				content: [
					{
						type: 'text',
						text: `Updated task #${task.id}: ${task.subject} [${task.status}]`,
					},
				],
			};
		} catch (error) {
			const message = toError(error).message;
			return {
				content: [{ type: 'text', text: `Error: ${message}` }],
				isError: true,
			};
		}
	};

	// -- task-delete --
	const handleTaskDelete = async (
		args: Record<string, unknown>,
	): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!taskList) {
			return {
				content: [{ type: 'text', text: 'Error: Task list not configured' }],
				isError: true,
			};
		}
		const deleted = taskList.delete(args.id as string);
		if (!deleted) {
			return {
				content: [{ type: 'text', text: `Task not found: ${args.id}` }],
				isError: true,
			};
		}
		return {
			content: [{ type: 'text', text: `Deleted task #${args.id}` }],
		};
	};

	// -- task-list --
	const handleTaskList = async (): Promise<{
		content: Array<{ type: string; text: string }>;
		isError?: boolean;
	}> => {
		if (!taskList) {
			return {
				content: [{ type: 'text', text: 'Error: Task list not configured' }],
				isError: true,
			};
		}
		const tasks = taskList.list();
		if (tasks.length === 0) {
			return { content: [{ type: 'text', text: 'No tasks.' }] };
		}
		const text = tasks
			.map((t) => {
				let line = `#${t.id} [${t.status}] ${t.subject}`;
				if (t.blockedBy.length > 0)
					line += ` (blocked by: ${t.blockedBy.join(', ')})`;
				return line;
			})
			.join('\n');
		return { content: [{ type: 'text', text }] };
	};

	// -----------------------------------------------------------------------
	// Build tool definitions for the engine
	// -----------------------------------------------------------------------

	const buildToolDefinitions = (): Array<{
		name: string;
		title: string;
		description: string;
		inputSchema: Record<string, unknown>;
	}> => {
		const tools: Array<{
			name: string;
			title: string;
			description: string;
			inputSchema: Record<string, unknown>;
		}> = [];

		// Always register core tools
		tools.push(
			{
				name: 'generate',
				title: 'Generate Text',
				description: 'Generate text from a prompt using an ACP agent',
				inputSchema: {
					type: 'object',
					properties: {
						prompt: {
							type: 'string',
							description: 'The prompt to send to the agent',
						},
						agentId: {
							type: 'string',
							description:
								'ACP agent ID to use (defaults to configured default)',
						},
						serverName: {
							type: 'string',
							description:
								'ACP server name to use (defaults to configured default)',
						},
						systemPrompt: {
							type: 'string',
							description: 'System prompt',
						},
					},
					required: ['prompt'],
				},
			},
			{
				name: 'run-chain',
				title: 'Run Chain',
				description:
					'Execute a multi-step LangChain pipeline. Provide steps as a JSON array and initial values as a JSON object.',
				inputSchema: {
					type: 'object',
					properties: {
						steps: {
							type: 'string',
							description:
								'JSON array of step objects: [{"name":"step1","template":"...","agentId":"my-agent"}]',
						},
						values: {
							type: 'string',
							description: 'JSON object of initial template variable values',
						},
					},
					required: ['steps', 'values'],
				},
			},
			{
				name: 'list-agents',
				title: 'List Agents',
				description: 'List available ACP agents across configured servers',
				inputSchema: {
					type: 'object',
					properties: {
						serverName: {
							type: 'string',
							description:
								'Specific ACP server to query (defaults to all configured)',
						},
					},
				},
			},
		);

		// Library tools
		if (library) {
			tools.push(
				{
					name: 'library-search',
					title: 'Library Search',
					description: 'Search the library for relevant volumes',
					inputSchema: {
						type: 'object',
						properties: {
							query: { type: 'string', description: 'Search query' },
							maxResults: {
								type: 'number',
								description: 'Max results (default 10)',
							},
						},
						required: ['query'],
					},
				},
				{
					name: 'library-shelve',
					title: 'Library Shelve',
					description: 'Shelve a volume in the library',
					inputSchema: {
						type: 'object',
						properties: {
							text: { type: 'string', description: 'Text to shelve' },
							metadata: {
								type: 'string',
								description: 'Optional JSON metadata',
							},
						},
						required: ['text'],
					},
				},
				{
					name: 'library-withdraw',
					title: 'Library Withdraw',
					description: 'Withdraw a volume from the library',
					inputSchema: {
						type: 'object',
						properties: {
							id: {
								type: 'string',
								description: 'The volume ID to withdraw',
							},
						},
						required: ['id'],
					},
				},
			);
		}

		// VFS tools
		if (vfs) {
			tools.push(
				{
					name: 'vfs-read',
					title: 'VFS Read',
					description: 'Read a file from the virtual filesystem sandbox',
					inputSchema: {
						type: 'object',
						properties: {
							path: {
								type: 'string',
								description:
									'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							},
						},
						required: ['path'],
					},
				},
				{
					name: 'vfs-write',
					title: 'VFS Write',
					description: 'Write a file to the virtual filesystem sandbox',
					inputSchema: {
						type: 'object',
						properties: {
							path: {
								type: 'string',
								description:
									'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							},
							content: {
								type: 'string',
								description: 'The file content',
							},
						},
						required: ['path', 'content'],
					},
				},
				{
					name: 'vfs-list',
					title: 'VFS List',
					description:
						'List files and directories in the virtual filesystem sandbox',
					inputSchema: {
						type: 'object',
						properties: {
							path: {
								type: 'string',
								description:
									'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							},
						},
					},
				},
				{
					name: 'vfs-tree',
					title: 'VFS Tree',
					description: 'Show a tree view of the virtual filesystem sandbox',
					inputSchema: {
						type: 'object',
						properties: {
							path: {
								type: 'string',
								description:
									'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							},
						},
					},
				},
			);
		}

		// Task tools
		if (taskList) {
			tools.push(
				{
					name: 'task-create',
					title: 'Task Create',
					description: 'Create a new task to track work',
					inputSchema: {
						type: 'object',
						properties: {
							subject: {
								type: 'string',
								description: 'Brief task title',
							},
							description: {
								type: 'string',
								description: 'Detailed task description',
							},
							activeForm: {
								type: 'string',
								description: 'Present continuous form for spinner',
							},
						},
						required: ['subject', 'description'],
					},
				},
				{
					name: 'task-get',
					title: 'Task Get',
					description: 'Get full details of a task by ID',
					inputSchema: {
						type: 'object',
						properties: {
							id: { type: 'string', description: 'The task ID' },
						},
						required: ['id'],
					},
				},
				{
					name: 'task-update',
					title: 'Task Update',
					description: 'Update a task status or fields',
					inputSchema: {
						type: 'object',
						properties: {
							id: { type: 'string', description: 'The task ID' },
							status: {
								type: 'string',
								description:
									'New status: "pending", "in_progress", or "completed"',
							},
							subject: {
								type: 'string',
								description: 'New subject',
							},
							description: {
								type: 'string',
								description: 'New description',
							},
						},
						required: ['id'],
					},
				},
				{
					name: 'task-delete',
					title: 'Task Delete',
					description: 'Delete a task by ID',
					inputSchema: {
						type: 'object',
						properties: {
							id: {
								type: 'string',
								description: 'The task ID to delete',
							},
						},
						required: ['id'],
					},
				},
				{
					name: 'task-list',
					title: 'Task List',
					description: 'List all tasks with status and dependencies',
					inputSchema: {
						type: 'object',
						properties: {},
					},
				},
			);
		}

		return tools;
	};

	// -----------------------------------------------------------------------
	// Build resource definitions for the engine
	// -----------------------------------------------------------------------

	const buildResourceDefinitions = (): Array<{
		name: string;
		uri: string;
		description: string;
		mimeType: string;
	}> => {
		const resources: Array<{
			name: string;
			uri: string;
			description: string;
			mimeType: string;
		}> = [];

		resources.push({
			name: 'acp-agents',
			uri: 'agents://acp',
			description: 'List of available ACP agents across all servers',
			mimeType: 'application/json',
		});

		if (taskList) {
			resources.push({
				name: 'tasks-list',
				uri: 'tasks://list',
				description: 'Current task list with statuses and dependencies',
				mimeType: 'application/json',
			});
		}

		if (vfs) {
			resources.push({
				name: 'vfs-tree',
				uri: 'vfs://tree',
				description: 'Full tree view of the virtual filesystem',
				mimeType: 'text/plain',
			});
		}

		return resources;
	};

	// -----------------------------------------------------------------------
	// Build prompt definitions for the engine
	// -----------------------------------------------------------------------

	const buildPromptDefinitions = (): Array<{
		name: string;
		title: string;
		description: string;
		argsSchema: Record<string, unknown>;
	}> => {
		return [
			{
				name: 'single-prompt',
				title: 'Single Prompt',
				description: 'A reusable prompt template with {variable} substitution',
				argsSchema: {
					type: 'object',
					properties: {
						template: {
							type: 'string',
							description:
								'Prompt template with {variable} placeholders, e.g. "Translate {text} to {language}"',
						},
						variables: {
							type: 'string',
							description:
								'JSON object of variable values, e.g. {"text":"hello","language":"French"}',
						},
					},
					required: ['template', 'variables'],
				},
			},
		];
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	const start = async (): Promise<void> => {
		if (started) return;
		if (startPromise) return startPromise;

		startPromise = (async () => {
			const enginePath =
				process.env.SIMSE_MCP_ENGINE_PATH ?? 'simse-mcp-engine';
			engineClient = createMcpEngineClient({
				enginePath,
				logger: undefined, // Engine logs go to stderr
			});

			// Subscribe to tool execution requests from the engine
			engineClient.onNotification('tool/execute', (params: unknown) => {
				const request = params as ToolExecuteRequest;
				handleToolExecute(request)
					.then((result) => {
						return engineClient?.request<void>('server/toolResult', result);
					})
					.catch(() => {
						// Ignore errors sending result back
					});
			});

			// Subscribe to resource read requests from the engine
			engineClient.onNotification('resource/read', (params: unknown) => {
				const request = params as {
					requestId: string;
					uri: string;
				};
				handleResourceRead(request)
					.then((result) => {
						return engineClient?.request<void>('server/resourceResult', result);
					})
					.catch(() => {
						// Ignore errors sending result back
					});
			});

			// Subscribe to prompt get requests from the engine
			engineClient.onNotification('prompt/get', (params: unknown) => {
				const request = params as {
					requestId: string;
					name: string;
					args: Record<string, string>;
				};
				handlePromptGet(request)
					.then((result) => {
						return engineClient?.request<void>('server/promptResult', result);
					})
					.catch(() => {
						// Ignore errors sending result back
					});
			});

			// Register tools, resources, and prompts with the engine, then start
			await engineClient.request<void>('server/start', {
				name: config.name,
				version: config.version,
				tools: buildToolDefinitions(),
				resources: buildResourceDefinitions(),
				prompts: buildPromptDefinitions(),
			});

			started = true;
		})().finally(() => {
			startPromise = null;
		});

		return startPromise;
	};

	// Resource read handler
	const handleResourceRead = async (request: {
		requestId: string;
		uri: string;
	}): Promise<{
		requestId: string;
		contents: Array<{ uri: string; text: string }>;
	}> => {
		const { requestId, uri } = request;

		if (uri === 'agents://acp') {
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
				requestId,
				contents: [{ uri, text: JSON.stringify(allAgents, null, 2) }],
			};
		}

		if (uri === 'tasks://list' && taskList) {
			const tasks = taskList.list();
			return {
				requestId,
				contents: [{ uri, text: JSON.stringify(tasks, null, 2) }],
			};
		}

		if (uri === 'vfs://tree' && vfs) {
			const tree = await vfs.tree('vfs:///');
			return {
				requestId,
				contents: [{ uri, text: tree }],
			};
		}

		return {
			requestId,
			contents: [{ uri, text: 'Resource not found' }],
		};
	};

	// Prompt get handler
	const handlePromptGet = async (request: {
		requestId: string;
		name: string;
		args: Record<string, string>;
	}): Promise<{
		requestId: string;
		messages: Array<{
			role: string;
			content: { type: string; text: string };
		}>;
	}> => {
		const { requestId, name, args } = request;

		if (name === 'single-prompt') {
			let parsedVars: Record<string, string>;
			try {
				parsedVars = JSON.parse(args.variables);
			} catch {
				return {
					requestId,
					messages: [
						{
							role: 'user',
							content: {
								type: 'text',
								text: 'Error: "variables" is not valid JSON',
							},
						},
					],
				};
			}

			let formatted: string;
			try {
				const pt = createPromptTemplate(args.template);
				formatted = pt.format(parsedVars);
			} catch (error) {
				const message = toError(error).message;
				return {
					requestId,
					messages: [
						{
							role: 'user',
							content: { type: 'text', text: `Error: ${message}` },
						},
					],
				};
			}

			return {
				requestId,
				messages: [
					{
						role: 'user',
						content: { type: 'text', text: formatted },
					},
				],
			};
		}

		return {
			requestId,
			messages: [
				{
					role: 'user',
					content: { type: 'text', text: `Unknown prompt: ${name}` },
				},
			],
		};
	};

	const stop = async (): Promise<void> => {
		if (engineClient) {
			try {
				await engineClient.request<void>('server/stop', {});
			} catch {
				// Ignore
			}
			await engineClient.dispose();
			engineClient = undefined;
		}
		started = false;
	};

	const sendToolListChanged = (): void => {
		if (engineClient) {
			engineClient.request<void>('server/sendToolListChanged', {}).catch(() => {
				/* ignore */
			});
		}
	};

	const sendResourceListChanged = (): void => {
		if (engineClient) {
			engineClient
				.request<void>('server/sendResourceListChanged', {})
				.catch(() => {
					/* ignore */
				});
		}
	};

	const sendPromptListChanged = (): void => {
		if (engineClient) {
			engineClient
				.request<void>('server/sendPromptListChanged', {})
				.catch(() => {
					/* ignore */
				});
		}
	};

	return Object.freeze({
		start,
		stop,
		sendToolListChanged,
		sendResourceListChanged,
		sendPromptListChanged,
	});
}
