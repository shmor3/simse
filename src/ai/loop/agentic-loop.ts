// ---------------------------------------------------------------------------
// Agentic Loop
//
// Core agentic loop: send conversation to ACP, parse tool calls from
// the response, execute them, and repeat until the model produces a
// final text response (no tool calls) or the turn limit is reached.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import { createLoopError } from '../../errors/loop.js';
import { isTransientError } from '../../utils/retry.js';
import type { ToolCallResult } from '../tools/types.js';
import type {
	AgenticLoop,
	AgenticLoopOptions,
	AgenticLoopResult,
	LoopCallbacks,
	LoopTurn,
} from './types.js';

// ---------------------------------------------------------------------------
// Transient tool error heuristic
// ---------------------------------------------------------------------------

const TRANSIENT_PATTERNS =
	/timeout|unavailable|econnrefused|econnreset|etimedout|socket hang up|network|503|429/i;

function isTransientLikeToolError(output: string): boolean {
	return TRANSIENT_PATTERNS.test(output);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an agentic loop that repeatedly streams from ACP, parses tool calls,
 * executes them via the tool registry, and feeds results back until the agent
 * produces a final text response or hits the turn limit.
 *
 * @param options - ACP client, tool registry, conversation, max turns, signal.
 * @returns A frozen {@link AgenticLoop} with a `run(prompt)` method.
 * @throws {LoopTurnLimitError} When `maxTurns` is exceeded.
 * @throws {LoopAbortedError} When the AbortSignal fires mid-loop.
 */
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
		autoCompact = false,
		compactionProvider,
		streamRetry,
		toolRetry,
		eventBus,
		memoryMiddleware,
		agentManagesTools = false,
		systemPromptBuilder,
		contextPruner,
	} = options;

	const streamMaxAttempts = streamRetry?.maxAttempts ?? 2;
	const streamBaseDelayMs = streamRetry?.baseDelayMs ?? 1000;
	const toolMaxAttempts = toolRetry?.maxAttempts ?? 2;
	const toolBaseDelayMs = toolRetry?.baseDelayMs ?? 500;

	const run = async (
		userInput: string,
		callbacks?: LoopCallbacks,
	): Promise<AgenticLoopResult> => {
		const loopStart = Date.now();
		conversation.addUser(userInput);

		// Build base system prompt: use builder if provided, otherwise
		// concatenate tool definitions + user-provided system prompt.
		// When agentManagesTools is true, skip injecting tool definitions — the
		// ACP agent discovers tools via MCP servers passed during session/new.
		let baseSystemPrompt: string;
		if (systemPromptBuilder) {
			baseSystemPrompt = systemPromptBuilder.build();
		} else {
			const toolPrompt = agentManagesTools
				? ''
				: toolRegistry.formatForSystemPrompt();
			baseSystemPrompt = [toolPrompt, systemPrompt]
				.filter(Boolean)
				.join('\n\n');
		}

		const turns: LoopTurn[] = [];
		let lastText = '';

		for (let turn = 1; turn <= maxTurns; turn++) {
			// Enrich system prompt with memory context each turn
			let turnSystemPrompt = baseSystemPrompt;
			if (memoryMiddleware) {
				try {
					turnSystemPrompt = await memoryMiddleware.enrichSystemPrompt({
						userInput,
						currentSystemPrompt: baseSystemPrompt,
						conversationHistory: conversation.serialize(),
						turn,
					});
				} catch {
					// Graceful degradation: use base prompt
				}
			}

			if (turnSystemPrompt) {
				conversation.setSystemPrompt(turnSystemPrompt);
			}
			// Check for abort before each turn
			if (signal?.aborted) {
				return Object.freeze({
					finalText: lastText,
					turns: Object.freeze(turns),
					totalTurns: turn - 1,
					hitTurnLimit: false,
					aborted: true,
					totalDurationMs: Date.now() - loopStart,
				});
			}

			// Two-stage compaction:
			// Stage 1: Lightweight pruning — strip old tool outputs
			// Stage 2: Full summarization — only if still over threshold after pruning
			if (autoCompact && conversation.needsCompaction) {
				// Stage 1: Context pruning (no LLM call needed)
				if (contextPruner && conversation.replaceMessages) {
					try {
						const pruned = contextPruner.prune(conversation.toMessages());
						conversation.replaceMessages(pruned);
						eventBus?.publish('compaction.prune', {
							messageCount: conversation.messageCount,
							estimatedChars: conversation.estimatedChars,
						});
					} catch (err) {
						callbacks?.onError?.(toError(err));
					}
				}

				// Stage 2: Full summarization (only if still over threshold)
				if (compactionProvider && conversation.needsCompaction) {
					try {
						eventBus?.publish('compaction.start', {
							messageCount: conversation.messageCount,
							estimatedChars: conversation.estimatedChars,
						});
						const summary = await compactionProvider.generate(
							`Summarize this conversation concisely, preserving key context and decisions:\n\n${conversation.serialize()}`,
						);
						conversation.compact(summary);
						callbacks?.onCompaction?.(summary);
						eventBus?.publish('compaction.complete', {
							summaryLength: summary.length,
						});
					} catch (err) {
						callbacks?.onError?.(toError(err));
					}
				}
			}

			const turnStart = Date.now();

			// Serialize conversation to a single prompt string
			const prompt = conversation.serialize();

			// Stream from ACP with retry on transient errors
			let fullResponse = '';
			callbacks?.onStreamStart?.();

			for (
				let streamAttempt = 1;
				streamAttempt <= streamMaxAttempts;
				streamAttempt++
			) {
				if (streamAttempt > 1) {
					// Reset accumulated response before retry
					fullResponse = '';
					const delay = streamBaseDelayMs * 2 ** (streamAttempt - 2);
					await new Promise<void>((r) => setTimeout(r, delay));
				}

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
								totalDurationMs: Date.now() - loopStart,
							});
						}

						if (chunk.type === 'delta') {
							fullResponse += chunk.text;
							callbacks?.onStreamDelta?.(chunk.text);
							eventBus?.publish('stream.delta', { text: chunk.text });
						}
					}
					break; // success — exit retry loop
				} catch (err) {
					if (streamAttempt < streamMaxAttempts && isTransientError(err)) {
						continue; // retry
					}
					const error = toError(err);
					callbacks?.onError?.(error);
					fullResponse =
						fullResponse ||
						`Error communicating with ACP server: ${error.message}`;
					break;
				}
			}

			// If response is completely empty, report it
			if (!fullResponse.trim()) {
				const emptyMsg = 'No response received from model.';
				callbacks?.onError?.(createLoopError(emptyMsg));
				fullResponse = emptyMsg;
			}

			// When agent manages tools, skip parsing — the response is always
			// final text. The agent already executed its tools internally.
			const parsed = agentManagesTools
				? {
						text: fullResponse.trim(),
						toolCalls:
							[] as readonly import('../tools/types.js').ToolCallRequest[],
					}
				: toolRegistry.parseToolCalls(fullResponse);
			conversation.addAssistant(fullResponse);

			if (parsed.toolCalls.length === 0) {
				// No tool calls — final response
				const loopTurn: LoopTurn = Object.freeze({
					turn,
					type: 'text' as const,
					text: parsed.text,
					durationMs: Date.now() - turnStart,
				});
				turns.push(loopTurn);
				callbacks?.onTurnComplete?.(loopTurn);
				eventBus?.publish('turn.complete', {
					turn: loopTurn.turn,
					type: loopTurn.type,
				});
				lastText = parsed.text;

				// Store response in memory after final text turn
				if (memoryMiddleware && lastText) {
					try {
						await memoryMiddleware.afterResponse(userInput, lastText);
					} catch {
						// Graceful degradation
					}
				}

				return Object.freeze({
					finalText: lastText,
					turns: Object.freeze(turns),
					totalTurns: turn,
					hitTurnLimit: false,
					aborted: false,
					totalDurationMs: Date.now() - loopStart,
				});
			}

			// Execute tool calls with retry on transient-looking errors
			const toolResults: ToolCallResult[] = [];
			for (const call of parsed.toolCalls) {
				if (signal?.aborted) {
					return Object.freeze({
						finalText: parsed.text || lastText,
						turns: Object.freeze(turns),
						totalTurns: turn,
						hitTurnLimit: false,
						aborted: true,
						totalDurationMs: Date.now() - loopStart,
					});
				}

				callbacks?.onToolCallStart?.(call);
				eventBus?.publish('tool.call.start', {
					callId: call.id,
					name: call.name,
					args: call.arguments,
				});

				let result = await toolRegistry.execute(call);

				// Retry tool if it returned a transient-looking error
				if (result.isError && toolMaxAttempts > 1) {
					for (
						let toolAttempt = 2;
						toolAttempt <= toolMaxAttempts;
						toolAttempt++
					) {
						if (!isTransientLikeToolError(result.output)) break;
						const delay = toolBaseDelayMs * 2 ** (toolAttempt - 2);
						await new Promise<void>((r) => setTimeout(r, delay));
						result = await toolRegistry.execute(call);
						if (!result.isError) break;
					}
				}

				toolResults.push(result);
				callbacks?.onToolCallEnd?.(result);
				eventBus?.publish('tool.call.end', {
					callId: result.id,
					name: result.name,
					output: result.output,
					isError: result.isError,
					durationMs: result.durationMs ?? 0,
				});

				// Add tool result to conversation for next turn
				conversation.addToolResult(call.id, call.name, result.output);
			}

			const loopTurn: LoopTurn = Object.freeze({
				turn,
				type: 'tool_use' as const,
				text: parsed.text,
				toolCalls: Object.freeze(parsed.toolCalls),
				toolResults: Object.freeze(toolResults),
				durationMs: Date.now() - turnStart,
			});
			turns.push(loopTurn);
			callbacks?.onTurnComplete?.(loopTurn);
			eventBus?.publish('turn.complete', {
				turn: loopTurn.turn,
				type: loopTurn.type,
			});
			lastText = parsed.text;
		}

		// Hit turn limit — store response in memory
		if (memoryMiddleware && lastText) {
			try {
				await memoryMiddleware.afterResponse(userInput, lastText);
			} catch {
				// Graceful degradation
			}
		}

		return Object.freeze({
			finalText: lastText,
			turns: Object.freeze(turns),
			totalTurns: maxTurns,
			hitTurnLimit: true,
			aborted: false,
			totalDurationMs: Date.now() - loopStart,
		});
	};

	return Object.freeze({ run });
}
