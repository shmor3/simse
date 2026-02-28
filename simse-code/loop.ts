/**
 * SimSE CLI — Agentic Loop
 *
 * Core agentic loop that sends conversation history to the ACP server,
 * parses tool calls from the response, executes them, and continues
 * until the model produces a final text response (no tool calls).
 *
 * Pattern: user input -> model reasons -> tool calls -> execute -> repeat
 */

import type { ACPClient, ACPToolCall, ACPToolCallUpdate, Logger } from 'simse';
import { toError } from 'simse';
import type { Conversation } from './conversation.js';
import type { ImageAttachment } from './image-input.js';
import type {
	ToolCallRequest,
	ToolCallResult,
	ToolRegistry,
} from './tool-registry.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgenticLoopOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly conversation: Conversation;
	readonly logger?: Logger;
	readonly maxTurns?: number;
	readonly serverName?: string;
	readonly agentId?: string;
	readonly systemPrompt?: string;
	readonly signal?: AbortSignal;
	/**
	 * When true, the ACP agent manages its own tool calling (e.g. Claude Code).
	 * Skips injecting `<tool_use>` XML and skips parsing tool calls from responses.
	 * Tool activity is reported via `onAgentToolCall`/`onAgentToolCallUpdate` callbacks.
	 */
	readonly agentManagesTools?: boolean;
}

export interface LoopTurn {
	readonly turn: number;
	readonly type: 'text' | 'tool_use';
	readonly text?: string;
	readonly toolCalls?: readonly ToolCallRequest[];
	readonly toolResults?: readonly ToolCallResult[];
}

export interface LoopCallbacks {
	readonly onStreamDelta?: (text: string) => void;
	readonly onStreamStart?: () => void;
	readonly onToolCallStart?: (call: ToolCallRequest) => void;
	readonly onToolCallEnd?: (result: ToolCallResult) => void;
	readonly onTurnComplete?: (turn: LoopTurn) => void;
	readonly onError?: (error: Error) => void;
	/** Called before tool execution to check permission. Return 'deny' to skip. */
	readonly onPermissionCheck?: (
		call: ToolCallRequest,
	) => Promise<'allow' | 'deny'>;
	/** Called when the ACP agent starts a tool call (agentManagesTools mode). */
	readonly onAgentToolCall?: (toolCall: ACPToolCall) => void;
	/** Called when the ACP agent updates a tool call (agentManagesTools mode). */
	readonly onAgentToolCallUpdate?: (update: ACPToolCallUpdate) => void;
	/** Called when the loop detects consecutive identical tool calls (doom loop). */
	readonly onDoomLoop?: (toolName: string, count: number) => void;
	/** Called when auto-compaction fires to summarize conversation. */
	readonly onCompaction?: () => void;
	/** Called with token usage after each ACP response. */
	readonly onTokenUsage?: (usage: {
		promptTokens: number;
		completionTokens: number;
		totalTokens: number;
	}) => void;
}

export interface AgenticLoopResult {
	readonly finalText: string;
	readonly turns: readonly LoopTurn[];
	readonly totalTurns: number;
	readonly hitTurnLimit: boolean;
	readonly aborted: boolean;
}

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface AgenticLoop {
	readonly run: (
		userInput: string,
		callbacks?: LoopCallbacks,
		images?: readonly ImageAttachment[],
	) => Promise<AgenticLoopResult>;
}

// ---------------------------------------------------------------------------
// Tool call parser
// ---------------------------------------------------------------------------

interface ParsedResponse {
	readonly text: string;
	readonly toolCalls: readonly ToolCallRequest[];
}

