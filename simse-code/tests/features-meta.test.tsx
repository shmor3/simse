import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { metaCommands } from '../features/meta/index.js';
import { ContextGrid, HelpView } from '../features/meta/components.js';

describe('meta feature module', () => {
	test('exports an array of command definitions', () => {
		expect(Array.isArray(metaCommands)).toBe(true);
		expect(metaCommands.length).toBeGreaterThan(0);
	});

	test('all commands have category "meta"', () => {
		for (const cmd of metaCommands) {
			expect(cmd.category).toBe('meta');
		}
	});

	test('includes help command with alias', () => {
		const help = metaCommands.find((c) => c.name === 'help');
		expect(help).toBeDefined();
		expect(help!.aliases).toContain('?');
	});

	test('includes clear command', () => {
		expect(metaCommands.find((c) => c.name === 'clear')).toBeDefined();
	});

	test('includes exit command with aliases', () => {
		const exit = metaCommands.find((c) => c.name === 'exit');
		expect(exit).toBeDefined();
		expect(exit!.aliases).toContain('quit');
		expect(exit!.aliases).toContain('q');
	});
});

describe('ContextGrid', () => {
	test('renders percentage', () => {
		const { lastFrame } = render(<ContextGrid usedChars={80000} maxChars={200000} />);
		expect(lastFrame()).toContain('40%');
	});
});

describe('HelpView', () => {
	test('renders command list', () => {
		const { lastFrame } = render(<HelpView commands={metaCommands} />);
		const frame = lastFrame()!;
		expect(frame).toContain('/help');
		expect(frame).toContain('/exit');
	});
});
