import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createContextPruner } from '../src/ai/conversation/context-pruner.js';
import type { ConversationMessage } from '../src/ai/conversation/types.js';
import { discoverInstructions } from '../src/ai/prompts/instruction-discovery.js';
import { createProviderPromptResolver } from '../src/ai/prompts/provider-prompts.js';
import { registerBashTool } from '../src/ai/tools/host/bash.js';
import { registerFilesystemTools } from '../src/ai/tools/host/filesystem.js';
import { fuzzyMatch } from '../src/ai/tools/host/fuzzy-edit.js';
import {
	createToolPermissionResolver,
	type ToolPermissionConfig,
} from '../src/ai/tools/permissions.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { createEventBus } from '../src/events/event-bus.js';
import type { EventType } from '../src/events/types.js';
import { createHookSystem } from '../src/hooks/hook-system.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

let tempDir: string;

beforeEach(async () => {
	tempDir = await mkdtemp(join(tmpdir(), 'simse-integration-'));
});

afterEach(async () => {
	await rm(tempDir, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// 1. Event bus + tool registry + hooks work together
// ---------------------------------------------------------------------------

describe('event bus + tool registry + hooks', () => {
	it('publishes events via hook on tool execution', async () => {
		const bus = createEventBus();
		const hooks = createHookSystem();
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registerFilesystemTools(registry, { workingDirectory: tempDir });

		const receivedEvents: { type: EventType; payload: unknown }[] = [];
		bus.subscribeAll((type, payload) => {
			receivedEvents.push({ type, payload });
		});

		// Hook into tool.execute.before to publish an event via the bus
		hooks.register('tool.execute.before', async (ctx) => {
			bus.publish('tool.call.start', {
				callId: ctx.request.id,
				name: ctx.request.name,
				args: ctx.request.arguments,
			});
			return ctx.request;
		});

		// Write a file via the registry
		await writeFile(join(tempDir, 'test.txt'), 'hello', 'utf-8');

		// Simulate the hook running before execution
		const request = {
			id: 'call-1',
			name: 'fs_read',
			arguments: { path: 'test.txt' },
		};
		await hooks.run('tool.execute.before', { request });

		// Now execute the tool
		const result = await registry.execute(request);

		expect(result.isError).toBe(false);
		expect(result.output).toContain('hello');
		expect(receivedEvents).toHaveLength(1);
		expect(receivedEvents[0].type).toBe('tool.call.start');
		expect((receivedEvents[0].payload as { name: string }).name).toBe(
			'fs_read',
		);
	});
});

// ---------------------------------------------------------------------------
// 2. Permission resolver gates tool execution
// ---------------------------------------------------------------------------

describe('permission resolver gates tool execution', () => {
	it('allows and denies based on rules', async () => {
		const config: ToolPermissionConfig = {
			defaultPolicy: 'deny',
			rules: [
				{ tool: 'fs_read', policy: 'allow' },
				{ tool: 'fs_write', policy: 'deny' },
			],
		};
		const resolver = createToolPermissionResolver(config);

		const readResult = await resolver.check({
			id: 'c1',
			name: 'fs_read',
			arguments: { path: 'test.txt' },
		});
		expect(readResult).toBe(true);

		const writeResult = await resolver.check({
			id: 'c2',
			name: 'fs_write',
			arguments: { path: 'test.txt', content: 'x' },
		});
		expect(writeResult).toBe(false);

		// Unlisted tool falls back to default deny
		const unknownResult = await resolver.check({
			id: 'c3',
			name: 'bash',
			arguments: { command: 'ls' },
		});
		expect(unknownResult).toBe(false);
	});
});

// ---------------------------------------------------------------------------
// 3. Batch execution runs tools concurrently
// ---------------------------------------------------------------------------

describe('batch execution', () => {
	it('runs multiple tool calls concurrently and returns all results', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registerFilesystemTools(registry, { workingDirectory: tempDir });

		// Write two files
		await writeFile(join(tempDir, 'a.txt'), 'content-A', 'utf-8');
		await writeFile(join(tempDir, 'b.txt'), 'content-B', 'utf-8');

		const results = await registry.batchExecute([
			{ id: 'r1', name: 'fs_read', arguments: { path: 'a.txt' } },
			{ id: 'r2', name: 'fs_read', arguments: { path: 'b.txt' } },
		]);

		expect(results).toHaveLength(2);
		expect(results[0].isError).toBe(false);
		expect(results[0].output).toContain('content-A');
		expect(results[1].isError).toBe(false);
		expect(results[1].output).toContain('content-B');
	});
});

// ---------------------------------------------------------------------------
// 4. Context pruner reduces conversation size
// ---------------------------------------------------------------------------

describe('context pruner', () => {
	it('prunes old tool results while protecting recent turns', () => {
		const pruner = createContextPruner({ protectRecentTurns: 1 });

		const messages: ConversationMessage[] = [];

		// Build 10 turns with large tool_result content
		for (let i = 0; i < 10; i++) {
			messages.push({
				role: 'user',
				content: `Turn ${i} question`,
			});
			messages.push({
				role: 'assistant',
				content: `Turn ${i} response`,
			});
			messages.push({
				role: 'tool_result',
				content: 'X'.repeat(500), // >200 chars to trigger pruning
				toolCallId: `call-${i}`,
				toolName: 'fs_read',
			});
		}

		const originalSize = messages.reduce((s, m) => s + m.content.length, 0);
		const pruned = pruner.prune(messages);
		const prunedSize = pruned.reduce((s, m) => s + m.content.length, 0);

		// Pruned output should be smaller
		expect(prunedSize).toBeLessThan(originalSize);

		// The pruned messages should contain marker text
		const prunedMarkers = pruned.filter((m) =>
			m.content.includes('[OUTPUT PRUNED'),
		);
		expect(prunedMarkers.length).toBeGreaterThan(0);
	});
});

// ---------------------------------------------------------------------------
// 5. Provider prompts resolve correctly
// ---------------------------------------------------------------------------

describe('provider prompt resolver', () => {
	it('matches glob patterns and falls back to default', () => {
		const resolver = createProviderPromptResolver({
			prompts: {
				'anthropic/*': 'You are using an Anthropic model.',
				'openai/*': 'You are using an OpenAI model.',
			},
			defaultPrompt: 'Generic model prompt.',
		});

		expect(resolver.resolve('anthropic/claude-3')).toBe(
			'You are using an Anthropic model.',
		);
		expect(resolver.resolve('openai/gpt-4')).toBe(
			'You are using an OpenAI model.',
		);
		expect(resolver.resolve('google/gemini-pro')).toBe('Generic model prompt.');
	});
});

// ---------------------------------------------------------------------------
// 6. Instruction discovery finds project files
// ---------------------------------------------------------------------------

describe('instruction discovery', () => {
	it('discovers CLAUDE.md in a project directory', async () => {
		await writeFile(
			join(tempDir, 'CLAUDE.md'),
			'# Instructions\nDo great things.',
			'utf-8',
		);

		const results = await discoverInstructions({
			rootDir: tempDir,
			patterns: ['CLAUDE.md'],
		});

		expect(results).toHaveLength(1);
		expect(results[0].content).toContain('Do great things');
	});
});

// ---------------------------------------------------------------------------
// 7. Fuzzy edit handles real-world code edits
// ---------------------------------------------------------------------------

describe('fuzzy edit', () => {
	it('applies an edit via exact match on realistic code', () => {
		const code = [
			'function add(a: number, b: number): number {',
			'  return a + b;',
			'}',
			'',
			'export default add;',
		].join('\n');

		const result = fuzzyMatch(code, '  return a + b;', '  return a + b + 1;');

		expect(result).not.toBeNull();
		expect(result!.replaced).toContain('return a + b + 1');
		expect(result!.replaced).not.toContain('  return a + b;\n');
		expect(result!.strategy).toBe('exact');
	});
});

// ---------------------------------------------------------------------------
// 8. Bash tool runs in working directory
// ---------------------------------------------------------------------------

describe('bash tool with working directory', () => {
	it('reads a marker file via cat', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registerBashTool(registry, { workingDirectory: tempDir });

		// Write a marker file directly
		await writeFile(join(tempDir, 'marker.txt'), 'INTEGRATION_OK', 'utf-8');

		const result = await registry.execute({
			id: 'bash-1',
			name: 'bash',
			arguments: { command: 'cat marker.txt' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('INTEGRATION_OK');
	});
});

// ---------------------------------------------------------------------------
// 9. End-to-end: write, edit, read through tools
// ---------------------------------------------------------------------------

describe('end-to-end: write, edit, read', () => {
	it('writes a file, edits it, and reads back the edited content', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registerFilesystemTools(registry, { workingDirectory: tempDir });

		// Step 1: Write a file
		const writeResult = await registry.execute({
			id: 'w1',
			name: 'fs_write',
			arguments: {
				path: 'config.json',
				content: '{\n  "version": "1.0.0",\n  "debug": false\n}',
			},
		});
		expect(writeResult.isError).toBe(false);

		// Step 2: Edit the file — change version
		const editResult = await registry.execute({
			id: 'e1',
			name: 'fs_edit',
			arguments: {
				path: 'config.json',
				old_string: '"version": "1.0.0"',
				new_string: '"version": "2.0.0"',
			},
		});
		expect(editResult.isError).toBe(false);

		// Step 3: Read it back and verify the edit took effect
		const readResult = await registry.execute({
			id: 'r1',
			name: 'fs_read',
			arguments: { path: 'config.json' },
		});
		expect(readResult.isError).toBe(false);
		expect(readResult.output).toContain('"version": "2.0.0"');
		expect(readResult.output).not.toContain('"version": "1.0.0"');
		expect(readResult.output).toContain('"debug": false');
	});
});
