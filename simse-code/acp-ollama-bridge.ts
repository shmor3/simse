// ---------------------------------------------------------------------------
// ACP-Ollama Bridge — JSON-RPC 2.0 over NDJSON stdio
// ---------------------------------------------------------------------------
//
// Implements the server side of the ACP protocol over stdin/stdout.
// Translates ACP session/prompt requests into Ollama /api/generate and
// /api/embed calls. Streams via session/update notifications.
//
// Usage:
//   bun run acp-ollama-bridge.ts
//   bun run acp-ollama-bridge.ts --ollama http://10.0.0.103:11434
//   bun run acp-ollama-bridge.ts --model llama3.2 --embedding-model nomic-embed-text
//
// ---------------------------------------------------------------------------

import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
import type {
	ACPContentBlock,
	ACPInitializeResult,
	ACPSessionNewResult,
	ACPSessionPromptParams,
	ACPSessionPromptResult,
	ACPSessionUpdateParams,
	ACPStopReason,
	JsonRpcError,
	JsonRpcMessage,
	JsonRpcNotification,
	JsonRpcRequest,
	JsonRpcResponse,
} from '../src/ai/acp/types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface BridgeOptions {
	/** Ollama base URL. */
	readonly ollamaUrl: string;
	/** Default model for text generation. */
	readonly defaultModel: string;
	/** Default model for embeddings. */
	readonly embeddingModel: string;
	/** Server name advertised in initialize response. */
	readonly serverName: string;
	/** Server version advertised in initialize response. */
	readonly serverVersion: string;
	/** Whether to stream generation via session/update notifications. */
	readonly streaming: boolean;
}

// ---------------------------------------------------------------------------
// Ollama API types (minimal)
// ---------------------------------------------------------------------------

interface OllamaGenerateChunk {
	readonly response?: string;
	readonly done?: boolean;
	readonly prompt_eval_count?: number;
	readonly eval_count?: number;
}

interface OllamaChatMessage {
	readonly role: 'system' | 'user' | 'assistant' | 'tool';
	readonly content: string;
	readonly tool_calls?: readonly OllamaToolCall[];
}

interface OllamaToolCall {
	readonly function: {
		readonly name: string;
		readonly arguments: Record<string, unknown>;
	};
}

interface OllamaTool {
	readonly type: 'function';
	readonly function: {
		readonly name: string;
		readonly description: string;
		readonly parameters: {
			readonly type: 'object';
			readonly properties: Record<
				string,
				{ readonly type: string; readonly description: string }
			>;
			readonly required: readonly string[];
		};
	};
}

interface OllamaChatResponse {
	readonly message: {
		readonly role: string;
		readonly content: string;
		readonly tool_calls?: readonly OllamaToolCall[];
	};
	readonly done: boolean;
	readonly prompt_eval_count?: number;
	readonly eval_count?: number;
}

// ---------------------------------------------------------------------------
// JSON-RPC error codes
// ---------------------------------------------------------------------------

const JSON_RPC_ERRORS = {
	PARSE_ERROR: -32700,
	INVALID_REQUEST: -32600,
	METHOD_NOT_FOUND: -32601,
	INVALID_PARAMS: -32602,
	INTERNAL_ERROR: -32603,
} as const;

// ---------------------------------------------------------------------------
// Ollama client
// ---------------------------------------------------------------------------

interface OllamaClient {
	readonly generate: (
		model: string,
		prompt: string,
		system: string | undefined,
	) => Promise<OllamaGenerateChunk>;
	readonly generateStream: (
		model: string,
		prompt: string,
		system: string | undefined,
		onChunk: (text: string) => void,
	) => Promise<{ promptTokens: number; completionTokens: number }>;
	readonly chat: (
		model: string,
		messages: readonly OllamaChatMessage[],
		tools?: readonly OllamaTool[],
	) => Promise<OllamaChatResponse>;
	readonly chatStream: (
		model: string,
		messages: readonly OllamaChatMessage[],
		tools: readonly OllamaTool[] | undefined,
		onChunk: (text: string) => void,
	) => Promise<{
		promptTokens: number;
		completionTokens: number;
		toolCalls: readonly OllamaToolCall[];
	}>;
	readonly embed: (
		model: string,
		texts: readonly string[],
	) => Promise<{
		embeddings: readonly (readonly number[])[];
		promptTokens: number;
	}>;
}

