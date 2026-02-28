import { describe, expect, test } from 'bun:test';
import { aiCommands } from '../features/ai/index.js';
import { configCommands } from '../features/config/index.js';
import { filesCommands } from '../features/files/index.js';
import { libraryCommands } from '../features/library/index.js';
import type { SessionCommandContext } from '../features/session/index.js';
import { createSessionCommands } from '../features/session/index.js';
import { createToolsCommands } from '../features/tools/index.js';

/** Minimal mock context for session commands. */
function mockSessionCtx(): SessionCommandContext {
	return {
		sessionStore: {
			create: () => 'test-id',
			append: () => {},
			load: () => [],
			list: () => [],
			get: () => undefined,
			rename: () => {},
			remove: () => {},
			latest: () => undefined,
		},
		getSessionId: () => 'test-id',
		getServerName: () => undefined,
		getModelName: () => undefined,
		resumeSession: () => {},
	};
}

/** Minimal mock tool registry for tools commands. */
function mockToolRegistry() {
	return {
		discover: async () => {},
		getToolDefinitions: () => [],
		formatForSystemPrompt: () => '',
		execute: async () => ({ id: '', name: '', output: '', isError: false }),
		toolCount: 0,
	};
}

const sessionCommands = createSessionCommands(mockSessionCtx());
const toolsCommands = createToolsCommands({
	getToolRegistry: () => mockToolRegistry(),
});

describe('all feature modules', () => {
	test('library module exports commands with correct category', () => {
		expect(libraryCommands.length).toBeGreaterThan(0);
		for (const cmd of libraryCommands) expect(cmd.category).toBe('library');
	});

	test('tools module exports commands with correct category', () => {
		expect(toolsCommands.length).toBeGreaterThan(0);
		for (const cmd of toolsCommands) expect(cmd.category).toBe('tools');
	});

	test('session module exports commands with correct category', () => {
		expect(sessionCommands.length).toBeGreaterThan(0);
		for (const cmd of sessionCommands) expect(cmd.category).toBe('session');
	});

	test('files module exports commands with correct category', () => {
		expect(filesCommands.length).toBeGreaterThan(0);
		for (const cmd of filesCommands) expect(cmd.category).toBe('files');
	});

	test('config module exports commands with correct category', () => {
		expect(configCommands.length).toBeGreaterThan(0);
		for (const cmd of configCommands) expect(cmd.category).toBe('config');
	});

	test('ai module exports commands with correct category', () => {
		expect(aiCommands.length).toBeGreaterThan(0);
		for (const cmd of aiCommands) expect(cmd.category).toBe('ai');
	});

	test('no duplicate command names across modules', () => {
		const allNames = [
			...libraryCommands,
			...toolsCommands,
			...sessionCommands,
			...filesCommands,
			...configCommands,
			...aiCommands,
		].map((c) => c.name);
		const unique = new Set(allNames);
		expect(unique.size).toBe(allNames.length);
	});
});
