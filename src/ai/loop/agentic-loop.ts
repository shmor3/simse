// ---------------------------------------------------------------------------
// Agentic Loop
//
// Core agentic loop: send conversation to ACP, parse tool calls from
// the response, execute them, and repeat until the model produces a
// final text response (no tool calls) or the turn limit is reached.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import { createLoopError } from '../../errors/loop.js';
import type { ToolCallResult } from '../tools/types.js';
import type {
	AgenticLoop,
	AgenticLoopOptions,
	AgenticLoopResult,
	LoopCallbacks,
	LoopTurn,
} from './types.js';

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
		autoCompact = false,
		compactionProvider,
	} = options;

	const run = async (
		userInput: string,
		callbacks?: LoopCallbacks,
	): Promise<AgenticLoopResult> => {
		const loopStart = Date.now();
		conversation.addUser(userInput);

		// Build system prompt: tool definitions + user-provided system prompt
		const toolPrompt = toolRegistry.formatForSystemPrompt();
		const fullSystemPrompt = [toolPrompt, systemPrompt]
			.filter(Boolean)
			.join('\n\n');

		if (fullSystemPrompt) {
			conversation.setSystemPrompt(fullSystemPrompt);
		}

		const turns: LoopTurn[] = [];
		let lastText = '';

		for (let turn = 1; turn <= maxTurns; turn++) {
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

			// Auto-compaction
			if (autoCompact && compactionProvider && conversation.needsCompaction) {
				try {
					const summary = await compactionProvider.generate(
						`Summarize this conversation concisely, preserving key context and decisions:\n\n${conversation.serialize()}`,
					);
					conversation.compact(summary);
					callbacks?.onCompaction?.(summary);
				} catch (err) {
					callbacks?.onError?.(toError(err));
				}
			}

			const turnStart = Date.now();

			// Serialize conversation to a single prompt string
			const prompt = conversation.serialize();

			// Stream from ACP
			let fullResponse = '';
			callbacks?.onStreamStart?.();

			try {
				const stream = acpClient.generateStream(prompt, {
					serverName,
					agentId,
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
					}
				}
			} catch (err) {
				const error = toError(err);
				callbacks?.onError?.(error);
				fullResponse =
					fullResponse ||
					`Error communicating with ACP server: ${error.message}`;
			}

			// If response is completely empty, report it
			if (!fullResponse.trim()) {
				const emptyMsg = 'No response received from model.';
				callbacks?.onError?.(createLoopError(emptyMsg));
				fullResponse = emptyMsg;
			}

			// Parse response for tool calls
			const parsed = toolRegistry.parseToolCalls(fullResponse);
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
				lastText = parsed.text;

				return Object.freeze({
					finalText: lastText,
					turns: Object.freeze(turns),
					totalTurns: turn,
					hitTurnLimit: false,
					aborted: false,
					totalDurationMs: Date.now() - loopStart,
				});
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
						totalDurationMs: Date.now() - loopStart,
					});
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
				durationMs: Date.now() - turnStart,
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
			totalDurationMs: Date.now() - loopStart,
		});
	};

	return Object.freeze({ run });
}
