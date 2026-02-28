import { describe, expect, test } from 'bun:test';
import { createCommandRegistry } from '../command-registry.js';
import type { CommandDefinition } from '../ink-types.js';

describe('createCommandRegistry', () => {
	const testCommand: CommandDefinition = {
		name: 'test',
		aliases: ['t'],
		usage: '/test <arg>',
		description: 'A test command',
		category: 'meta',
		execute: (args) => ({ text: `executed: ${args}` }),
	};

	test('registers and looks up commands by name', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		expect(registry.get('test')).toBe(testCommand);
	});

	test('looks up commands by alias', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		expect(registry.get('t')).toBe(testCommand);
	});

	test('returns undefined for unknown commands', () => {
		const registry = createCommandRegistry();
		expect(registry.get('nonexistent')).toBeUndefined();
	});

	test('lists all commands', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		const all = registry.getAll();
		expect(all).toHaveLength(1);
		expect(all[0]?.name).toBe('test');
	});

	test('lists commands by category', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		registry.register({
			...testCommand,
			name: 'other',
			aliases: [],
			category: 'ai',
		});
		expect(registry.getByCategory('meta')).toHaveLength(1);
		expect(registry.getByCategory('ai')).toHaveLength(1);
	});

	test('registerAll registers multiple commands', () => {
		const registry = createCommandRegistry();
		registry.registerAll([
			testCommand,
			{ ...testCommand, name: 'test2', aliases: [] },
		]);
		expect(registry.getAll()).toHaveLength(2);
	});
});
