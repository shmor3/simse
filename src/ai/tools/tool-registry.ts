// ---------------------------------------------------------------------------
// Tool Registry
//
// Discovers tools (built-in + MCP), formats them for system prompts,
// parses tool calls from model responses, and executes them.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import {
	createToolExecutionError,
	createToolNotFoundError,
} from '../../errors/tools.js';
import { registerMemoryTools, registerVFSTools } from './builtin-tools.js';
import type {
	RegisteredTool,
	ToolCallRequest,
	ToolCallResult,
	ToolDefinition,
	ToolHandler,
	ToolParameter,
	ToolRegistry,
	ToolRegistryOptions,
} from './types.js';

// ---------------------------------------------------------------------------
// Tool call parser
// ---------------------------------------------------------------------------

interface ParsedResponse {
	readonly text: string;
	readonly toolCalls: readonly ToolCallRequest[];
}

function parseToolCallsFromResponse(response: string): ParsedResponse {
	const toolCalls: ToolCallRequest[] = [];

	// Match <tool_use>...</tool_use> blocks
	const pattern = /<tool_use>\s*([\s\S]*?)\s*<\/tool_use>/g;
	let match: RegExpExecArray | null = pattern.exec(response);

	while (match !== null) {
		const jsonStr = match[1].trim();
		try {
			const parsed = JSON.parse(jsonStr) as {
				id?: string;
				name?: string;
				arguments?: Record<string, unknown>;
			};
			if (parsed.name) {
				toolCalls.push(
					Object.freeze({
						id: parsed.id ?? `call_${toolCalls.length + 1}`,
						name: parsed.name,
						arguments: parsed.arguments ?? {},
					}),
				);
			}
		} catch {
			// Malformed JSON — skip this tool call
		}
		match = pattern.exec(response);
	}

	// Strip tool_use blocks from the text
	const text = response
		.replace(/<tool_use>\s*[\s\S]*?\s*<\/tool_use>/g, '')
		.trim();

	return Object.freeze({
		text,
		toolCalls: Object.freeze(toolCalls),
	});
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createToolRegistry(options: ToolRegistryOptions): ToolRegistry {
	const { mcpClient, memoryManager, vfs, permissionResolver } = options;
	const tools = new Map<string, RegisteredTool>();

	// -----------------------------------------------------------------------
	// Registration
	// -----------------------------------------------------------------------

	const register = (definition: ToolDefinition, handler: ToolHandler): void => {
		tools.set(definition.name, { definition, handler });
	};

	const unregister = (name: string): boolean => {
		return tools.delete(name);
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
							((schema as Record<string, unknown>).required as
								| string[]
								| undefined) ?? [];

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
							category: 'other',
						},
						async (args) => {
							const result = await mcpClient.callTool(
								serverName,
								tool.name,
								args,
							);
							if (result.isError) {
								throw createToolExecutionError(qualifiedName, result.content);
							}
							return result.content;
						},
					);
				}
			} catch (err) {
				// Log and skip servers that fail to list tools
				const error = toError(err);
				options.logger?.warn?.(
					`Failed to discover tools from MCP server "${serverName}": ${error.message}`,
				);
			}
		}
	};

	// -----------------------------------------------------------------------
	// Public interface
	// -----------------------------------------------------------------------

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
				output: createToolNotFoundError(call.name).message,
				isError: true,
			});
		}

		// Check permissions if a resolver is configured
		if (permissionResolver) {
			const allowed = await permissionResolver.check(call);
			if (!allowed) {
				return Object.freeze({
					id: call.id,
					name: call.name,
					output: `Permission denied for tool: "${call.name}"`,
					isError: true,
				});
			}
		}

		const start = Date.now();
		try {
			const output = await registered.handler(call.arguments);
			return Object.freeze({
				id: call.id,
				name: call.name,
				output,
				isError: false,
				durationMs: Date.now() - start,
			});
		} catch (err) {
			const error = toError(err);
			return Object.freeze({
				id: call.id,
				name: call.name,
				output: createToolExecutionError(call.name, error.message).message,
				isError: true,
				durationMs: Date.now() - start,
			});
		}
	};

	const discover = async (): Promise<void> => {
		tools.clear();
		registerBuiltins();
		await discoverMCPTools();
	};

	// Build the frozen registry
	const registry: ToolRegistry = Object.freeze({
		discover,
		register,
		unregister,
		getToolDefinitions,
		formatForSystemPrompt,
		execute,
		parseToolCalls: parseToolCallsFromResponse,
		get toolCount() {
			return tools.size;
		},
		get toolNames() {
			return Object.freeze([...tools.keys()]);
		},
	});

	// Register built-ins through the public registry interface
	const registerBuiltins = (): void => {
		if (memoryManager) {
			registerMemoryTools(registry, memoryManager);
		}
		if (vfs) {
			registerVFSTools(registry, vfs);
		}
	};

	registerBuiltins();

	return registry;
}
