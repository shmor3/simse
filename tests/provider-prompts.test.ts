import { describe, expect, it } from 'bun:test';
import { createProviderPromptResolver } from '../src/ai/prompts/provider-prompts.js';

// ---------------------------------------------------------------------------
// createProviderPromptResolver
// ---------------------------------------------------------------------------

describe('createProviderPromptResolver', () => {
	it('matches an exact provider pattern', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'gpt-4o': 'You are GPT-4o.',
				'claude-3-opus': 'You are Claude Opus.',
			},
		});
		expect(resolver.resolve('gpt-4o')).toBe('You are GPT-4o.');
		expect(resolver.resolve('claude-3-opus')).toBe('You are Claude Opus.');
	});

	it('falls back to defaultPrompt when no pattern matches', () => {
		const resolver = createProviderPromptResolver({
			prompts: { 'gpt-4o': 'GPT prompt' },
			defaultPrompt: 'Fallback prompt',
		});
		expect(resolver.resolve('unknown-model')).toBe('Fallback prompt');
	});

	it('returns empty string when no match and no default', () => {
		const resolver = createProviderPromptResolver({
			prompts: { 'gpt-4o': 'GPT prompt' },
		});
		expect(resolver.resolve('unknown-model')).toBe('');
	});

	it('matches wildcard patterns with *', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'gpt-*': 'You are a GPT model.',
				'claude-*': 'You are Claude.',
			},
		});
		expect(resolver.resolve('gpt-4o')).toBe('You are a GPT model.');
		expect(resolver.resolve('gpt-3.5-turbo')).toBe('You are a GPT model.');
		expect(resolver.resolve('claude-3-opus')).toBe('You are Claude.');
		expect(resolver.resolve('claude-3.5-sonnet')).toBe('You are Claude.');
	});

	it('returns first matching pattern when multiple match', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'claude-3-opus': 'Specific Opus prompt',
				'claude-*': 'Generic Claude prompt',
			},
		});
		expect(resolver.resolve('claude-3-opus')).toBe('Specific Opus prompt');
	});

	it('handles empty prompts record', () => {
		const resolver = createProviderPromptResolver({
			prompts: {},
			defaultPrompt: 'Default',
		});
		expect(resolver.resolve('anything')).toBe('Default');
	});

	it('handles no prompts property at all', () => {
		const resolver = createProviderPromptResolver({
			defaultPrompt: 'Only default',
		});
		expect(resolver.resolve('any-model')).toBe('Only default');
	});

	it('handles config with no prompts and no default', () => {
		const resolver = createProviderPromptResolver({});
		expect(resolver.resolve('any-model')).toBe('');
	});

	it('escapes regex metacharacters in patterns', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'model.v1': 'Exact dot match',
			},
		});
		// Should match literal dot, not any character
		expect(resolver.resolve('model.v1')).toBe('Exact dot match');
		expect(resolver.resolve('modelXv1')).toBe('');
	});

	it('supports wildcards with complex suffixes', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'openai/*': 'OpenAI model',
				'anthropic/claude-*': 'Anthropic Claude',
			},
		});
		expect(resolver.resolve('openai/gpt-4o')).toBe('OpenAI model');
		expect(resolver.resolve('anthropic/claude-3-opus')).toBe(
			'Anthropic Claude',
		);
		expect(resolver.resolve('anthropic/gemini')).toBe('');
	});

	it('returns a frozen object', () => {
		const resolver = createProviderPromptResolver({});
		expect(Object.isFrozen(resolver)).toBe(true);
	});
});
