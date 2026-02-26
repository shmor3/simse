import { describe, expect, it } from 'bun:test';
import { createConversation } from '../src/ai/conversation/conversation.js';

// ---------------------------------------------------------------------------
// replaceMessages
// ---------------------------------------------------------------------------

describe('conversation replaceMessages', () => {
	it('exposes replaceMessages method', () => {
		const conv = createConversation();
		expect(typeof conv.replaceMessages).toBe('function');
	});

	it('replaces all messages with new ones', () => {
		const conv = createConversation();
		conv.addUser('old user msg');
		conv.addAssistant('old assistant msg');
		conv.addToolResult('call_1', 'search', 'old results');
		expect(conv.messageCount).toBe(3);

		conv.replaceMessages!([
			{ role: 'user', content: 'new user msg', timestamp: Date.now() },
			{
				role: 'tool_result',
				content: '[OUTPUT PRUNED — 500 chars]',
				toolCallId: 'call_1',
				toolName: 'search',
				timestamp: Date.now(),
			},
		]);

		expect(conv.messageCount).toBe(2);
		const msgs = conv.toMessages();
		expect(msgs[0].content).toBe('new user msg');
		expect(msgs[1].content).toBe('[OUTPUT PRUNED — 500 chars]');
	});

	it('filters out system messages from replacement', () => {
		const conv = createConversation({ systemPrompt: 'Original system' });
		conv.addUser('hello');

		conv.replaceMessages!([
			{
				role: 'system',
				content: 'Should be filtered out',
				timestamp: Date.now(),
			},
			{ role: 'user', content: 'kept', timestamp: Date.now() },
		]);

		// System prompt should remain as set via setSystemPrompt / constructor
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('system');
		expect(msgs[0].content).toBe('Original system');
		expect(msgs[1].role).toBe('user');
		expect(msgs[1].content).toBe('kept');
		expect(conv.messageCount).toBe(1); // Only non-system messages
	});

	it('clears all messages when given empty array', () => {
		const conv = createConversation();
		conv.addUser('hello');
		conv.addAssistant('hi');
		conv.replaceMessages!([]);
		expect(conv.messageCount).toBe(0);
	});

	it('updates estimatedChars after replacement', () => {
		const conv = createConversation();
		conv.addUser('hello world this is a long message');
		const charsBefore = conv.estimatedChars;

		conv.replaceMessages!([{ role: 'user', content: 'short' }]);
		expect(conv.estimatedChars).toBeLessThan(charsBefore);
	});

	it('freezes replaced messages', () => {
		const conv = createConversation();
		conv.replaceMessages!([
			{ role: 'user', content: 'test', timestamp: Date.now() },
		]);
		const msgs = conv.toMessages();
		expect(Object.isFrozen(msgs[0])).toBe(true);
	});
});
