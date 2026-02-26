import { describe, expect, it } from 'bun:test';
import { createContextPruner } from '../src/ai/conversation/context-pruner.js';
import type { ConversationMessage } from '../src/ai/conversation/types.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function msg(
	role: ConversationMessage['role'],
	content: string,
	extra?: Partial<ConversationMessage>,
): ConversationMessage {
	return Object.freeze({ role, content, ...extra });
}

function toolResult(
	toolName: string,
	content: string,
	toolCallId = `call_${Math.random().toString(36).slice(2, 8)}`,
): ConversationMessage {
	return msg('tool_result', content, { toolName, toolCallId });
}

/** Generate a string of exactly N characters. */
function chars(n: number): string {
	return 'x'.repeat(n);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createContextPruner', () => {
	it('returns a frozen object', () => {
		const pruner = createContextPruner();
		expect(Object.isFrozen(pruner)).toBe(true);
	});

	it('prunes old tool outputs beyond protected window', () => {
		const pruner = createContextPruner({ protectRecentTurns: 1 });

		const messages: ConversationMessage[] = [
			msg('user', 'first question'),
			msg('assistant', 'let me search'),
			toolResult('web_search', chars(500)),
			msg('assistant', 'here is what I found'),
			msg('user', 'second question'),
			msg('assistant', 'let me look again'),
			toolResult('web_search', chars(300)),
			msg('assistant', 'done'),
		];

		const result = pruner.prune(messages);

		// First tool result (index 2) should be pruned
		expect(result[2].content).toBe('[OUTPUT PRUNED \u2014 500 chars]');
		expect(result[2].role).toBe('tool_result');
		expect(result[2].toolName).toBe('web_search');

		// Second tool result (index 6) is inside the protected window — preserved
		expect(result[6].content).toBe(chars(300));
	});

	it('includes original size in pruned marker', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });
		const content = chars(1234);

		const messages: ConversationMessage[] = [
			msg('user', 'question'),
			toolResult('search', content),
		];

		const result = pruner.prune(messages);
		expect(result[1].content).toBe('[OUTPUT PRUNED \u2014 1234 chars]');
	});

	it('preserves toolCallId and toolName on pruned messages', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('my_tool', chars(500), 'call_abc'),
		];

		const result = pruner.prune(messages);
		expect(result[1].toolCallId).toBe('call_abc');
		expect(result[1].toolName).toBe('my_tool');
		expect(result[1].role).toBe('tool_result');
	});

	it('preserves timestamp on pruned messages', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });
		const ts = Date.now();

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('my_tool', chars(500), 'call_abc'),
		];
		// Manually add timestamp
		const withTs: ConversationMessage[] = [
			messages[0],
			Object.freeze({ ...messages[1], timestamp: ts }),
		];

		const result = pruner.prune(withTs);
		expect(result[1].timestamp).toBe(ts);
	});

	it('preserves protected tools', () => {
		const pruner = createContextPruner({
			protectRecentTurns: 0,
			pruneProtectedTools: ['memory_search'],
		});

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('memory_search', chars(500)),
			toolResult('web_search', chars(500)),
		];

		const result = pruner.prune(messages);

		// memory_search should be preserved
		expect(result[1].content).toBe(chars(500));
		// web_search should be pruned
		expect(result[2].content).toBe('[OUTPUT PRUNED \u2014 500 chars]');
	});

	it('preserves messages after summary marker', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'old question'),
			toolResult('search', chars(500)),
			msg('assistant', '[SUMMARY] The user asked about X'),
			toolResult('search', chars(400)),
			msg('user', 'new question'),
		];

		const result = pruner.prune(messages);

		// Before summary — pruned
		expect(result[1].content).toBe('[OUTPUT PRUNED \u2014 500 chars]');
		// After summary — preserved
		expect(result[3].content).toBe(chars(400));
	});

	it('returns same messages when nothing to prune', () => {
		const pruner = createContextPruner();

		const messages: ConversationMessage[] = [
			msg('user', 'hello'),
			msg('assistant', 'hi'),
			msg('user', 'how are you'),
			msg('assistant', 'good'),
		];

		const result = pruner.prune(messages);
		// Should return the exact same array reference
		expect(result).toBe(messages);
	});

	it('skips pruning for short content (< 200 chars)', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('search', chars(199)),
			toolResult('search', chars(200)),
		];

		const result = pruner.prune(messages);

		// 199 chars — not pruned
		expect(result[1].content).toBe(chars(199));
		// 200 chars — pruned
		expect(result[2].content).toBe('[OUTPUT PRUNED \u2014 200 chars]');
	});

	it('defaults to protectRecentTurns=2', () => {
		const pruner = createContextPruner();

		const messages: ConversationMessage[] = [
			msg('user', 'turn 1'),
			toolResult('search', chars(500)),
			msg('assistant', 'response 1'),
			msg('user', 'turn 2'),
			toolResult('search', chars(500)),
			msg('assistant', 'response 2'),
			msg('user', 'turn 3'),
			toolResult('search', chars(500)),
			msg('assistant', 'response 3'),
		];

		const result = pruner.prune(messages);

		// turn 1 tool result should be pruned (outside 2-turn window)
		expect(result[1].content).toBe('[OUTPUT PRUNED \u2014 500 chars]');
		// turn 2 tool result should be preserved (within 2-turn window)
		expect(result[4].content).toBe(chars(500));
		// turn 3 tool result should be preserved (within 2-turn window)
		expect(result[7].content).toBe(chars(500));
	});

	it('does not prune non-tool_result messages', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const longContent = chars(500);
		const messages: ConversationMessage[] = [
			msg('user', longContent),
			msg('assistant', longContent),
			msg('system', longContent),
		];

		const result = pruner.prune(messages);
		// None should be modified — no tool_result messages
		expect(result).toBe(messages);
	});

	it('handles empty message array', () => {
		const pruner = createContextPruner();
		const result = pruner.prune([]);
		expect(result).toEqual([]);
	});

	it('freezes pruned messages', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('search', chars(500)),
		];

		const result = pruner.prune(messages);
		expect(Object.isFrozen(result[1])).toBe(true);
	});

	it('freezes the returned array when pruning occurred', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('search', chars(500)),
		];

		const result = pruner.prune(messages);
		expect(Object.isFrozen(result)).toBe(true);
	});

	it('handles multiple protected tools', () => {
		const pruner = createContextPruner({
			protectRecentTurns: 0,
			pruneProtectedTools: ['memory_search', 'file_read'],
		});

		const messages: ConversationMessage[] = [
			msg('user', 'go'),
			toolResult('memory_search', chars(500)),
			toolResult('file_read', chars(500)),
			toolResult('web_search', chars(500)),
		];

		const result = pruner.prune(messages);

		expect(result[1].content).toBe(chars(500)); // protected
		expect(result[2].content).toBe(chars(500)); // protected
		expect(result[3].content).toBe('[OUTPUT PRUNED \u2014 500 chars]'); // pruned
	});

	it('summary marker only considers assistant messages', () => {
		const pruner = createContextPruner({ protectRecentTurns: 0 });

		const messages: ConversationMessage[] = [
			msg('user', 'question with [SUMMARY] in it'),
			toolResult('search', chars(500)),
			msg('user', 'next'),
		];

		const result = pruner.prune(messages);
		// User message with [SUMMARY] should not create a barrier
		expect(result[1].content).toBe('[OUTPUT PRUNED \u2014 500 chars]');
	});
});
