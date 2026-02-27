import { describe, expect, test } from 'bun:test';
import { libraryCommands } from '../features/library/index.js';
import { toolsCommands } from '../features/tools/index.js';
import { sessionCommands } from '../features/session/index.js';
import { filesCommands } from '../features/files/index.js';
import { configCommands } from '../features/config/index.js';
import { aiCommands } from '../features/ai/index.js';

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
			...libraryCommands, ...toolsCommands, ...sessionCommands,
			...filesCommands, ...configCommands, ...aiCommands,
		].map((c) => c.name);
		const unique = new Set(allNames);
		expect(unique.size).toBe(allNames.length);
	});
});
