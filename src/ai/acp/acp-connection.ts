// ---------------------------------------------------------------------------
// ACP Connection — JSON-RPC 2.0 over NDJSON stdio transport
// ---------------------------------------------------------------------------

import { type ChildProcess, spawn } from 'node:child_process';
import {
	createProviderError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	toError,
} from '../../errors/index.js';
import type {
	ACPInitializeResult,
	ACPPermissionPolicy,
	JsonRpcMessage,
	JsonRpcNotification,
	JsonRpcRequest,
	JsonRpcResponse,
} from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface ACPConnectionOptions {
	readonly command: string;
	readonly args?: readonly string[];
	readonly cwd?: string;
	readonly env?: Readonly<Record<string, string>>;
	readonly timeoutMs?: number;
	readonly initTimeoutMs?: number;
	readonly permissionPolicy?: ACPPermissionPolicy;
	readonly clientName?: string;
	readonly clientVersion?: string;
}

// ---------------------------------------------------------------------------
// Connection interface
// ---------------------------------------------------------------------------

export interface ACPConnection {
	readonly initialize: () => Promise<ACPInitializeResult>;
	readonly request: <T>(method: string, params?: unknown) => Promise<T>;
	readonly notify: (method: string, params?: unknown) => void;
	readonly onNotification: (
		method: string,
		handler: (params: unknown) => void,
	) => () => void;
	readonly close: () => Promise<void>;
	readonly setPermissionPolicy: (policy: ACPPermissionPolicy) => void;
	readonly isConnected: boolean;
	readonly serverInfo: ACPInitializeResult | undefined;
	readonly permissionPolicy: ACPPermissionPolicy;
}

// ---------------------------------------------------------------------------
// Pending request tracking
// ---------------------------------------------------------------------------

