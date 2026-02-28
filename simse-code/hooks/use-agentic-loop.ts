import { useCallback, useRef, useState } from 'react';
import type { ACPClient, ACPToolCall, ACPToolCallUpdate } from 'simse';
import { toError } from 'simse';
import type { Conversation } from '../conversation.js';
import type { ImageAttachment } from '../image-input.js';
import type { OutputItem, ToolCallState } from '../ink-types.js';
import {
	type AgenticLoopResult,
	createAgenticLoop,
	type LoopCallbacks,
} from '../loop.js';
import type { PermissionManager } from '../permission-manager.js';
import type { ToolCallRequest, ToolRegistry } from '../tool-registry.js';

export function deriveToolSummary(
	name: string,
	output: string,
): string | undefined {
	if (!output) return undefined;
	const lines = output.split('\n');
	if (lines.length > 1) return `${lines.length} lines`;
	if (output.length > 100) return `${output.length} chars`;
	return undefined;
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface UseAgenticLoopOptions {
	readonly acpClient: ACPClient;
	readonly conversation: Conversation;
	readonly toolRegistry: ToolRegistry;
	readonly serverName?: string;
	readonly maxTurns?: number;
	readonly permissionManager?: PermissionManager;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

interface AgenticLoopState {
	readonly status: 'idle' | 'streaming' | 'tool-executing';
	readonly streamText: string;
	readonly activeToolCalls: readonly ToolCallState[];
	readonly pendingPermission: {
		readonly call: ToolCallRequest;
		readonly resolve: (decision: 'allow' | 'deny') => void;
	} | null;
}

export interface UseAgenticLoopResult {
	readonly state: AgenticLoopState;
	readonly submit: (
		input: string,
		images?: readonly ImageAttachment[],
	) => Promise<readonly OutputItem[]>;
	readonly abort: () => void;
	readonly pendingPermission: AgenticLoopState['pendingPermission'];
	readonly resolvePermission: (
		decision: 'allow' | 'deny',
		alwaysAllow?: boolean,
	) => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgenticLoop(
	options: UseAgenticLoopOptions,
): UseAgenticLoopResult {
	const [state, setState] = useState<AgenticLoopState>({
		status: 'idle',
		streamText: '',
		activeToolCalls: [],
		pendingPermission: null,
	});

	const abortRef = useRef<AbortController | undefined>(undefined);
	const optionsRef = useRef(options);
	optionsRef.current = options;

	const submit = useCallback(
		async (
			input: string,
			images?: readonly ImageAttachment[],
		): Promise<readonly OutputItem[]> => {
			const ctrl = new AbortController();
			abortRef.current = ctrl;

			const { acpClient, conversation, toolRegistry, serverName, maxTurns } =
				optionsRef.current;

			setState({
				status: 'streaming',
				streamText: '',
				activeToolCalls: [],
			});

			const completedItems: OutputItem[] = [];
			const toolTimings = new Map<string, number>();
			const toolArgsMap = new Map<string, string>();

			const loop = createAgenticLoop({
				acpClient,
				toolRegistry,
				conversation,
				maxTurns: maxTurns ?? 10,
				serverName,
				signal: ctrl.signal,
				agentManagesTools: false,
			});

			let currentStreamText = '';

			const callbacks: LoopCallbacks = {
				onStreamStart: () => {
					setState((prev) => ({
						...prev,
						status: 'streaming',
						streamText: '',
					}));
					currentStreamText = '';
				},
				onStreamDelta: (text: string) => {
					currentStreamText += text;
					setState((prev) => ({
						...prev,
						status: 'streaming',
						streamText: currentStreamText,
					}));
				},
				onToolCallStart: (call) => {
					// Flush any accumulated stream text as a message
					if (currentStreamText.trim()) {
						completedItems.push({
							kind: 'message',
							role: 'assistant',
							text: currentStreamText.trim(),
						});
						currentStreamText = '';
					}

					const argsStr = JSON.stringify(call.arguments);
					toolTimings.set(call.id, Date.now());
					toolArgsMap.set(call.id, argsStr);
					const toolState: ToolCallState = {
						id: call.id,
						name: call.name,
						args: argsStr,
						status: 'active',
						startedAt: Date.now(),
					};
					setState((prev) => ({
						...prev,
						status: 'tool-executing',
						streamText: '',
						activeToolCalls: [...prev.activeToolCalls, toolState],
					}));
				},
				onToolCallEnd: (result) => {
					const startTime = toolTimings.get(result.id);
					const durationMs =
						startTime !== undefined ? Date.now() - startTime : undefined;
					const summary = result.isError
						? undefined
						: deriveToolSummary(result.name, result.output);

					completedItems.push({
						kind: 'tool-call',
						name: result.name,
						args: toolArgsMap.get(result.id) ?? '{}',
						status: result.isError ? 'failed' : 'completed',
						duration: durationMs,
						summary,
						error: result.isError ? result.output : undefined,
					});

					setState((prev) => ({
						...prev,
						activeToolCalls: prev.activeToolCalls.filter(
							(tc) => tc.id !== result.id,
						),
					}));
				},
				onAgentToolCall: (toolCall: ACPToolCall) => {
					const toolState: ToolCallState = {
						id: toolCall.toolCallId,
						name: toolCall.title,
						args: '{}',
						status: 'active',
						startedAt: Date.now(),
					};
					toolTimings.set(toolCall.toolCallId, Date.now());
					setState((prev) => ({
						...prev,
						status: 'tool-executing',
						activeToolCalls: [...prev.activeToolCalls, toolState],
					}));
				},
				onAgentToolCallUpdate: (update: ACPToolCallUpdate) => {
					if (update.status === 'completed' || update.status === 'failed') {
						const startTime = toolTimings.get(update.toolCallId);
						const durationMs =
							startTime !== undefined ? Date.now() - startTime : undefined;

						completedItems.push({
							kind: 'tool-call',
							name: update.toolCallId,
							args: '{}',
							status: update.status === 'failed' ? 'failed' : 'completed',
							duration: durationMs,
						});

						setState((prev) => ({
							...prev,
							activeToolCalls: prev.activeToolCalls.filter(
								(tc) => tc.id !== update.toolCallId,
							),
						}));
					}
				},
				onPermissionCheck: async (call) => {
					const { permissionManager } = optionsRef.current;
					if (!permissionManager) return 'allow';

					const decision = permissionManager.check(
						call.name,
						call.arguments,
					);
					if (decision === 'allow') return 'allow';
					if (decision === 'deny') return 'deny';

					// decision === 'ask' — show dialog and wait for user
					return new Promise<'allow' | 'deny'>((resolve) => {
						setState((prev) => ({
							...prev,
							pendingPermission: { call, resolve },
						}));
					});
				},
				onDoomLoop: (toolName: string, count: number) => {
					completedItems.push({
						kind: 'info',
						text: `Warning: detected repeated "${toolName}" calls (${count}x). Asking model to try a different approach.`,
					});
				},
				onError: (error: Error) => {
					completedItems.push({
						kind: 'error',
						message: error.message,
					});
				},
			};

			try {
				const result: AgenticLoopResult = await loop.run(
					input,
					callbacks,
					images,
				);

				// Flush final stream text as assistant message
				const finalText = currentStreamText.trim() || result.finalText;
				if (finalText) {
					completedItems.push({
						kind: 'message',
						role: 'assistant',
						text: finalText,
					});
				}

				// Notify when turn limit is reached
				if (result.hitTurnLimit) {
					completedItems.push({
						kind: 'info',
						text: `Reached turn limit (${result.totalTurns} turns). Use /continue or send another message to keep going.`,
					});
				}
			} catch (err) {
				const error = toError(err);
				completedItems.push({
					kind: 'error',
					message: error.message,
				});
			} finally {
				setState({
					status: 'idle',
					streamText: '',
					activeToolCalls: [],
					pendingPermission: null,
				});
			}

			return completedItems;
		},
		[],
	);

	const abort = useCallback(() => {
		abortRef.current?.abort();
		setState({
			status: 'idle',
			streamText: '',
			activeToolCalls: [],
			pendingPermission: null,
		});
	}, []);

	const resolvePermission = useCallback(
		(decision: 'allow' | 'deny', alwaysAllow?: boolean) => {
			setState((prev) => {
				if (prev.pendingPermission) {
					if (alwaysAllow && optionsRef.current.permissionManager) {
						optionsRef.current.permissionManager.addRule({
							tool: prev.pendingPermission.call.name,
							policy: 'allow',
						});
					}
					prev.pendingPermission.resolve(decision);
				}
				return { ...prev, pendingPermission: null };
			});
		},
		[],
	);

	return {
		state,
		submit,
		abort,
		pendingPermission: state.pendingPermission,
		resolvePermission,
	};
}
