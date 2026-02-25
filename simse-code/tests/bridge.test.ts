import { describe, expect, it } from 'bun:test';
import type {
	ACPInitializeResult,
	ACPSessionNewResult,
	JsonRpcRequest,
	JsonRpcResponse,
} from '../../src/ai/acp/types.js';

// ---------------------------------------------------------------------------
// We test the bridge by spawning it as a subprocess and speaking JSON-RPC
// over its stdin/stdout — exactly as ACPConnection does.
// ---------------------------------------------------------------------------

const BRIDGE_PATH = new URL('../acp-ollama-bridge.ts', import.meta.url)
	.pathname;

interface BridgeProcess {
	readonly send: (msg: JsonRpcRequest) => void;
	readonly receive: () => Promise<JsonRpcResponse>;
	readonly close: () => void;
}

function spawnBridge(): BridgeProcess {
	const proc = Bun.spawn(['bun', 'run', BRIDGE_PATH, '--no-streaming'], {
		stdin: 'pipe',
		stdout: 'pipe',
		stderr: 'pipe',
	});

	let buffer = '';
	const lines: string[] = [];
	let resolveNext: ((line: string) => void) | undefined;

	// Read stdout as text stream
	const reader = proc.stdout.getReader();
	const decoder = new TextDecoder();

	const readLoop = async () => {
		try {
			while (true) {
				const { done, value } = await reader.read();
				if (done) break;
				buffer += decoder.decode(value, { stream: true });
				const parts = buffer.split('\n');
				buffer = parts.pop() ?? '';
				for (const part of parts) {
					const trimmed = part.trim();
					if (!trimmed) continue;
					if (resolveNext) {
						const r = resolveNext;
						resolveNext = undefined;
						r(trimmed);
					} else {
						lines.push(trimmed);
					}
				}
			}
		} catch {
			// Process ended
		}
	};
	readLoop();

	const send = (msg: JsonRpcRequest): void => {
		proc.stdin.write(`${JSON.stringify(msg)}\n`);
	};

	const receive = (): Promise<JsonRpcResponse> => {
		if (lines.length > 0) {
			return Promise.resolve(JSON.parse(lines.shift()!) as JsonRpcResponse);
		}
		return new Promise<JsonRpcResponse>((resolve) => {
			resolveNext = (line) => resolve(JSON.parse(line) as JsonRpcResponse);
		});
	};

	const close = (): void => {
		proc.stdin.end();
		proc.kill();
	};

	return { send, receive, close };
}

// ---------------------------------------------------------------------------
// Tests — these don't need a running Ollama instance since they test the
// JSON-RPC layer, not the Ollama proxy. Only initialize and session/new
// are fully testable without Ollama.
// ---------------------------------------------------------------------------

describe('acp-ollama-bridge (JSON-RPC)', () => {
	it('should respond to initialize', async () => {
		const bridge = spawnBridge();
		try {
			bridge.send({
				jsonrpc: '2.0',
				id: 1,
				method: 'initialize',
				params: {
					client_info: { name: 'test', version: '1.0.0' },
				},
			});

			const response = await bridge.receive();
			expect(response.jsonrpc).toBe('2.0');
			expect(response.id).toBe(1);
			expect(response.error).toBeUndefined();

			const result = response.result as ACPInitializeResult;
			expect(result.server_info.name).toBe('acp-ollama-bridge');
			expect(result.server_info.version).toBe('2.0.0');
		} finally {
			bridge.close();
		}
	});

	it('should create a session with session/new', async () => {
		const bridge = spawnBridge();
		try {
			// Initialize first
			bridge.send({
				jsonrpc: '2.0',
				id: 1,
				method: 'initialize',
				params: {
					client_info: { name: 'test', version: '1.0.0' },
				},
			});
			await bridge.receive();

			// Create session
			bridge.send({
				jsonrpc: '2.0',
				id: 2,
				method: 'session/new',
				params: {},
			});

			const response = await bridge.receive();
			expect(response.id).toBe(2);
			expect(response.error).toBeUndefined();

			const result = response.result as ACPSessionNewResult;
			expect(result.session_id).toBeDefined();
			expect(typeof result.session_id).toBe('string');
			expect(result.session_id.length).toBeGreaterThan(0);
		} finally {
			bridge.close();
		}
	});

	it('should reject unknown methods with METHOD_NOT_FOUND', async () => {
		const bridge = spawnBridge();
		try {
			bridge.send({
				jsonrpc: '2.0',
				id: 1,
				method: 'nonexistent/method',
			});

			const response = await bridge.receive();
			expect(response.id).toBe(1);
			expect(response.error).toBeDefined();
			expect(response.error?.code).toBe(-32601);
		} finally {
			bridge.close();
		}
	});

	it('should reject session/prompt with invalid session_id', async () => {
		const bridge = spawnBridge();
		try {
			// Initialize
			bridge.send({
				jsonrpc: '2.0',
				id: 1,
				method: 'initialize',
				params: {
					client_info: { name: 'test', version: '1.0.0' },
				},
			});
			await bridge.receive();

			// Prompt with bad session
			bridge.send({
				jsonrpc: '2.0',
				id: 2,
				method: 'session/prompt',
				params: {
					session_id: 'does-not-exist',
					content: [{ type: 'text', text: 'hello' }],
				},
			});

			const response = await bridge.receive();
			expect(response.id).toBe(2);
			expect(response.error).toBeDefined();
			expect(response.error?.code).toBe(-32602); // INVALID_PARAMS
			expect(response.error?.message).toContain('does-not-exist');
		} finally {
			bridge.close();
		}
	});

	it('should handle multiple sequential requests', async () => {
		const bridge = spawnBridge();
		try {
			// Initialize
			bridge.send({
				jsonrpc: '2.0',
				id: 1,
				method: 'initialize',
				params: {
					client_info: { name: 'test', version: '1.0.0' },
				},
			});
			const initResp = await bridge.receive();
			expect(initResp.id).toBe(1);

			// Two session/new in sequence
			bridge.send({
				jsonrpc: '2.0',
				id: 2,
				method: 'session/new',
				params: {},
			});
			const sess1 = await bridge.receive();
			expect(sess1.id).toBe(2);

			bridge.send({
				jsonrpc: '2.0',
				id: 3,
				method: 'session/new',
				params: {},
			});
			const sess2 = await bridge.receive();
			expect(sess2.id).toBe(3);

			// Different session IDs
			const id1 = (sess1.result as ACPSessionNewResult).session_id;
			const id2 = (sess2.result as ACPSessionNewResult).session_id;
			expect(id1).not.toBe(id2);
		} finally {
			bridge.close();
		}
	});
});