function parseToolCalls(response: string): ParsedResponse {
	const toolCalls: ToolCallRequest[] = [];
	let text = response;

	// Match <tool_use>...</tool_use> blocks
	const pattern = /<tool_use>\s*([\s\S]*?)\s*<\/tool_use>/g;
	let match: RegExpExecArray | null = pattern.exec(response);

	while (match !== null) {
		const jsonStr = match[1].trim();
		try {
			const parsed = JSON.parse(jsonStr) as {
				id?: string;
				name?: string;
				arguments?: Record<string, unknown>;
			};
			if (parsed.name) {
				toolCalls.push(
					Object.freeze({
						id: parsed.id ?? `call_${toolCalls.length + 1}`,
						name: parsed.name,
						arguments: parsed.arguments ?? {},
					}),
				);
			}
		} catch {
			// Malformed JSON — skip this tool call
		}
		match = pattern.exec(response);
	}

	// Strip tool_use blocks from the text
	text = response.replace(pattern, '').trim();

	return Object.freeze({
		text,
		toolCalls: Object.freeze(toolCalls),
	});
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createAgenticLoop(options: AgenticLoopOptions): AgenticLoop {
	const {
		acpClient,
		toolRegistry,
		conversation,
		maxTurns = 10,
		serverName,
		agentId,
		systemPrompt,
		signal,
		agentManagesTools = false,
	} = options;

	const run = async (
		userInput: string,
		callbacks?: LoopCallbacks,
		images?: readonly ImageAttachment[],
	): Promise<AgenticLoopResult> => {
		// Add user message to conversation (images sent separately via ACP content blocks)
		conversation.addUser(userInput);

		// Build system prompt: tool definitions + user-provided system prompt.
		// When agentManagesTools is true, skip injecting tool definitions —
		// the ACP agent discovers tools via MCP servers.
		const toolPrompt = agentManagesTools
			? ''
			: toolRegistry.formatForSystemPrompt();
		const fullSystemPrompt = [toolPrompt, systemPrompt]
			.filter(Boolean)
			.join('\n\n');

		if (fullSystemPrompt) {
			conversation.setSystemPrompt(fullSystemPrompt);
		}

		const turns: LoopTurn[] = [];
		let lastText = '';

		// Doom loop detection: track consecutive identical tool calls
		const maxIdenticalToolCalls = 3;
		let lastToolKey = '';
		let identicalCount = 0;

		for (let turn = 1; turn <= maxTurns; turn++) {
			// Check for abort before each turn
			if (signal?.aborted) {
				return Object.freeze({
					finalText: lastText,
					turns: Object.freeze(turns),
					totalTurns: turn - 1,
					hitTurnLimit: false,
					aborted: true,
				});
			}

			// Auto-compact when conversation exceeds threshold
			if (conversation.needsCompaction && turn > 1) {
				try {
					const compactPrompt =
						'Summarize this conversation concisely. Include: current goal, progress made, key decisions, relevant file paths, and next steps.\n\n' +
						conversation.serialize();
					const summary = await acpClient.generate(compactPrompt, {
						serverName,
					});
					if (summary.content) {
						conversation.compact(summary.content);
						callbacks?.onCompaction?.();
					}
				} catch {
					// Best-effort: continue without compaction
				}
			}

			// Serialize conversation to a single prompt string
			const prompt = conversation.serialize();

			// Stream from ACP
			let fullResponse = '';
			callbacks?.onStreamStart?.();

			try {
				const stream = acpClient.generateStream(prompt, {
					serverName,
					agentId,
					onToolCall: agentManagesTools
						? (tc) => callbacks?.onAgentToolCall?.(tc)
						: undefined,
					onToolCallUpdate: agentManagesTools
						? (update) => callbacks?.onAgentToolCallUpdate?.(update)
						: undefined,
					// Only send images on the first turn (when the user just provided them)
					...(turn === 1 && images && images.length > 0
						? {
								images: images.map((img) => ({
									mimeType: img.mimeType,
									base64: img.base64,
								})),
							}
						: {}),
				});

				for await (const chunk of stream) {
					// Check abort during streaming
					if (signal?.aborted) {
						return Object.freeze({
							finalText: fullResponse || lastText,
							turns: Object.freeze(turns),
							totalTurns: turn,
							hitTurnLimit: false,
							aborted: true,
						});
					}

					if (chunk.type === 'delta') {
						fullResponse += chunk.text;
						callbacks?.onStreamDelta?.(chunk.text);
					} else if (chunk.type === 'complete' && chunk.usage) {
						callbacks?.onTokenUsage?.(chunk.usage);
					}
				}
			} catch (err) {
				const error = toError(err);
				callbacks?.onError?.(error);
				// Surface the error as the response text so it's visible
				fullResponse =
					fullResponse ||
					`Error communicating with ACP server: ${error.message}`;
			}

			// If response is completely empty, report it
			if (!fullResponse.trim()) {
				const emptyMsg = 'No response received from model.';
				callbacks?.onError?.(new Error(emptyMsg));
				fullResponse = emptyMsg;
			}

			// When agent manages tools, skip parsing — the response is always
			// final text. The agent already executed its tools internally.
			const parsed = agentManagesTools
				? {
						text: fullResponse.trim(),
						toolCalls: [] as readonly ToolCallRequest[],
					}
				: parseToolCalls(fullResponse);
			conversation.addAssistant(fullResponse);

			if (parsed.toolCalls.length === 0) {
				// No tool calls — final response
				const loopTurn: LoopTurn = Object.freeze({
					turn,
					type: 'text' as const,
					text: parsed.text,
				});
				turns.push(loopTurn);
				callbacks?.onTurnComplete?.(loopTurn);
				lastText = parsed.text;

				return Object.freeze({
					finalText: lastText,
					turns: Object.freeze(turns),
					totalTurns: turn,
					hitTurnLimit: false,
					aborted: false,
				});
			}

			// Doom loop detection
			const toolKey = parsed.toolCalls
				.map((tc) => `${tc.name}:${JSON.stringify(tc.arguments)}`)
				.join('|');
			if (toolKey === lastToolKey) {
				identicalCount++;
				if (identicalCount >= maxIdenticalToolCalls) {
					const firstTool = parsed.toolCalls[0]?.name ?? 'unknown';
					callbacks?.onDoomLoop?.(firstTool, identicalCount);
					// Inject a warning into the conversation to break the loop
					conversation.addUser(
						'[System] You are repeating the same tool calls. Please try a different approach or respond with your findings so far.',
					);
				}
			} else {
				lastToolKey = toolKey;
				identicalCount = 1;
			}

			// Execute tool calls
			const toolResults: ToolCallResult[] = [];
			for (const call of parsed.toolCalls) {
				if (signal?.aborted) {
					return Object.freeze({
						finalText: parsed.text || lastText,
						turns: Object.freeze(turns),
						totalTurns: turn,
						hitTurnLimit: false,
						aborted: true,
					});
				}

				// Check permission before executing
				if (callbacks?.onPermissionCheck) {
					const decision = await callbacks.onPermissionCheck(call);
					if (decision === 'deny') {
						const denied: ToolCallResult = Object.freeze({
							id: call.id,
							name: call.name,
							output: 'Tool call denied by user.',
							isError: true,
						});
						toolResults.push(denied);
						callbacks?.onToolCallEnd?.(denied);
						continue;
					}
				}

				callbacks?.onToolCallStart?.(call);
				const result = await toolRegistry.execute(call);
				toolResults.push(result);
				callbacks?.onToolCallEnd?.(result);

				// Add tool result to conversation for next turn
				conversation.addToolResult(call.id, call.name, result.output);
			}

			const loopTurn: LoopTurn = Object.freeze({
				turn,
				type: 'tool_use' as const,
				text: parsed.text,
				toolCalls: Object.freeze(parsed.toolCalls),
				toolResults: Object.freeze(toolResults),
			});
			turns.push(loopTurn);
			callbacks?.onTurnComplete?.(loopTurn);
			lastText = parsed.text;
		}

		// Hit turn limit
		return Object.freeze({
			finalText: lastText,
			turns: Object.freeze(turns),
			totalTurns: maxTurns,
			hitTurnLimit: true,
			aborted: false,
		});
	};

	return Object.freeze({ run });
}
