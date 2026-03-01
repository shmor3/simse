// ---------------------------------------------------------------------------
// Vector Client — JSON-RPC 2.0 over NDJSON stdio transport to Rust subprocess
// ---------------------------------------------------------------------------

import { type ChildProcess, spawn } from 'node:child_process';
import type { Logger } from '../shared/logger.js';
import { createNoopLogger } from '../shared/logger.js';
import { createStacksError, toError } from './errors.js';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface VectorClientOptions {
	readonly enginePath: string;
	readonly timeoutMs?: number;
	readonly logger?: Logger;
}

export interface VectorClient {
	readonly request: <T>(method: string, params?: unknown) => Promise<T>;
	readonly dispose: () => Promise<void>;
	readonly isHealthy: boolean;
}

// ---------------------------------------------------------------------------
// JSON-RPC message shapes (internal)
// ---------------------------------------------------------------------------

interface JsonRpcRequest {
	readonly jsonrpc: '2.0';
	readonly id: number;
	readonly method: string;
	readonly params?: unknown;
}

interface JsonRpcResponse {
	readonly jsonrpc: '2.0';
	readonly id: number;
	readonly result?: unknown;
	readonly error?: {
		readonly code: number;
		readonly message: string;
		readonly data?: {
			readonly vectorCode?: string;
			readonly message?: string;
		};
	};
}

// ---------------------------------------------------------------------------
// Pending request tracking
// ---------------------------------------------------------------------------

interface PendingRequest {
	readonly resolve: (value: unknown) => void;
	readonly reject: (reason: unknown) => void;
	readonly timer: ReturnType<typeof setTimeout>;
	readonly method: string;
}

// ---------------------------------------------------------------------------
// Message type guard
// ---------------------------------------------------------------------------

const isResponse = (msg: unknown): msg is JsonRpcResponse =>
	typeof msg === 'object' &&
	msg !== null &&
	'id' in msg &&
	typeof (msg as JsonRpcResponse).id === 'number';

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createVectorClient(options: VectorClientOptions): VectorClient {
	const timeoutMs = options.timeoutMs ?? 60_000;
	const logger = options.logger ?? createNoopLogger();
	const log = logger.child('vector-client');

	let nextId = 1;
	let child: ChildProcess | undefined;
	let buffer = '';
	let disposed = false;

	const pending = new Map<number, PendingRequest>();

	// -----------------------------------------------------------------------
	// Incoming data processing
	// -----------------------------------------------------------------------

	const processLine = (line: string): void => {
		const trimmed = line.trim();
		if (trimmed.length === 0) return;

		let msg: unknown;
		try {
			msg = JSON.parse(trimmed);
			if (typeof msg !== 'object' || msg === null) return;
		} catch {
			log.debug('Skipping malformed JSON line', { line: trimmed });
			return;
		}

		if (isResponse(msg)) {
			const resp = msg as JsonRpcResponse;
			const req = pending.get(resp.id);
			if (!req) return;

			pending.delete(resp.id);
			clearTimeout(req.timer);

			if (resp.error) {
				const vectorCode = resp.error.data?.vectorCode ?? 'STACKS_ERROR';
				req.reject(
					createStacksError(resp.error.message, {
						code: vectorCode,
					}),
				);
			} else {
				req.resolve(resp.result);
			}
		}
		// Notifications are ignored (vector engine doesn't emit them)
	};

	const onData = (data: Buffer): void => {
		buffer += data.toString();
		const lines = buffer.split('\n');
		buffer = lines.pop() ?? '';
		for (const line of lines) {
			processLine(line);
		}
	};

	// -----------------------------------------------------------------------
	// Send request
	// -----------------------------------------------------------------------

	const sendRequest = <T>(method: string, params?: unknown): Promise<T> => {
		if (disposed || !child?.stdin?.writable) {
			return Promise.reject(
				createStacksError('Vector client is not connected', {
					code: 'STACKS_ERROR',
					metadata: { reason: 'Client disposed or stdin not writable' },
				}),
			);
		}

		const id = nextId++;

		return new Promise<T>((resolve, reject) => {
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(
					createStacksError(
						`Vector request "${method}" timed out after ${timeoutMs}ms`,
						{
							code: 'STACKS_ERROR',
							metadata: { method, timeoutMs },
						},
					),
				);
			}, timeoutMs);

			pending.set(id, {
				resolve: resolve as (value: unknown) => void,
				reject,
				timer,
				method,
			});

			const request: JsonRpcRequest = {
				jsonrpc: '2.0',
				id,
				method,
				params,
			};

			child?.stdin?.write(`${JSON.stringify(request)}\n`);
		});
	};

	// -----------------------------------------------------------------------
	// Cleanup
	// -----------------------------------------------------------------------

	const rejectAll = (error: unknown): void => {
		for (const [, req] of pending) {
			clearTimeout(req.timer);
			req.reject(error);
		}
		pending.clear();
	};

	const dispose = async (): Promise<void> => {
		if (disposed) return;
		disposed = true;

		rejectAll(
			createStacksError('Vector client disposed', {
				code: 'STACKS_ERROR',
				metadata: { reason: 'Client disposed' },
			}),
		);

		if (child) {
			child.stdin?.end();
			child.kill('SIGTERM');
			child = undefined;
		}
	};

	// -----------------------------------------------------------------------
	// Spawn process
	// -----------------------------------------------------------------------

	child = spawn(options.enginePath, [], {
		stdio: ['pipe', 'pipe', 'pipe'],
		detached: false,
	});

	child.stdout?.on('data', onData);

	child.stderr?.on('data', (data: Buffer) => {
		const text = data.toString().trim();
		if (text) {
			log.debug('vector-engine stderr', { text });
		}
	});

	child.on('error', (err) => {
		log.error('vector-engine process error', toError(err));
		rejectAll(
			createStacksError(
				`Vector engine process error: ${toError(err).message}`,
				{
					code: 'STACKS_ERROR',
					cause: err,
				},
			),
		);
	});

	child.on('exit', (code) => {
		if (!disposed) {
			log.warn('vector-engine exited unexpectedly', { exitCode: code });
			rejectAll(
				createStacksError(`Vector engine exited with code ${code}`, {
					code: 'STACKS_ERROR',
					metadata: { exitCode: code },
				}),
			);
		}
	});

	log.info('Vector engine process spawned');

	// -----------------------------------------------------------------------
	// Return frozen interface
	// -----------------------------------------------------------------------

	return Object.freeze({
		request: sendRequest,
		dispose,
		get isHealthy() {
			return !disposed && !!child && !child.killed && child.exitCode === null;
		},
	});
}
