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

// ---------------------------------------------------------------------------
// estimatedTokens
// ---------------------------------------------------------------------------

describe('conversation estimatedTokens', () => {
	it('returns estimatedChars / 4 by default', () => {
		const conv = createConversation();
		conv.addUser('hello world'); // 11 chars
		// Math.ceil(11 / 4) = 3
		expect(conv.estimatedTokens).toBe(Math.ceil(conv.estimatedChars / 4));
	});

	it('includes system prompt in default estimate', () => {
		const conv = createConversation({ systemPrompt: 'You are a helper' }); // 16 chars
		conv.addUser('hi'); // 2 chars
		// Total: 18 chars, Math.ceil(18 / 4) = 5
		expect(conv.estimatedTokens).toBe(Math.ceil(18 / 4));
	});

	it('uses custom tokenEstimator when provided', () => {
		// Custom estimator: count words
		const conv = createConversation({
			tokenEstimator: (text) => text.split(/\s+/).filter(Boolean).length,
		});
		conv.addUser('hello world foo bar'); // 4 words

		expect(conv.estimatedTokens).toBe(4);
	});

	it('custom tokenEstimator includes system prompt', () => {
		const conv = createConversation({
			systemPrompt: 'be helpful',
			tokenEstimator: (text) => text.split(/\s+/).filter(Boolean).length,
		});
		conv.addUser('hello world'); // 2 words + 2 system words = 4

		expect(conv.estimatedTokens).toBe(4);
	});

	it('estimatedTokens updates after adding messages', () => {
		const conv = createConversation();
		const tokensBefore = conv.estimatedTokens;
		conv.addUser('a'.repeat(100)); // 100 chars
		expect(conv.estimatedTokens).toBeGreaterThan(tokensBefore);
	});

	it('estimatedTokens updates after clear', () => {
		const conv = createConversation();
		conv.addUser('a'.repeat(100));
		expect(conv.estimatedTokens).toBeGreaterThan(0);
		conv.clear();
		expect(conv.estimatedTokens).toBe(0);
	});
});

// ---------------------------------------------------------------------------
// contextUsagePercent
// ---------------------------------------------------------------------------

describe('conversation contextUsagePercent', () => {
	it('returns 0 when contextWindowTokens is not configured', () => {
		const conv = createConversation();
		conv.addUser('hello');
		expect(conv.contextUsagePercent).toBe(0);
	});

	it('returns percentage based on estimatedTokens and contextWindowTokens', () => {
		const conv = createConversation({ contextWindowTokens: 100 });
		// 400 chars = ~100 tokens at default estimator
		conv.addUser('x'.repeat(400));
		expect(conv.contextUsagePercent).toBe(100);
	});

	it('caps at 100 percent', () => {
		const conv = createConversation({ contextWindowTokens: 10 });
		conv.addUser('x'.repeat(1000));
		expect(conv.contextUsagePercent).toBe(100);
	});

	it('tracks partial usage', () => {
		const conv = createConversation({ contextWindowTokens: 1000 });
		// 100 chars = ~25 tokens => 25/1000 = 3%
		conv.addUser('x'.repeat(100));
		expect(conv.contextUsagePercent).toBe(3);
	});

	it('uses custom tokenEstimator for percentage', () => {
		const conv = createConversation({
			contextWindowTokens: 100,
			tokenEstimator: (text) => text.length, // 1 char = 1 token
		});
		conv.addUser('x'.repeat(50));
		expect(conv.contextUsagePercent).toBe(50);
	});
});