function createOllamaClient(baseUrl: string): OllamaClient {
	const post = async (path: string, body: unknown): Promise<Response> => {
		const response = await fetch(`${baseUrl}${path}`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify(body),
		});
		if (!response.ok) {
			const text = await response.text().catch(() => '');
			throw new Error(`Ollama ${path} ${response.status}: ${text}`);
		}
		return response;
	};

	const generate = async (
		model: string,
		prompt: string,
		system: string | undefined,
	): Promise<OllamaGenerateChunk> => {
		const body: Record<string, unknown> = { model, prompt, stream: false };
		if (system) body.system = system;
		const response = await post('/api/generate', body);
		return response.json() as Promise<OllamaGenerateChunk>;
	};

	const generateStream = async (
		model: string,
		prompt: string,
		system: string | undefined,
		onChunk: (text: string) => void,
	): Promise<{ promptTokens: number; completionTokens: number }> => {
		const body: Record<string, unknown> = { model, prompt, stream: true };
		if (system) body.system = system;

		const response = await post('/api/generate', body);
		if (!response.body) {
			throw new Error('Ollama returned no response body for streaming');
		}

		const reader = response.body.getReader();
		const decoder = new TextDecoder();
		let lineBuffer = '';
		let promptTokens = 0;
		let completionTokens = 0;

		try {
			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				lineBuffer += decoder.decode(value, { stream: true });
				const lines = lineBuffer.split('\n');
				lineBuffer = lines.pop() ?? '';

				for (const line of lines) {
					const trimmed = line.trim();
					if (!trimmed) continue;
					let chunk: OllamaGenerateChunk;
					try {
						chunk = JSON.parse(trimmed) as OllamaGenerateChunk;
					} catch {
						continue;
					}
					if (chunk.done) {
						promptTokens = chunk.prompt_eval_count ?? 0;
						completionTokens = chunk.eval_count ?? 0;
					}
					if (chunk.response) {
						onChunk(chunk.response);
					}
				}
			}
		} finally {
			reader.releaseLock();
		}

		return { promptTokens, completionTokens };
	};

	const chat = async (
		model: string,
		messages: readonly OllamaChatMessage[],
		tools?: readonly OllamaTool[],
	): Promise<OllamaChatResponse> => {
		const body: Record<string, unknown> = {
			model,
			messages,
			stream: false,
		};
		if (tools && tools.length > 0) body.tools = tools;
		const response = await post('/api/chat', body);
		return response.json() as Promise<OllamaChatResponse>;
	};

	const chatStream = async (
		model: string,
		messages: readonly OllamaChatMessage[],
		tools: readonly OllamaTool[] | undefined,
		onChunk: (text: string) => void,
	): Promise<{
		promptTokens: number;
		completionTokens: number;
		toolCalls: readonly OllamaToolCall[];
	}> => {
		const body: Record<string, unknown> = {
			model,
			messages,
			stream: true,
		};
		if (tools && tools.length > 0) body.tools = tools;

		const response = await post('/api/chat', body);
		if (!response.body) {
			throw new Error('Ollama returned no response body for streaming');
		}

		const reader = response.body.getReader();
		const decoder = new TextDecoder();
		let lineBuffer = '';
		let promptTokens = 0;
		let completionTokens = 0;
		const toolCalls: OllamaToolCall[] = [];

		try {
			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				lineBuffer += decoder.decode(value, { stream: true });
				const lines = lineBuffer.split('\n');
				lineBuffer = lines.pop() ?? '';

				for (const line of lines) {
					const trimmed = line.trim();
					if (!trimmed) continue;
					let chunk: OllamaChatResponse;
					try {
						chunk = JSON.parse(trimmed) as OllamaChatResponse;
					} catch {
						continue;
					}
					if (chunk.done) {
						promptTokens = chunk.prompt_eval_count ?? 0;
						completionTokens = chunk.eval_count ?? 0;
					}
					if (chunk.message?.content) {
						onChunk(chunk.message.content);
					}
					if (chunk.message?.tool_calls) {
						for (const tc of chunk.message.tool_calls) {
							toolCalls.push(tc);
						}
					}
				}
			}
		} finally {
			reader.releaseLock();
		}

		return { promptTokens, completionTokens, toolCalls };
	};

	const embed = async (
		model: string,
		texts: readonly string[],
	): Promise<{
		embeddings: readonly (readonly number[])[];
		promptTokens: number;
	}> => {
		const response = await post('/api/embed', { model, input: texts });
		const result = (await response.json()) as {
			embeddings: number[][];
			prompt_eval_count?: number;
		};
		return {
			embeddings: result.embeddings,
			promptTokens: result.prompt_eval_count ?? 0,
		};
	};

	return Object.freeze({ generate, generateStream, chat, chatStream, embed });
}

