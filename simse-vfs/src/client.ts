// ---------------------------------------------------------------------------
// VFS Client — JSON-RPC 2.0 over NDJSON stdio transport to Rust subprocess
// ---------------------------------------------------------------------------

import { type ChildProcess, spawn } from 'node:child_process';
import { createVFSError, toError } from './errors.js';
import type { Logger } from './logger.js';
import { createNoopLogger } from './logger.js';
import type { VFSHistoryOptions, VFSLimits } from './types.js';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface VFSClientOptions {
	readonly enginePath: string;
	readonly limits?: VFSLimits;
	readonly history?: VFSHistoryOptions;
	readonly timeoutMs?: number;
	readonly onEvent?: (event: VFSClientEvent) => void;
	readonly logger?: Logger;
}

export interface VFSClientEvent {
	readonly type: 'write' | 'delete' | 'rename' | 'mkdir';
	readonly path: string;
	readonly size?: number;
	readonly contentType?: string;
	readonly isNew?: boolean;
	readonly oldPath?: string;
	readonly newPath?: string;
}

export interface VFSClient {
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
			readonly vfsCode?: string;
			readonly metadata?: Readonly<Record<string, unknown>>;
		};
	};
}

interface JsonRpcNotification {
	readonly jsonrpc: '2.0';
	readonly method: string;
	readonly params?: unknown;
}

type JsonRpcMessage = JsonRpcResponse | JsonRpcNotification;

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

const isResponse = (msg: JsonRpcMessage): msg is JsonRpcResponse =>
	'id' in msg && typeof (msg as JsonRpcResponse).id === 'number';

const isNotification = (msg: JsonRpcMessage): msg is JsonRpcNotification =>
	!('id' in msg) && 'method' in msg;

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export async function createVFSClient(
	options: VFSClientOptions,
): Promise<VFSClient> {
	const timeoutMs = options.timeoutMs ?? 30_000;
	const logger = options.logger ?? createNoopLogger();
	const log = logger.child('vfs-client');
	const onEvent = options.onEvent;

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

		let msg: JsonRpcMessage;
		try {
			const parsed: unknown = JSON.parse(trimmed);
			if (typeof parsed !== 'object' || parsed === null) return;
			msg = parsed as JsonRpcMessage;
		} catch {
			log.debug('Skipping malformed JSON line', { line: trimmed });
			return;
		}

		if (isResponse(msg)) {
			const req = pending.get(msg.id);
			if (!req) return;

			pending.delete(msg.id);
			clearTimeout(req.timer);

			if (msg.error) {
				const vfsCode = msg.error.data?.vfsCode ?? 'VFS_ERROR';
				const metadata = msg.error.data?.metadata ?? {};
				req.reject(
					createVFSError(msg.error.message, {
						code: vfsCode,
						metadata,
					}),
				);
			} else {
				req.resolve(msg.result);
			}
		} else if (isNotification(msg)) {
			if (msg.method === 'vfs/event' && onEvent) {
				try {
					onEvent(msg.params as VFSClientEvent);
				} catch {
					// Swallow handler errors
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
				createVFSError('VFS client is not connected', {
					code: 'VFS_ERROR',
					metadata: { reason: 'Client disposed or stdin not writable' },
				}),
			);
		}

		const id = nextId++;

		return new Promise<T>((resolve, reject) => {
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(
					createVFSError(
						`VFS request "${method}" timed out after ${timeoutMs}ms`,
						{
							code: 'VFS_ERROR',
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
			createVFSError('VFS client disposed', {
				code: 'VFS_ERROR',
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
	// Spawn + initialize
	// -----------------------------------------------------------------------

	child = spawn(options.enginePath, [], {
		stdio: ['pipe', 'pipe', 'pipe'],
		detached: false,
	});

	child.stdout?.on('data', onData);

	child.stderr?.on('data', (data: Buffer) => {
		const text = data.toString().trim();
		if (text) {
			log.debug('vfs-engine stderr', { text });
		}
	});

	child.on('error', (err) => {
		log.error('vfs-engine process error', toError(err));
		rejectAll(
			createVFSError(`VFS engine process error: ${toError(err).message}`, {
				code: 'VFS_ERROR',
				cause: err,
			}),
		);
	});

	child.on('exit', (code) => {
		if (!disposed) {
			log.warn('vfs-engine exited unexpectedly', { exitCode: code });
			rejectAll(
				createVFSError(`VFS engine exited with code ${code}`, {
					code: 'VFS_ERROR',
					metadata: { exitCode: code },
				}),
			);
		}
	});

	// Send initialize request with limits and history config
	const initParams: Record<string, unknown> = {};
	if (options.limits) {
		initParams.limits = options.limits;
	}
	if (options.history) {
		initParams.history = options.history;
	}

	await sendRequest('initialize', initParams);
	log.info('VFS engine initialized');

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