interface PendingRequest {
	readonly resolve: (value: unknown) => void;
	readonly reject: (reason: unknown) => void;
	readonly timer: ReturnType<typeof setTimeout>;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createACPConnection(
	options: ACPConnectionOptions,
): ACPConnection {
	const timeoutMs = options.timeoutMs ?? 30_000;
	const initTimeoutMs = options.initTimeoutMs ?? 10_000;
	let permissionPolicy: ACPPermissionPolicy =
		options.permissionPolicy ?? 'deny';
	const clientName = options.clientName ?? 'simse';
	const clientVersion = options.clientVersion ?? '1.0.0';

	let nextId = 1;
	let child: ChildProcess | undefined;
	let connected = false;
	let serverInfo: ACPInitializeResult | undefined;
	let buffer = '';

	const pending = new Map<number, PendingRequest>();
	const notificationHandlers = new Map<
		string,
		Set<(params: unknown) => void>
	>();

	// -----------------------------------------------------------------------
	// Message parsing
	// -----------------------------------------------------------------------

	const isJsonRpcResponse = (msg: JsonRpcMessage): msg is JsonRpcResponse =>
		'id' in msg && !('method' in msg);

	const isJsonRpcRequest = (msg: JsonRpcMessage): msg is JsonRpcRequest =>
		'id' in msg && 'method' in msg;

	const isJsonRpcNotification = (
		msg: JsonRpcMessage,
	): msg is JsonRpcNotification => !('id' in msg) && 'method' in msg;

	// -----------------------------------------------------------------------
	// Permission handling
	// -----------------------------------------------------------------------

	const handlePermissionRequest = (id: number, params: unknown): void => {
		const p = params as {
			options?: readonly { optionId: string; kind: string }[];
		};
		const options = p?.options ?? [];

		if (permissionPolicy === 'auto-approve') {
			// Pick the first "allow" option, preferring allow_always > allow_once
			const allowAlways = options.find(
				(o) => o.kind === 'allow_always',
			);
			const allowOnce = options.find((o) => o.kind === 'allow_once');
			const pick = allowAlways ?? allowOnce;

			if (pick) {
				sendResponse(id, {
					outcome: { outcome: 'selected', optionId: pick.optionId },
				});
				return;
			}

			// Fallback if options don't contain allow kinds
			sendResponse(id, {
				outcome: { outcome: 'selected', optionId: options[0]?.optionId },
			});
			return;
		}

		// 'deny' or 'prompt' — reject
		const reject = options.find(
			(o) => o.kind === 'reject_once' || o.kind === 'reject_always',
		);
		if (reject) {
			sendResponse(id, {
				outcome: { outcome: 'selected', optionId: reject.optionId },
			});
		} else {
			sendResponse(id, { outcome: { outcome: 'cancelled' } });
		}
	};

	const sendResponse = (id: number, result: unknown): void => {
		if (!child?.stdin?.writable) return;

		const response: JsonRpcResponse = {
			jsonrpc: '2.0',
			id,
			result,
		};
		child.stdin.write(`${JSON.stringify(response)}\n`);
	};

	// -----------------------------------------------------------------------
	// Incoming data processing
	// -----------------------------------------------------------------------

	const processLine = (line: string): void => {
		const trimmed = line.trim();
		if (trimmed.length === 0) return;

		let msg: JsonRpcMessage;
		try {
			msg = JSON.parse(trimmed) as JsonRpcMessage;
		} catch {
			return; // Skip malformed lines
		}

		if (isJsonRpcResponse(msg)) {
			const req = pending.get(msg.id);
			if (!req) return;

			pending.delete(msg.id);
			clearTimeout(req.timer);

			if (msg.error) {
				req.reject(
					createProviderError(
						'acp',
						`ACP error ${msg.error.code}: ${msg.error.message}`,
						{
							code: 'PROVIDER_GENERATION_ERROR',
							metadata: { jsonrpcError: msg.error },
						},
					),
				);
			} else {
				req.resolve(msg.result);
			}
		} else if (isJsonRpcRequest(msg)) {
			// Server sending a request to the client (e.g., session/request_permission)
			if (msg.method === 'session/request_permission') {
				handlePermissionRequest(msg.id, msg.params);
			} else {
				// Unknown request — respond with method not found
				sendResponse(msg.id, undefined);
			}
		} else if (isJsonRpcNotification(msg)) {
			const handlers = notificationHandlers.get(msg.method);
			if (handlers) {
				for (const handler of handlers) {
					try {
						handler(msg.params);
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
	// Spawn + initialize
	// -----------------------------------------------------------------------

	const initialize = async (): Promise<ACPInitializeResult> => {
		if (connected && serverInfo) return serverInfo;

		const args = options.args ? [...options.args] : [];
		const env = options.env ? { ...process.env, ...options.env } : process.env;

		child = spawn(options.command, args, {
			stdio: ['pipe', 'pipe', 'pipe'],
			detached: false,
			cwd: options.cwd,
			env,
		});

		child.stdout?.on('data', onData);

		child.stderr?.on('data', (_data: Buffer) => {
			// stderr is not part of NDJSON — ignore or log
		});

		child.on('error', (err) => {
			connected = false;
			rejectAll(
				createProviderUnavailableError('acp', {
					cause: err,
					metadata: { reason: `Process error: ${toError(err).message}` },
				}),
			);
		});

		child.on('exit', (code) => {
			connected = false;
			rejectAll(
				createProviderUnavailableError('acp', {
					metadata: {
						reason: `ACP server exited with code ${code}`,
					},
				}),
			);
		});

		connected = true;

		const result = await sendRequest<ACPInitializeResult>(
			'initialize',
			{
				protocolVersion: 1,
				client_info: { name: clientName, version: clientVersion },
				capabilities: {},
			},
			initTimeoutMs,
		);

		serverInfo = result;
		return result;
	};

	// -----------------------------------------------------------------------
	// Send request / notification
	// -----------------------------------------------------------------------

	const sendRequest = <T>(
		method: string,
		params?: unknown,
		customTimeoutMs?: number,
	): Promise<T> => {
		if (!child?.stdin?.writable) {
			return Promise.reject(
				createProviderUnavailableError('acp', {
					metadata: { reason: 'ACP connection is not open' },
				}),
			);
		}

		const id = nextId++;
		const reqTimeoutMs = customTimeoutMs ?? timeoutMs;

		return new Promise<T>((resolve, reject) => {
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(
					createProviderTimeoutError('acp', reqTimeoutMs, {
						cause: new Error(
							`ACP request "${method}" timed out after ${reqTimeoutMs}ms`,
						),
					}),
				);
			}, reqTimeoutMs);

			pending.set(id, {
				resolve: resolve as (value: unknown) => void,
				reject,
				timer,
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

	const sendNotify = (method: string, params?: unknown): void => {
		if (!child?.stdin?.writable) return;

		const notification: JsonRpcNotification = {
			jsonrpc: '2.0',
			method,
			params,
		};

		child.stdin.write(`${JSON.stringify(notification)}\n`);
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
		for (const [_id, req] of pending) {
			clearTimeout(req.timer);
			req.reject(error);
		}
		pending.clear();
	};

	const close = async (): Promise<void> => {
		connected = false;

		rejectAll(
			createProviderUnavailableError('acp', {
				metadata: { reason: 'Connection closed' },
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
	// Return frozen interface
	// -----------------------------------------------------------------------

	return Object.freeze({
		initialize,
		request: sendRequest,
		notify: sendNotify,
		onNotification,
		close,
		setPermissionPolicy: (policy: ACPPermissionPolicy) => {
			permissionPolicy = policy;
		},
		get isConnected() {
			return connected;
		},
		get serverInfo() {
			return serverInfo;
		},
		get permissionPolicy() {
			return permissionPolicy;
		},
	});
}