// ---------------------------------------------------------------------------
// NDJSON transport — reads stdin line-by-line, writes to stdout
// ---------------------------------------------------------------------------

interface NdjsonTransport {
	readonly writeNotification: (method: string, params: unknown) => void;
	readonly writeResponse: (id: number, result: unknown) => void;
	readonly writeError: (id: number, error: JsonRpcError) => void;
	readonly start: (onMessage: (msg: JsonRpcMessage) => void) => void;
}

function createNdjsonTransport(): NdjsonTransport {
	const writeLine = (msg: JsonRpcMessage): void => {
		process.stdout.write(`${JSON.stringify(msg)}\n`);
	};

	const writeNotification = (method: string, params: unknown): void => {
		const notification: JsonRpcNotification = {
			jsonrpc: '2.0',
			method,
			params,
		};
		writeLine(notification);
	};

	const writeResponse = (id: number, result: unknown): void => {
		const response: JsonRpcResponse = { jsonrpc: '2.0', id, result };
		writeLine(response);
	};

	const writeError = (id: number, error: JsonRpcError): void => {
		const response: JsonRpcResponse = { jsonrpc: '2.0', id, error };
		writeLine(response);
	};

	const start = (onMessage: (msg: JsonRpcMessage) => void): void => {
		const rl = createInterface({
			input: process.stdin,
			crlfDelay: Infinity,
			terminal: false,
		});

		rl.on('line', (line: string) => {
			const trimmed = line.trim();
			if (!trimmed) return;
			try {
				onMessage(JSON.parse(trimmed) as JsonRpcMessage);
			} catch {
				writeError(0, {
					code: JSON_RPC_ERRORS.PARSE_ERROR,
					message: 'Parse error: invalid JSON',
				});
			}
		});

		rl.on('close', () => {
			process.exit(0);
		});
	};

	return Object.freeze({
		writeNotification,
		writeResponse,
		writeError,
		start,
	});
}

// ---------------------------------------------------------------------------
// Content block helpers
// ---------------------------------------------------------------------------

function extractTextFromContent(content: readonly ACPContentBlock[]): {
	prompt: string;
	system: string | undefined;
} {
	const textBlocks = content
		.filter((b): b is { type: 'text'; text: string } => b.type === 'text')
		.map((b) => b.text);

	if (textBlocks.length === 0) return { prompt: '', system: undefined };
	if (textBlocks.length === 1)
		return { prompt: textBlocks[0], system: undefined };

	// Multi-block: first block(s) are system context, last is the prompt
	const system = textBlocks.slice(0, -1).join('\n');
	const prompt = textBlocks[textBlocks.length - 1];
	return { prompt, system };
}

