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

	return Object.freeze({ generate, generateStream, embed });
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
		const result: ACPSessionNewResult = { session_id: sessionId };
		transport.writeResponse(id, result);
	};

	const handleSessionPrompt = async (
		id: number,
		params: unknown,
	): Promise<void> => {
		const p = params as ACPSessionPromptParams;

		if (!p?.session_id || !sessions.has(p.session_id)) {
			transport.writeError(id, {
				code: JSON_RPC_ERRORS.INVALID_PARAMS,
				message: `Session '${p?.session_id ?? ''}' not found`,
			});
			return;
		}

		const { content, session_id: sessionId } = p;

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
		const chunk = await ollama.generate(opts.defaultModel, prompt, system);

		const text = chunk.response ?? '';
		const promptTokens = chunk.prompt_eval_count ?? 0;
		const completionTokens = chunk.eval_count ?? 0;

		const result: ACPSessionPromptResult = {
			content: [{ type: 'text', text }],
			stop_reason: 'end_turn',
			metadata: {
				usage: {
					prompt_tokens: promptTokens,
					completion_tokens: completionTokens,
					total_tokens: promptTokens + completionTokens,
				},
			},
		};
		transport.writeResponse(id, result);
	};

	const handleGenerateStream = async (
		id: number,
		sessionId: string,
		content: readonly ACPContentBlock[],
	): Promise<void> => {
		const { prompt, system } = extractTextFromContent(content);
		let fullText = '';

		const { promptTokens, completionTokens } = await ollama.generateStream(
			opts.defaultModel,
			prompt,
			system,
			(text) => {
				fullText += text;
				const updateParams: ACPSessionUpdateParams = {
					session_id: sessionId,
					kind: 'agent_message_chunk',
					content: [{ type: 'text', text }],
				};
				transport.writeNotification('session/update', updateParams);
			},
		);

		const result: ACPSessionPromptResult = {
			content: [{ type: 'text', text: fullText }],
			stop_reason: 'end_turn',
			metadata: {
				usage: {
					prompt_tokens: promptTokens,
					completion_tokens: completionTokens,
					total_tokens: promptTokens + completionTokens,
				},
			},
		};
		transport.writeResponse(id, result);
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
			stop_reason: 'end_turn',
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
