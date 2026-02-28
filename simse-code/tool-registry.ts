/**
 * SimSE CLI — Tool Registry
 *
 * Discovers available tools (built-in + MCP), formats them for the
 * system prompt, and executes tool calls from the agentic loop.
 */

import type { Library, Logger, MCPClient, VirtualFS } from 'simse';
import { toError } from 'simse';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ToolParameter {
	readonly type: string;
	readonly description: string;
	readonly required?: boolean;
}

export interface ToolDefinition {
	readonly name: string;
	readonly description: string;
	readonly parameters: Readonly<Record<string, ToolParameter>>;
}

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
}

type ToolHandler = (args: Record<string, unknown>) => Promise<string>;

interface RegisteredTool {
	readonly definition: ToolDefinition;
	readonly handler: ToolHandler;
}

// ---------------------------------------------------------------------------
// Options & Interface
// ---------------------------------------------------------------------------

export interface ToolRegistryOptions {
	readonly mcpClient?: MCPClient;
	readonly library?: Library;
	readonly vfs?: VirtualFS;
	readonly logger?: Logger;
}

export interface ToolRegistry {
	readonly discover: () => Promise<void>;
	readonly getToolDefinitions: () => readonly ToolDefinition[];
	readonly formatForSystemPrompt: () => string;
	readonly execute: (call: ToolCallRequest) => Promise<ToolCallResult>;
	readonly toolCount: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createToolRegistry(options: ToolRegistryOptions): ToolRegistry {
	const { mcpClient, library, vfs } = options;
	const tools = new Map<string, RegisteredTool>();

	// -----------------------------------------------------------------------
	// Registration helper
	// -----------------------------------------------------------------------

	const register = (definition: ToolDefinition, handler: ToolHandler): void => {
		tools.set(definition.name, { definition, handler });
	};

	// -----------------------------------------------------------------------
	// Built-in tools
	// -----------------------------------------------------------------------

	const registerBuiltins = (): void => {
		// -- Library tools --
		if (library) {
			register(
				{
					name: 'library_search',
					description:
						'Search the library for relevant volumes and context. Returns matching volumes ranked by relevance.',
					parameters: {
						query: {
							type: 'string',
							description: 'The search query',
							required: true,
						},
						maxResults: {
							type: 'number',
							description: 'Maximum number of results to return (default: 5)',
						},
					},
				},
				async (args) => {
					const query = String(args.query ?? '');
					const maxResults =
						typeof args.maxResults === 'number' ? args.maxResults : 5;
					const results = await library.search(query, maxResults);
					if (results.length === 0) return 'No matching volumes found.';
					return results
						.map(
							(r, i) =>
								`${i + 1}. [${r.volume.metadata.topic ?? 'uncategorized'}] (score: ${r.score.toFixed(2)})\n   ${r.volume.text}`,
						)
						.join('\n\n');
				},
			);

			register(
				{
					name: 'library_shelve',
					description: 'Shelve a volume in the library for long-term storage.',
					parameters: {
						text: {
							type: 'string',
							description: 'The text content to shelve',
							required: true,
						},
						topic: {
							type: 'string',
							description: 'Topic category for the volume',
							required: true,
						},
					},
				},
				async (args) => {
					const text = String(args.text ?? '');
					const topic = String(args.topic ?? 'general');
					const id = await library.add(text, { topic });
					return `Shelved volume with ID: ${id}`;
				},
			);
		}

		// -- VFS tools --
		if (vfs) {
			register(
				{
					name: 'vfs_read',
					description: 'Read a file from the virtual filesystem sandbox.',
					parameters: {
						path: {
							type: 'string',
							description:
								'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							required: true,
						},
					},
				},
				async (args) => {
					const path = String(args.path ?? 'vfs:///');
					const result = vfs.readFile(path);
					if (result.contentType === 'binary') {
						return `[Binary file: ${result.size} bytes]`;
					}
					return result.text;
				},
			);

			register(
				{
					name: 'vfs_write',
					description: 'Write a file to the virtual filesystem sandbox.',
					parameters: {
						path: {
							type: 'string',
							description:
								'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
							required: true,
						},
						content: {
							type: 'string',
							description: 'The file content to write',
							required: true,
						},
					},
				},
				async (args) => {
					const path = String(args.path ?? 'vfs:///');
					const content = String(args.content ?? '');
					vfs.writeFile(path, content, { createParents: true });
					return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
				},
			);

			register(
				{
					name: 'vfs_list',
					description:
						'List files and directories in the virtual filesystem sandbox.',
					parameters: {
						path: {
							type: 'string',
							description:
								'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
						},
					},
				},
				async (args) => {
					const path = String(args.path ?? 'vfs:///');
					const entries = vfs.readdir(path);
					if (entries.length === 0) return 'Directory is empty.';
					return entries
						.map((e) => `${e.type === 'directory' ? 'd' : 'f'} ${e.name}`)
						.join('\n');
				},
			);

			register(
				{
					name: 'vfs_tree',
					description: 'Show a tree view of the virtual filesystem sandbox.',
					parameters: {
						path: {
							type: 'string',
							description:
								'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
						},
					},
				},
				async (args) => {
					const path = String(args.path ?? 'vfs:///');
					return vfs.tree(path);
				},
			);
		}
	};

	// -----------------------------------------------------------------------
	// MCP tool discovery
	// -----------------------------------------------------------------------

	const discoverMCPTools = async (): Promise<void> => {
		if (!mcpClient) return;

		for (const serverName of mcpClient.connectedServerNames) {
			try {
				const mcpTools = await mcpClient.listTools(serverName);
				for (const tool of mcpTools) {
					const qualifiedName = `mcp:${serverName}/${tool.name}`;
					const params: Record<string, ToolParameter> = {};

					// Extract parameters from JSON Schema input schema
					const schema = tool.inputSchema;
					if (schema && typeof schema === 'object') {
						const props = (schema as Record<string, unknown>).properties as
							| Record<string, Record<string, unknown>>
							| undefined;
						const required =
							((schema as Record<string, unknown>).required as string[]) ?? [];

						if (props) {
							for (const [key, prop] of Object.entries(props)) {
								params[key] = {
									type: String(prop.type ?? 'string'),
									description: String(prop.description ?? ''),
									required: required.includes(key),
								};
							}
						}
					}

					register(
						{
							name: qualifiedName,
							description: tool.description ?? `MCP tool: ${tool.name}`,
							parameters: params,
						},
						async (args) => {
							const result = await mcpClient.callTool(
								serverName,
								tool.name,
								args,
							);
							if (result.isError) {
								throw new Error(result.content);
							}
							return result.content;
						},
					);
				}
			} catch {
				// Skip servers that fail to list tools
			}
		}
	};

	// -----------------------------------------------------------------------
	// Public interface
	// -----------------------------------------------------------------------

	const discover = async (): Promise<void> => {
		tools.clear();
		registerBuiltins();
		await discoverMCPTools();
	};

	const getToolDefinitions = (): readonly ToolDefinition[] => {
		return Object.freeze([...tools.values()].map((t) => t.definition));
	};

	const formatForSystemPrompt = (): string => {
		if (tools.size === 0) return '';

		const lines: string[] = [
			'You have access to tools. To use a tool, include a JSON block wrapped in <tool_use> tags:',
			'',
			'<tool_use>',
			'{"id": "call_1", "name": "tool_name", "arguments": {"key": "value"}}',
			'</tool_use>',
			'',
			'You can call multiple tools in one response. After tool results are provided, continue your response.',
			'Only use tools when necessary — if you can answer directly, do so.',
			'',
			'Available tools:',
			'',
		];

		for (const tool of tools.values()) {
			lines.push(`- ${tool.definition.name}: ${tool.definition.description}`);
			const paramEntries = Object.entries(tool.definition.parameters);
			if (paramEntries.length > 0) {
				const paramDesc = paramEntries
					.map(([k, v]) => `${k} (${v.type}${v.required ? ', required' : ''})`)
					.join(', ');
				lines.push(`  Parameters: ${paramDesc}`);
			}
			lines.push('');
		}

		return lines.join('\n');
	};

	const execute = async (call: ToolCallRequest): Promise<ToolCallResult> => {
		const registered = tools.get(call.name);
		if (!registered) {
			return Object.freeze({
				id: call.id,
				name: call.name,
				output: `Unknown tool: "${call.name}"`,
				isError: true,
			});
		}

		try {
			const output = await registered.handler(call.arguments);
			return Object.freeze({
				id: call.id,
				name: call.name,
				output,
				isError: false,
			});
		} catch (err) {
			return Object.freeze({
				id: call.id,
				name: call.name,
				output: `Tool error: ${toError(err).message}`,
				isError: true,
			});
		}
	};

	// Register built-ins immediately
	registerBuiltins();

	return Object.freeze({
		discover,
		getToolDefinitions,
		formatForSystemPrompt,
		execute,
		get toolCount() {
			return tools.size;
		},
	});
}