function isEmbedRequest(content: readonly ACPContentBlock[]): boolean {
	for (const block of content) {
		if (block.type === 'data') {
			const data = block.data as Record<string, unknown> | null | undefined;
			if (data?.action === 'embed') return true;
		}
	}
	return false;
}

function extractEmbedTexts(content: readonly ACPContentBlock[]): string[] {
	for (const block of content) {
		if (block.type === 'data') {
			const data = block.data as Record<string, unknown> | null | undefined;
			if (Array.isArray(data?.texts)) return data.texts as string[];
		}
	}
	return [];
}

function extractEmbedModel(
	content: readonly ACPContentBlock[],
): string | undefined {
	for (const block of content) {
		if (block.type === 'data') {
			const data = block.data as Record<string, unknown> | null | undefined;
			if (typeof data?.model === 'string') return data.model;
		}
	}
	return undefined;
}

// ---------------------------------------------------------------------------
// Tool format conversion helpers
// ---------------------------------------------------------------------------

const TOOL_SECTION_START =
	'You have access to tools. To use a tool, include a JSON block';
const TOOL_LIST_HEADER = 'Available tools:';

function parseToolsFromSystemPrompt(system: string | undefined): {
	tools: readonly OllamaTool[];
	cleanedSystem: string | undefined;
} {
	if (!system) return { tools: [], cleanedSystem: undefined };

	const startIdx = system.indexOf(TOOL_SECTION_START);
	if (startIdx === -1) return { tools: [], cleanedSystem: system };

	const listIdx = system.indexOf(TOOL_LIST_HEADER, startIdx);
	if (listIdx === -1) return { tools: [], cleanedSystem: system };

	// Everything after "Available tools:\n\n" is the tool definitions
	const toolSection = system.slice(listIdx + TOOL_LIST_HEADER.length + 1);
	const cleaned = system.slice(0, startIdx).trim();

	// Parse individual tool entries: "- name: description\n  Parameters: ..."
	const tools: OllamaTool[] = [];
	const toolBlocks = toolSection.split('\n\n- ');

	for (let i = 0; i < toolBlocks.length; i++) {
		let block = toolBlocks[i].trim();
		if (i === 0 && block.startsWith('- ')) {
			block = block.slice(2);
		}
		if (!block) continue;

		const lines = block.split('\n');
		const headerLine = lines[0];
		const colonIdx = headerLine.indexOf(':');
		if (colonIdx === -1) continue;

		const name = headerLine.slice(0, colonIdx).trim();
		const description = headerLine.slice(colonIdx + 1).trim();

		const properties: Record<
			string,
			{ readonly type: string; readonly description: string }
		> = {};
		const required: string[] = [];

		// Parse "  Parameters: param1 (type, required), param2 (type)"
		const paramsLine = lines.find((l) => l.trim().startsWith('Parameters:'));
		if (paramsLine) {
			const paramsText = paramsLine
				.slice(paramsLine.indexOf('Parameters:') + 'Parameters:'.length)
				.trim();
			const paramEntries = paramsText.split('),');

			for (const entry of paramEntries) {
				const trimmed = entry.trim();
				if (!trimmed) continue;

				const parenIdx = trimmed.indexOf('(');
				if (parenIdx === -1) continue;

				const paramName = trimmed.slice(0, parenIdx).trim();
				const meta = trimmed
					.slice(parenIdx + 1)
					.replace(/\)$/, '')
					.trim();
				const parts = meta.split(',').map((s) => s.trim());
				const paramType = parts[0] || 'string';
				const isRequired = parts.includes('required');

				properties[paramName] = {
					type: paramType,
					description: paramName,
				};
				if (isRequired) required.push(paramName);
			}
		}

		tools.push({
			type: 'function',
			function: {
				name,
				description,
				parameters: {
					type: 'object',
					properties,
					required,
				},
			},
		});
	}

	return {
		tools,
		cleanedSystem: cleaned || undefined,
	};
}

