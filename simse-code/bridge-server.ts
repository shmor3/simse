// ---------------------------------------------------------------------------
// Bridge Server — JSON-RPC 2.0 over NDJSON stdio
// ---------------------------------------------------------------------------
//
// A TypeScript bridge that wraps simse core APIs for consumption by the
// Rust TUI. Communicates over stdin/stdout using NDJSON (newline-delimited
// JSON). Currently returns stub responses — real implementations will be
// wired in Phase 8.
//
// Usage:
//   bun run bridge-server.ts
//
// ---------------------------------------------------------------------------

import { createInterface } from 'node:readline';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface NdjsonTransport {
	readonly writeResponse: (id: number, result: unknown) => void;
	readonly writeError: (
		id: number,
		code: number,
		message: string,
		data?: unknown,
	) => void;
	readonly writeNotification: (method: string, params?: unknown) => void;
}

export interface JsonRpcMessage {
	jsonrpc: string;
	id?: number;
	method?: string;
	params?: unknown;
}

// ---------------------------------------------------------------------------
// JSON-RPC error codes
// ---------------------------------------------------------------------------

const METHOD_NOT_FOUND = -32601;
const INTERNAL_ERROR = -32603;

// ---------------------------------------------------------------------------
// NDJSON transport
// ---------------------------------------------------------------------------

export function createNdjsonTransport(
	writeLine: (line: string) => void = (line) =>
		process.stdout.write(`${line}\n`),
): NdjsonTransport {
	const writeResponse = (id: number, result: unknown): void => {
		writeLine(JSON.stringify({ jsonrpc: '2.0', id, result }));
	};

	const writeError = (
		id: number,
		code: number,
		message: string,
		data?: unknown,
	): void => {
		const error: Record<string, unknown> = { code, message };
		if (data !== undefined) {
			error.data = data;
		}
		writeLine(JSON.stringify({ jsonrpc: '2.0', id, error }));
	};

	const writeNotification = (method: string, params?: unknown): void => {
		const msg: Record<string, unknown> = { jsonrpc: '2.0', method };
		if (params !== undefined) {
			msg.params = params;
		}
		writeLine(JSON.stringify(msg));
	};

	return Object.freeze({ writeResponse, writeError, writeNotification });
}

// ---------------------------------------------------------------------------
// Method handlers (stubs)
// ---------------------------------------------------------------------------

type MethodHandler = (
	params: unknown,
) => unknown | Promise<unknown>;

function createMethodHandlers(): ReadonlyMap<string, MethodHandler> {
	const handlers = new Map<string, MethodHandler>();

	handlers.set('initialize', () => ({
		protocolVersion: 1,
		name: 'simse-bridge',
	}));

	handlers.set('generate', (params) => {
		const p = params as Record<string, unknown> | undefined;
		return {
			content: [
				{
					type: 'text',
					text: `[stub] generate response for: ${p?.prompt ?? '(no prompt)'}`,
				},
			],
			stopReason: 'end_turn',
		};
	});

	handlers.set('generateStream', (params) => {
		const p = params as Record<string, unknown> | undefined;
		return {
			content: [
				{
					type: 'text',
					text: `[stub] generateStream response for: ${p?.prompt ?? '(no prompt)'}`,
				},
			],
			stopReason: 'end_turn',
		};
	});

	handlers.set('library.search', () => ({
		results: [],
	}));

	handlers.set('library.add', () => ({
		id: 'stub-id',
		success: true,
	}));

	handlers.set('library.recommend', () => ({
		recommendations: [],
	}));

	handlers.set('tools.list', () => ({
		tools: [],
	}));

	handlers.set('tools.execute', (params) => {
		const p = params as Record<string, unknown> | undefined;
		return {
			output: `[stub] executed tool: ${p?.name ?? '(unknown)'}`,
			success: true,
		};
	});

	handlers.set('session.load', (params) => {
		const p = params as Record<string, unknown> | undefined;
		return {
			sessionId: p?.sessionId ?? 'stub-session',
			loaded: true,
		};
	});

	handlers.set('session.save', () => ({
		success: true,
	}));

	handlers.set('config.read', () => ({
		config: {},
	}));

	handlers.set('config.write', () => ({
		success: true,
	}));

	return handlers;
}

// ---------------------------------------------------------------------------
// Bridge handler
// ---------------------------------------------------------------------------

export function createBridgeHandler(
	transport: NdjsonTransport,
): (msg: JsonRpcMessage) => void {
	const handlers = createMethodHandlers();

	const handleMessage = (msg: JsonRpcMessage): void => {
		const { id, method, params } = msg;

		// Notifications (no id) are silently ignored for now
		if (id === undefined || method === undefined) return;

		const handler = handlers.get(method);
		if (!handler) {
			transport.writeError(
				id,
				METHOD_NOT_FOUND,
				`Method not found: ${method}`,
			);
			return;
		}

		try {
			const result = handler(params);
			if (result instanceof Promise) {
				result.then(
					(resolved) => transport.writeResponse(id, resolved),
					(err) => {
						const message =
							err instanceof Error ? err.message : String(err);
						transport.writeError(
							id,
							INTERNAL_ERROR,
							`Internal error: ${message}`,
						);
					},
				);
			} else {
				transport.writeResponse(id, result);
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			transport.writeError(
				id,
				INTERNAL_ERROR,
				`Internal error: ${message}`,
			);
		}
	};

	return handleMessage;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

export function startBridge(): void {
	const transport = createNdjsonTransport();
	const handler = createBridgeHandler(transport);

	const rl = createInterface({
		input: process.stdin,
		crlfDelay: Infinity,
		terminal: false,
	});

	rl.on('line', (line: string) => {
		const trimmed = line.trim();
		if (!trimmed) return;
		try {
			const msg = JSON.parse(trimmed) as JsonRpcMessage;
			handler(msg);
		} catch {
			transport.writeError(0, -32700, 'Parse error: invalid JSON');
		}
	});

	rl.on('close', () => {
		process.exit(0);
	});
}

if (import.meta.main) {
	startBridge();
}
