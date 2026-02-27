import { describe, expect, it } from 'bun:test';
import { createConversation } from '../src/ai/conversation/conversation.js';

describe('createConversation', () => {
	it('returns a frozen object', () => {
		const conv = createConversation();
		expect(Object.isFrozen(conv)).toBe(true);
	});

	it('starts with zero messages', () => {
		const conv = createConversation();
		expect(conv.messageCount).toBe(0);
	});

	it('adds user messages', () => {
		const conv = createConversation();
		conv.addUser('hello');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('user');
		expect(msgs[0].content).toBe('hello');
	});

	it('adds assistant messages', () => {
		const conv = createConversation();
		conv.addAssistant('hi there');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('assistant');
	});

	it('adds tool result messages', () => {
		const conv = createConversation();
		conv.addToolResult('call_1', 'library_search', 'found 3 results');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('tool_result');
		expect(msgs[0].toolCallId).toBe('call_1');
		expect(msgs[0].toolName).toBe('library_search');
	});

	it('adds timestamps to messages', () => {
		const conv = createConversation();
		const before = Date.now();
		conv.addUser('hello');
		const after = Date.now();
		const msgs = conv.toMessages();
		expect(msgs[0].timestamp).toBeGreaterThanOrEqual(before);
		expect(msgs[0].timestamp).toBeLessThanOrEqual(after);
	});

	it('includes system prompt in toMessages', () => {
		const conv = createConversation({ systemPrompt: 'You are helpful.' });
		conv.addUser('hello');
		const msgs = conv.toMessages();
		expect(msgs.length).toBe(2);
		expect(msgs[0].role).toBe('system');
		expect(msgs[0].content).toBe('You are helpful.');
		expect(msgs[1].role).toBe('user');
	});

	it('setSystemPrompt updates the prompt', () => {
		const conv = createConversation({ systemPrompt: 'original' });
		conv.setSystemPrompt('updated');
		const msgs = conv.toMessages();
		expect(msgs[0].content).toBe('updated');
	});

	it('returns frozen messages from toMessages', () => {
		const conv = createConversation();
		conv.addUser('hello');
		const msgs = conv.toMessages();
		expect(Object.isFrozen(msgs)).toBe(true);
	});

	it('serialize formats messages correctly', () => {
		const conv = createConversation({ systemPrompt: 'sys' });
		conv.addUser('hello');
		conv.addAssistant('hi');
		conv.addToolResult('call_1', 'search', 'results');
		const serialized = conv.serialize();
		expect(serialized).toContain('[System]\nsys');
		expect(serialized).toContain('[User]\nhello');
		expect(serialized).toContain('[Assistant]\nhi');
		expect(serialized).toContain('[Tool Result: search]\nresults');
	});

	it('clear removes all messages', () => {
		const conv = createConversation();
		conv.addUser('hello');
		conv.addAssistant('hi');
		conv.clear();
		expect(conv.messageCount).toBe(0);
	});

	it('compact replaces messages with summary', () => {
		const conv = createConversation();
		conv.addUser('hello');
		conv.addAssistant('hi');
		conv.addUser('how are you');
		conv.compact('User greeted assistant');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('user');
		expect(msgs[0].content).toContain('[Conversation summary]');
		expect(msgs[0].content).toContain('User greeted assistant');
	});

	it('estimatedChars tracks character count', () => {
		const conv = createConversation({ systemPrompt: '1234567890' });
		expect(conv.estimatedChars).toBe(10);
		conv.addUser('hello'); // 5 chars
		expect(conv.estimatedChars).toBe(15);
	});

	it('needsCompaction is true when above threshold', () => {
		const conv = createConversation({ autoCompactChars: 10 });
		expect(conv.needsCompaction).toBe(false);
		conv.addUser('this is a long message that exceeds the threshold');
		expect(conv.needsCompaction).toBe(true);
	});

	it('trims oldest non-system messages when maxMessages is set', () => {
		const conv = createConversation({ maxMessages: 2 });
		conv.addUser('first');
		conv.addUser('second');
		conv.addUser('third');
		expect(conv.messageCount).toBe(2);
		const msgs = conv.toMessages();
		expect(msgs[0].content).toBe('second');
		expect(msgs[1].content).toBe('third');
	});

	it('does not trim when maxMessages is 0', () => {
		const conv = createConversation({ maxMessages: 0 });
		for (let i = 0; i < 100; i++) {
			conv.addUser(`msg ${i}`);
		}
		expect(conv.messageCount).toBe(100);
	});

	it('returns empty toMessages when no system prompt and no messages', () => {
		const conv = createConversation();
		const msgs = conv.toMessages();
		expect(msgs.length).toBe(0);
	});
});