function parseConversationMessages(prompt: string): OllamaChatMessage[] {
	if (!prompt.trim()) return [];

	// Split on message boundaries: "[Role]" or "[Tool Result: name]"
	const segments = prompt.split(
		/\n\n(?=\[(?:System|User|Assistant|Tool Result)[:\]])/,
	);
	const messages: OllamaChatMessage[] = [];

	for (const segment of segments) {
		const trimmed = segment.trim();
		if (!trimmed) continue;

		const newlineIdx = trimmed.indexOf('\n');
		if (newlineIdx === -1) continue;

		const header = trimmed.slice(0, newlineIdx).trim();
		const content = trimmed.slice(newlineIdx + 1).trim();

		if (header === '[System]') {
			messages.push({ role: 'system', content });
		} else if (header === '[User]') {
			messages.push({ role: 'user', content });
		} else if (header === '[Assistant]') {
			messages.push({ role: 'assistant', content });
		} else if (header.startsWith('[Tool Result:')) {
			messages.push({ role: 'tool', content });
		}
	}

	// If no recognized headers, treat entire prompt as a user message
	if (messages.length === 0 && prompt.trim()) {
		messages.push({ role: 'user', content: prompt.trim() });
	}

	return messages;
}

function formatToolCallsAsXml(toolCalls: readonly OllamaToolCall[]): string {
	return toolCalls
		.map((tc, i) => {
			const json = JSON.stringify({
				id: `call_${i + 1}`,
				name: tc.function.name,
				arguments: tc.function.arguments,
			});
			return `<tool_use>\n${json}\n</tool_use>`;
		})
		.join('\n\n');
}

// ---------------------------------------------------------------------------
// ACP server
// ---------------------------------------------------------------------------

