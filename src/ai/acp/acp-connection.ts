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

/**
 * A permission option presented by the ACP agent.
 */
export interface ACPPermissionOption {
	readonly optionId: string;
	readonly kind: string;
	/** ACP spec field — human-readable label for the option. */
	readonly name?: string;
	/** @deprecated Use `name` — kept for backwards compat with older servers. */
	readonly title?: string;
	readonly description?: string;
}

/**
 * Tool call details attached to a permission request.
 */
export interface ACPPermissionToolCall {
	readonly toolCallId?: string;
	readonly title?: string;
	readonly kind?: string;
	readonly rawInput?: unknown;
	readonly status?: string;
}

/**
 * Info passed to the permission handler callback in 'prompt' mode.
 */
export interface ACPPermissionRequestInfo {
	readonly title?: string;
	readonly description?: string;
	readonly toolCall?: ACPPermissionToolCall;
	readonly options: readonly ACPPermissionOption[];
}

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
	readonly stderrHandler?: (text: string) => void;
	/**
	 * Called when the ACP agent requests permission and the policy is 'prompt'.
	 * Return the selected optionId, or undefined to reject.
	 */
	readonly onPermissionRequest?: (
		info: ACPPermissionRequestInfo,
	) => Promise<string | undefined>;
}

// ---------------------------------------------------------------------------
// Connection interface
// ---------------------------------------------------------------------------

export interface ACPConnection {
	readonly initialize: () => Promise<ACPInitializeResult>;
	readonly request: <T>(
		method: string,
		params?: unknown,
		customTimeoutMs?: number,
		signal?: AbortSignal,
	) => Promise<T>;
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
	timer: ReturnType<typeof setTimeout>;
	readonly timeoutMs: number;
	readonly method: string;
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
		options.permissionPolicy ?? 'prompt';
	const clientName = options.clientName ?? 'simse';
	const clientVersion = options.clientVersion ?? '1.0.0';
	const stderrHandler = options.stderrHandler ?? (() => {});
	const onPermissionRequest = options.onPermissionRequest;

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
			title?: string;
			description?: string;
			toolCall?: ACPPermissionToolCall;
			options?: readonly ACPPermissionOption[];
		};
		const permOptions = p?.options ?? [];

		// Suspend the prompt timeout entirely while a permission prompt is
		// active — the user may take an arbitrary amount of time to decide.
		// The timeout is restored after the permission response is sent.
		for (const [, req] of pending) {
			if (req.method === 'session/prompt') {
				clearTimeout(req.timer);
			}
		}

		const restorePromptTimeouts = (): void => {
			for (const [, req] of pending) {
				if (req.method === 'session/prompt') {
					req.timer = setTimeout(() => {
						for (const [reqId, r] of pending) {
							if (r === req) {
								pending.delete(reqId);
								break;
							}
						}
						req.reject(
							createProviderTimeoutError('acp', req.timeoutMs, {
								cause: new Error(
									`ACP request "${req.method}" timed out after ${req.timeoutMs}ms`,
								),
							}),
						);
					}, req.timeoutMs);
				}
			}
		};

		if (permissionPolicy === 'auto-approve') {
			// Pick the first "allow" option, preferring allow_always > allow_once
			const allowAlways = permOptions.find((o) => o.kind === 'allow_always');
			const allowOnce = permOptions.find((o) => o.kind === 'allow_once');
			const pick = allowAlways ?? allowOnce;

			if (pick) {
				sendResponse(id, {
					outcome: { outcome: 'selected', optionId: pick.optionId },
				});
			} else {
				// Fallback if options don't contain allow kinds
				sendResponse(id, {
					outcome: {
						outcome: 'selected',
						optionId: permOptions[0]?.optionId,
					},
				});
			}
			restorePromptTimeouts();
			return;
		}

		if (permissionPolicy === 'prompt' && onPermissionRequest) {
			// Delegate to the consumer's handler (fire-and-forget async)
			onPermissionRequest({
				title: p?.title,
				description: p?.description,
				toolCall: p?.toolCall,
				options: permOptions,
			})
				.then((selectedId) => {
					if (selectedId) {
						sendResponse(id, {
							outcome: { outcome: 'selected', optionId: selectedId },
						});
					} else {
						// User cancelled or handler returned undefined — reject
						const reject = permOptions.find(
							(o) => o.kind === 'reject_once' || o.kind === 'reject_always',
						);
						sendResponse(id, {
							outcome: reject
								? { outcome: 'selected', optionId: reject.optionId }
								: { outcome: 'cancelled' },
						});
					}
				})
				.catch(() => {
					// Handler threw — reject for safety
					sendResponse(id, { outcome: { outcome: 'cancelled' } });
				})
				.finally(() => {
					restorePromptTimeouts();
				});
			return;
		}

		// 'deny' or 'prompt' without handler — reject
		const reject = permOptions.find(
			(o) => o.kind === 'reject_once' || o.kind === 'reject_always',
		);
		if (reject) {
			sendResponse(id, {
				outcome: { outcome: 'selected', optionId: reject.optionId },
			});
		} else {
			sendResponse(id, { outcome: { outcome: 'cancelled' } });
		}
		restorePromptTimeouts();
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
			// Reset timeout for pending prompt requests on session/update
			// notifications — the server is actively streaming, not stalled
			if (msg.method === 'session/update') {
				for (const [, req] of pending) {
					if (req.method === 'session/prompt') {
						clearTimeout(req.timer);
						req.timer = setTimeout(() => {
							for (const [reqId, r] of pending) {
								if (r === req) {
									pending.delete(reqId);
									break;
								}
							}
							req.reject(
								createProviderTimeoutError('acp', req.timeoutMs, {
									cause: new Error(
										`ACP request "${req.method}" timed out after ${req.timeoutMs}ms`,
									),
								}),
							);
						}, req.timeoutMs);
					}
				}
			}

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
		const baseEnv = { ...process.env, ...options.env };
		// Strip CLAUDECODE env var so child ACP processes aren't blocked
		// by Claude Code's nested-session detection
		delete baseEnv.CLAUDECODE;
		const env = baseEnv;

		child = spawn(options.command, args, {
			stdio: ['pipe', 'pipe', 'pipe'],
			detached: false,
			cwd: options.cwd,
			env,
		});

		child.stdout?.on('data', onData);

		child.stderr?.on('data', (data: Buffer) => {
			const text = data.toString().trim();
			if (text) {
				stderrHandler(text);
			}
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
		signal?: AbortSignal,
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

			if (signal) {
				const onAbort = () => {
					pending.delete(id);
					clearTimeout(timer);
					reject(
						createProviderTimeoutError('acp', 0, {
							cause: new Error('Request aborted'),
						}),
					);
				};
				if (signal.aborted) {
					onAbort();
					return;
				}
				signal.addEventListener('abort', onAbort, { once: true });
			}

			pending.set(id, {
				resolve: resolve as (value: unknown) => void,
				reject,
				timer,
				timeoutMs: reqTimeoutMs,
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
