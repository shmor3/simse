import { describe, expect, it } from 'bun:test';
import { createConversation } from '../conversation.js';

// ---------------------------------------------------------------------------
// createConversation
// ---------------------------------------------------------------------------

describe('createConversation', () => {
	it('should return a frozen object', () => {
		const conv = createConversation();
		expect(Object.isFrozen(conv)).toBe(true);
	});

	it('should start with zero messages', () => {
		const conv = createConversation();
		expect(conv.messageCount).toBe(0);
	});

	// -- Adding messages -------------------------------------------------------

	it('should add user messages', () => {
		const conv = createConversation();
		conv.addUser('Hello');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('user');
		expect(msgs[0].content).toBe('Hello');
	});

	it('should add assistant messages', () => {
		const conv = createConversation();
		conv.addAssistant('Response text');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('assistant');
		expect(msgs[0].content).toBe('Response text');
	});

	it('should add tool results with metadata', () => {
		const conv = createConversation();
		conv.addToolResult('call_1', 'library_search', '3 results found');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('tool_result');
		expect(msgs[0].content).toBe('3 results found');
		expect(msgs[0].toolCallId).toBe('call_1');
		expect(msgs[0].toolName).toBe('library_search');
	});

	it('should freeze individual messages', () => {
		const conv = createConversation();
		conv.addUser('Hello');
		const msgs = conv.toMessages();
		expect(Object.isFrozen(msgs[0])).toBe(true);
	});

	it('should maintain message order', () => {
		const conv = createConversation();
		conv.addUser('Q1');
		conv.addAssistant('A1');
		conv.addUser('Q2');
		const msgs = conv.toMessages();
		expect(msgs[0].content).toBe('Q1');
		expect(msgs[1].content).toBe('A1');
		expect(msgs[2].content).toBe('Q2');
	});

	// -- System prompt ---------------------------------------------------------

	it('should include system prompt via options', () => {
		const conv = createConversation({ systemPrompt: 'You are helpful.' });
		const msgs = conv.toMessages();
		expect(msgs).toHaveLength(1);
		expect(msgs[0].role).toBe('system');
		expect(msgs[0].content).toBe('You are helpful.');
	});

	it('should set system prompt via setSystemPrompt', () => {
		const conv = createConversation();
		conv.setSystemPrompt('Be concise.');
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('system');
		expect(msgs[0].content).toBe('Be concise.');
	});

	it('should replace system prompt on subsequent setSystemPrompt calls', () => {
		const conv = createConversation({ systemPrompt: 'old' });
		conv.setSystemPrompt('new');
		const msgs = conv.toMessages();
		const systemMsgs = msgs.filter((m) => m.role === 'system');
		expect(systemMsgs).toHaveLength(1);
		expect(systemMsgs[0].content).toBe('new');
	});

	it('should place system prompt before user messages in toMessages', () => {
		const conv = createConversation();
		conv.addUser('Hello');
		conv.setSystemPrompt('System text');
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('system');
		expect(msgs[1].role).toBe('user');
	});

	// -- Serialization ---------------------------------------------------------

	it('should serialize messages with role prefixes', () => {
		const conv = createConversation();
		conv.addUser('Hello');
		conv.addAssistant('Hi there');
		const serialized = conv.serialize();
		expect(serialized).toContain('[User]\nHello');
		expect(serialized).toContain('[Assistant]\nHi there');
	});

	it('should serialize system prompt', () => {
		const conv = createConversation({ systemPrompt: 'Be concise.' });
		const serialized = conv.serialize();
		expect(serialized).toContain('[System]\nBe concise.');
	});

	it('should serialize tool results with name', () => {
		const conv = createConversation();
		conv.addToolResult('call_1', 'search', 'result');
		const serialized = conv.serialize();
		expect(serialized).toContain('[Tool Result: search]');
		expect(serialized).toContain('result');
	});

	it('should separate messages with double newlines', () => {
		const conv = createConversation();
		conv.addUser('A');
		conv.addAssistant('B');
		const serialized = conv.serialize();
		expect(serialized).toContain('\n\n');
	});

	// -- Clear -----------------------------------------------------------------

	it('should clear all messages', () => {
		const conv = createConversation();
		conv.addUser('A');
		conv.addAssistant('B');
		conv.clear();
		expect(conv.messageCount).toBe(0);
	});

	it('should preserve system prompt after clear', () => {
		const conv = createConversation({ systemPrompt: 'Keep me.' });
		conv.addUser('Hello');
		conv.clear();
		const msgs = conv.toMessages();
		// System prompt is still there via setSystemPrompt (options-based)
		expect(msgs[0].role).toBe('system');
		expect(msgs[0].content).toBe('Keep me.');
		// But user messages are gone
		expect(conv.messageCount).toBe(0);
	});

	// -- Compact ---------------------------------------------------------------

	it('should compact conversation into a summary message', () => {
		const conv = createConversation();
		conv.addUser('Q1');
		conv.addAssistant('A1');
		conv.addUser('Q2');
		conv.addAssistant('A2');
		conv.compact('Summary of conversation so far.');
		expect(conv.messageCount).toBe(1);
		const msgs = conv.toMessages();
		// System prompt + compacted summary
		const userMsgs = msgs.filter((m) => m.role === 'user');
		expect(userMsgs).toHaveLength(1);
		expect(userMsgs[0].content).toContain('Conversation summary');
		expect(userMsgs[0].content).toContain('Summary of conversation so far.');
	});

	// -- estimatedChars --------------------------------------------------------

	it('should estimate chars from all messages', () => {
		const conv = createConversation();
		conv.addUser('Hello'); // 5 chars
		conv.addAssistant('World'); // 5 chars
		expect(conv.estimatedChars).toBe(10);
	});

	it('should include system prompt in char estimate', () => {
		const conv = createConversation({ systemPrompt: 'System' }); // 6 chars
		conv.addUser('Hi'); // 2 chars
		expect(conv.estimatedChars).toBe(8);
	});

	it('should return 0 for empty conversation', () => {
		const conv = createConversation();
		expect(conv.estimatedChars).toBe(0);
	});

	// -- needsCompaction -------------------------------------------------------

	it('should return false when under threshold', () => {
		const conv = createConversation({ autoCompactChars: 1000 });
		conv.addUser('short');
		expect(conv.needsCompaction).toBe(false);
	});

	it('should return true when over threshold', () => {
		const conv = createConversation({ autoCompactChars: 10 });
		conv.addUser('This message is definitely longer than 10 chars');
		expect(conv.needsCompaction).toBe(true);
	});

	it('should use default threshold of 100_000', () => {
		const conv = createConversation();
		conv.addUser('short');
		expect(conv.needsCompaction).toBe(false);
	});

	// -- maxMessages trimming --------------------------------------------------

	it('should trim oldest non-system messages when exceeding maxMessages', () => {
		const conv = createConversation({ maxMessages: 2 });
		conv.addUser('First');
		conv.addAssistant('Reply 1');
		conv.addUser('Second'); // This should trim "First"
		expect(conv.messageCount).toBe(2);
		const msgs = conv.toMessages();
		const contents = msgs.map((m) => m.content);
		expect(contents).not.toContain('First');
		expect(contents).toContain('Reply 1');
		expect(contents).toContain('Second');
	});

	it('should not trim when maxMessages is 0 (unlimited)', () => {
		const conv = createConversation({ maxMessages: 0 });
		for (let i = 0; i < 100; i++) {
			conv.addUser(`msg ${i}`);
		}
		expect(conv.messageCount).toBe(100);
	});

	// -- toMessages returns frozen array ---------------------------------------

	it('should return a frozen array from toMessages', () => {
		const conv = createConversation();
		conv.addUser('test');
		const msgs = conv.toMessages();
		expect(Object.isFrozen(msgs)).toBe(true);
	});
});