function createAcpServer(
	opts: BridgeOptions,
	transport: NdjsonTransport,
): { readonly handleMessage: (msg: JsonRpcMessage) => void } {
	const ollama = createOllamaClient(opts.ollamaUrl);
	const sessions = new Set<string>();

	const isRequest = (msg: JsonRpcMessage): msg is JsonRpcRequest =>
		'id' in msg && 'method' in msg;

	// -- Method handlers -------------------------------------------------------

	const handleInitialize = (id: number): void => {
		const result: ACPInitializeResult = {
			protocolVersion: 1,
			agentInfo: {
				name: opts.serverName,
				version: opts.serverVersion,
			},
		};
		transport.writeResponse(id, result);
	};

	const handleSessionNew = (id: number): void => {
		const sessionId = randomUUID();
		sessions.add(sessionId);
		const result: ACPSessionNewResult = { sessionId };
		transport.writeResponse(id, result);
	};

	const handleSessionPrompt = async (
		id: number,
		params: unknown,
	): Promise<void> => {
		const p = params as ACPSessionPromptParams;

		if (!p?.sessionId || !sessions.has(p.sessionId)) {
			transport.writeError(id, {
				code: JSON_RPC_ERRORS.INVALID_PARAMS,
				message: `Session '${p?.sessionId ?? ''}' not found`,
			});
			return;
		}

		const { prompt: content, sessionId } = p;

		try {
			if (isEmbedRequest(content)) {
				await handleEmbed(id, content);
			} else if (opts.streaming) {
				await handleGenerateStream(id, sessionId, content);
			} else {
				await handleGenerateSync(id, content);
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			transport.writeError(id, {
				code: JSON_RPC_ERRORS.INTERNAL_ERROR,
				message: `Ollama error: ${message}`,
			});
		}
	};

	const handleGenerateSync = async (
		id: number,
		content: readonly ACPContentBlock[],
	): Promise<void> => {
		const { prompt, system } = extractTextFromContent(content);
		const { tools, cleanedSystem } = parseToolsFromSystemPrompt(system);

		if (tools.length > 0) {
			const messages = parseConversationMessages(prompt);
			if (cleanedSystem) {
				messages.unshift({ role: 'system', content: cleanedSystem });
			}

			const response = await ollama.chat(opts.defaultModel, messages, tools);

			let text = response.message.content ?? '';
			let stopReason: ACPStopReason = 'end_turn';

			if (response.message.tool_calls?.length) {
				const xml = formatToolCallsAsXml(response.message.tool_calls);
				text = text ? `${text}\n\n${xml}` : xml;
				stopReason = 'tool_use';
			}

			const promptTokens = response.prompt_eval_count ?? 0;
			const completionTokens = response.eval_count ?? 0;

			const result: ACPSessionPromptResult = {
				content: [{ type: 'text', text }],
				stopReason,
				metadata: {
					usage: {
						prompt_tokens: promptTokens,
						completion_tokens: completionTokens,
						total_tokens: promptTokens + completionTokens,
					},
				},
			};
			transport.writeResponse(id, result);
		} else {
			const chunk = await ollama.generate(opts.defaultModel, prompt, system);

			const text = chunk.response ?? '';
			const promptTokens = chunk.prompt_eval_count ?? 0;
			const completionTokens = chunk.eval_count ?? 0;

			const result: ACPSessionPromptResult = {
				content: [{ type: 'text', text }],
				stopReason: 'end_turn',
				metadata: {
					usage: {
						prompt_tokens: promptTokens,
						completion_tokens: completionTokens,
						total_tokens: promptTokens + completionTokens,
					},
				},
			};
			transport.writeResponse(id, result);
		}
	};

	const handleGenerateStream = async (
		id: number,
		sessionId: string,
		content: readonly ACPContentBlock[],
	): Promise<void> => {
		const { prompt, system } = extractTextFromContent(content);
		const { tools, cleanedSystem } = parseToolsFromSystemPrompt(system);
		let fullText = '';

		if (tools.length > 0) {
			const messages = parseConversationMessages(prompt);
			if (cleanedSystem) {
				messages.unshift({ role: 'system', content: cleanedSystem });
			}

			const { promptTokens, completionTokens, toolCalls } =
				await ollama.chatStream(opts.defaultModel, messages, tools, (text) => {
					fullText += text;
					const updateParams: ACPSessionUpdateParams = {
						sessionId,
						update: {
							sessionUpdate: 'agent_message_chunk',
							content: [{ type: 'text', text }],
						},
					};
					transport.writeNotification('session/update', updateParams);
				});

			let stopReason: ACPStopReason = 'end_turn';

			if (toolCalls.length > 0) {
				const xml = formatToolCallsAsXml(toolCalls);
				fullText = fullText ? `${fullText}\n\n${xml}` : xml;
				stopReason = 'tool_use';

				// Send the tool call XML as a final chunk
				const updateParams: ACPSessionUpdateParams = {
					sessionId,
					update: {
						sessionUpdate: 'agent_message_chunk',
						content: [{ type: 'text', text: `\n\n${xml}` }],
					},
				};
				transport.writeNotification('session/update', updateParams);
			}

			const result: ACPSessionPromptResult = {
				content: [{ type: 'text', text: fullText }],
				stopReason,
				metadata: {
					usage: {
						prompt_tokens: promptTokens,
						completion_tokens: completionTokens,
						total_tokens: promptTokens + completionTokens,
					},
				},
			};
			transport.writeResponse(id, result);
		} else {
			const { promptTokens, completionTokens } = await ollama.generateStream(
				opts.defaultModel,
				prompt,
				system,
				(text) => {
					fullText += text;
					const updateParams: ACPSessionUpdateParams = {
						sessionId,
						update: {
							sessionUpdate: 'agent_message_chunk',
							content: [{ type: 'text', text }],
						},
					};
					transport.writeNotification('session/update', updateParams);
				},
			);

			const result: ACPSessionPromptResult = {
				content: [{ type: 'text', text: fullText }],
				stopReason: 'end_turn',
				metadata: {
					usage: {
						prompt_tokens: promptTokens,
						completion_tokens: completionTokens,
						total_tokens: promptTokens + completionTokens,
					},
				},
			};
			transport.writeResponse(id, result);
		}
	};

	const handleEmbed = async (
		id: number,
		content: readonly ACPContentBlock[],
	): Promise<void> => {
		const texts = extractEmbedTexts(content);
		const model = extractEmbedModel(content) ?? opts.embeddingModel;

		if (texts.length === 0) {
			transport.writeError(id, {
				code: JSON_RPC_ERRORS.INVALID_PARAMS,
				message: 'Embed request contained no texts',
			});
			return;
		}

		const response = await ollama.embed(model, texts);

		const result: ACPSessionPromptResult = {
			content: [
				{
					type: 'data',
					data: { embeddings: response.embeddings },
					mimeType: 'application/json',
				},
			],
			stopReason: 'end_turn',
			metadata: {
				usage: {
					prompt_tokens: response.promptTokens,
					completion_tokens: 0,
					total_tokens: response.promptTokens,
				},
			},
		};
		transport.writeResponse(id, result);
	};

	// -- Dispatch --------------------------------------------------------------

	const handleMessage = (msg: JsonRpcMessage): void => {
		if (!isRequest(msg)) return;

		const { id, method, params } = msg;

		switch (method) {
			case 'initialize':
				handleInitialize(id);
				break;
			case 'session/new':
				handleSessionNew(id);
				break;
			case 'session/prompt':
				handleSessionPrompt(id, params).catch((err) => {
					const message = err instanceof Error ? err.message : String(err);
					transport.writeError(id, {
						code: JSON_RPC_ERRORS.INTERNAL_ERROR,
						message: `Unhandled error: ${message}`,
					});
				});
				break;
			default:
				transport.writeError(id, {
					code: JSON_RPC_ERRORS.METHOD_NOT_FOUND,
					message: `Method not found: ${method}`,
				});
		}
	};

	return Object.freeze({ handleMessage });
}

// ---------------------------------------------------------------------------
// Bridge factory
// ---------------------------------------------------------------------------

export interface Bridge {
	readonly start: () => void;
}

export function createBridge(options: BridgeOptions): Bridge {
	const transport = createNdjsonTransport();
	const server = createAcpServer(options, transport);

	const start = (): void => {
		transport.start((msg) => server.handleMessage(msg));
	};

	return Object.freeze({ start });
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

function parseArgs(): BridgeOptions {
	const args = process.argv.slice(2);
	let ollamaUrl = 'http://10.0.0.103:11434';
	let defaultModel = 'gpt-oss:20b';
	let embeddingModel = 'nomic-embed-text';
	let serverName = 'acp-ollama-bridge';
	let serverVersion = '2.0.0';
	let streaming = true;

	for (let i = 0; i < args.length; i++) {
		if (args[i] === '--ollama' && args[i + 1]) ollamaUrl = args[++i];
		if (args[i] === '--model' && args[i + 1]) defaultModel = args[++i];
		if (args[i] === '--embedding-model' && args[i + 1])
			embeddingModel = args[++i];
		if (args[i] === '--server-name' && args[i + 1]) serverName = args[++i];
		if (args[i] === '--server-version' && args[i + 1])
			serverVersion = args[++i];
		if (args[i] === '--no-streaming') streaming = false;
	}

	return Object.freeze({
		ollamaUrl,
		defaultModel,
		embeddingModel,
		serverName,
		serverVersion,
		streaming,
	});
}

if (import.meta.main) {
	createBridge(parseArgs()).start();
}
