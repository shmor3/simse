// ---------------------------------------------------------------------------
// ACP Engine Client — JSON-RPC 2.0 over NDJSON stdio transport to Rust subprocess
// ---------------------------------------------------------------------------

import { type ChildProcess, spawn } from 'node:child_process';
import {
	createProviderError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	toError,
} from '../../errors/index.js';
import type { Logger } from '../shared/logger.js';
import { createNoopLogger } from '../shared/logger.js';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface AcpEngineClientOptions {
	readonly enginePath: string;
	readonly timeoutMs?: number;
	readonly logger?: Logger;
}

export interface AcpEngineClient {
	readonly request: <T>(method: string, params?: unknown) => Promise<T>;
	readonly onNotification: (
		method: string,
		handler: (params: unknown) => void,
	) => () => void;
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

interface JsonRpcResponseMsg {
	readonly jsonrpc: '2.0';
	readonly id: number;
	readonly result?: unknown;
	readonly error?: {
		readonly code: number;
		readonly message: string;
		readonly data?: unknown;
	};
}

interface JsonRpcNotificationMsg {
	readonly jsonrpc: '2.0';
	readonly method: string;
	readonly params?: unknown;
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
// Message type guards
// ---------------------------------------------------------------------------

const isResponse = (msg: unknown): msg is JsonRpcResponseMsg =>
	typeof msg === 'object' &&
	msg !== null &&
	'id' in msg &&
	typeof (msg as JsonRpcResponseMsg).id === 'number' &&
	!('method' in msg);

const isNotification = (msg: unknown): msg is JsonRpcNotificationMsg =>
	typeof msg === 'object' &&
	msg !== null &&
	'method' in msg &&
	typeof (msg as JsonRpcNotificationMsg).method === 'string' &&
	!('id' in msg);

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createAcpEngineClient(
	options: AcpEngineClientOptions,
): AcpEngineClient {
	const timeoutMs = options.timeoutMs ?? 60_000;
	const logger = options.logger ?? createNoopLogger();
	const log = logger.child('acp-engine-client');

	let nextId = 1;
	let child: ChildProcess | undefined;
	let buffer = '';
	let disposed = false;

	const pending = new Map<number, PendingRequest>();
	const notificationHandlers = new Map<
		string,
		Set<(params: unknown) => void>
	>();

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
			const resp = msg as JsonRpcResponseMsg;
			const req = pending.get(resp.id);
			if (!req) return;

			pending.delete(resp.id);
			clearTimeout(req.timer);

			if (resp.error) {
				req.reject(
					createProviderError(
						'acp',
						`ACP engine error ${resp.error.code}: ${resp.error.message}`,
						{
							code: 'PROVIDER_GENERATION_ERROR',
							metadata: { jsonrpcError: resp.error },
						},
					),
				);
			} else {
				req.resolve(resp.result);
			}
		} else if (isNotification(msg)) {
			const notif = msg as JsonRpcNotificationMsg;
			const handlers = notificationHandlers.get(notif.method);
			if (handlers) {
				for (const handler of handlers) {
					try {
						handler(notif.params);
					} catch {
						// Swallow handler errors
					}
				}
			}
		}
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
				createProviderUnavailableError('acp', {
					metadata: {
						reason: 'ACP engine client is not connected',
					},
				}),
			);
		}

		const id = nextId++;

		return new Promise<T>((resolve, reject) => {
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(
					createProviderTimeoutError('acp', timeoutMs, {
						cause: new Error(
							`ACP engine request "${method}" timed out after ${timeoutMs}ms`,
						),
					}),
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
	// Notification subscription
	// -----------------------------------------------------------------------

	const onNotification = (
		method: string,
		handler: (params: unknown) => void,
	): (() => void) => {
		let handlers = notificationHandlers.get(method);
		if (!handlers) {
			handlers = new Set();
			notificationHandlers.set(method, handlers);
		}
		handlers.add(handler);

		return () => {
			handlers?.delete(handler);
			if (handlers?.size === 0) {
				notificationHandlers.delete(method);
			}
		};
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
			createProviderUnavailableError('acp', {
				metadata: { reason: 'ACP engine client disposed' },
			}),
		);

		notificationHandlers.clear();

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
			log.debug('acp-engine stderr', { text });
		}
	});

	child.on('error', (err) => {
		log.error('acp-engine process error', toError(err));
		rejectAll(
			createProviderUnavailableError('acp', {
				cause: err,
				metadata: {
					reason: `ACP engine process error: ${toError(err).message}`,
				},
			}),
		);
	});

	child.on('exit', (code) => {
		if (!disposed) {
			log.warn('acp-engine exited unexpectedly', { exitCode: code });
			rejectAll(
				createProviderUnavailableError('acp', {
					metadata: {
						reason: `ACP engine exited with code ${code}`,
						exitCode: code,
					},
				}),
			);
		}
	});

	log.info('ACP engine process spawned');

	// -----------------------------------------------------------------------
	// Return frozen interface
	// -----------------------------------------------------------------------

	return Object.freeze({
		request: sendRequest,
		onNotification,
		dispose,
		get isHealthy() {
			return !disposed && !!child && !child.killed && child.exitCode === null;
		},
	});
}
