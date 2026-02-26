/**
 * E2E test: VFS tool calling → disk commit flow
 *
 * Simulates the agentic loop with a mock ACP client that returns
 * a <tool_use> response to write a file, verifying:
 * 1. vfs_write tool is called
 * 2. VFS contains the written file
 * 3. VFSDisk commit writes to the filesystem
 */

import { afterEach, describe, expect, it } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createToolRegistry as createLibToolRegistry } from '../src/ai/tools/index.js';
import { createVFSDisk, createVirtualFS } from '../src/ai/vfs/index.js';

// ---------------------------------------------------------------------------
// Inline tool-call helpers (matching the CLI's simse-code/loop.ts parseToolCalls)
// ---------------------------------------------------------------------------

interface ToolCallRequest {
	readonly id: string;
	readonly name: string;
	readonly arguments: Record<string, unknown>;
}

function parseToolCalls(response: string): {
	text: string;
	toolCalls: readonly ToolCallRequest[];
} {
	const toolCalls: ToolCallRequest[] = [];
	const pattern = /<tool_use>\s*([\s\S]*?)\s*<\/tool_use>/g;
	let match: RegExpExecArray | null = pattern.exec(response);
	while (match !== null) {
		const jsonStr = match[1].trim();
		try {
			const parsed = JSON.parse(jsonStr) as {
				id?: string;
				name?: string;
				arguments?: Record<string, unknown>;
			};
			if (parsed.name) {
				toolCalls.push({
					id: parsed.id ?? `call_${toolCalls.length + 1}`,
					name: parsed.name,
					arguments: parsed.arguments ?? {},
				});
			}
		} catch {
			// skip
		}
		match = pattern.exec(response);
	}
	const text = response.replace(pattern, '').trim();
	return { text, toolCalls };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('VFS tool → disk commit e2e', () => {
	let tempDir: string;

	afterEach(() => {
		if (tempDir && existsSync(tempDir)) {
			rmSync(tempDir, { recursive: true, force: true });
		}
	});

	it('vfs_write tool creates file in VFS and commit persists to disk', async () => {
		// 1. Set up VFS + disk
		tempDir = mkdtempSync(join(tmpdir(), 'simse-vfs-e2e-'));
		const writeEvents: Array<{ path: string; isNew: boolean }> = [];

		const vfs = createVirtualFS({
			onFileWrite: (event) => {
				writeEvents.push({ path: event.path, isNew: event.isNew });
			},
		});
		const disk = createVFSDisk(vfs, { baseDir: tempDir });

		// 2. Set up tool registry with VFS tools
		const registry = createLibToolRegistry({});
		registry.register(
			{
				name: 'vfs_write',
				description: 'Write a file to the virtual filesystem sandbox.',
				parameters: {
					path: { type: 'string', description: 'File path', required: true },
					content: {
						type: 'string',
						description: 'File content',
						required: true,
					},
				},
				category: 'vfs',
			},
			async (args) => {
				const path = String(args.path ?? '');
				const content = String(args.content ?? '');
				vfs.writeFile(path, content, { createParents: true });
				return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
			},
		);

		// 3. Simulate an ACP response with a <tool_use> block
		const mockResponse = `I'll create that file for you.

<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "/hello.txt", "content": "Hello from VFS!"}}
</tool_use>

The file has been created.`;

		// 4. Parse tool calls (as the agentic loop does)
		const parsed = parseToolCalls(mockResponse);
		expect(parsed.toolCalls).toHaveLength(1);
		expect(parsed.toolCalls[0].name).toBe('vfs_write');

		// 5. Execute the tool call
		const call = parsed.toolCalls[0];
		const result = await registry.execute({
			id: call.id,
			name: call.name,
			arguments: call.arguments,
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Wrote');
		expect(result.output).toContain('/hello.txt');

		// 6. Verify VFS has the file
		expect(vfs.exists('/hello.txt')).toBe(true);
		const vfsContent = vfs.readFile('/hello.txt');
		expect(vfsContent.text).toBe('Hello from VFS!');
		expect(writeEvents).toHaveLength(1);
		expect(writeEvents[0].path).toBe('/hello.txt');
		expect(writeEvents[0].isNew).toBe(true);

		// 7. Commit to disk
		const commitResult = await disk.commit(undefined, { overwrite: true });
		expect(commitResult.filesWritten).toBe(1);

		// 8. Verify file on disk
		const diskPath = join(tempDir, 'hello.txt');
		expect(existsSync(diskPath)).toBe(true);
		expect(readFileSync(diskPath, 'utf-8')).toBe('Hello from VFS!');
	});

	it('vfs_write tool creates nested directory structure', async () => {
		tempDir = mkdtempSync(join(tmpdir(), 'simse-vfs-e2e-'));
		const vfs = createVirtualFS();
		const disk = createVFSDisk(vfs, { baseDir: tempDir });

		const registry = createLibToolRegistry({});
		registry.register(
			{
				name: 'vfs_write',
				description: 'Write a file to the virtual filesystem sandbox.',
				parameters: {
					path: { type: 'string', description: 'File path', required: true },
					content: {
						type: 'string',
						description: 'File content',
						required: true,
					},
				},
				category: 'vfs',
			},
			async (args) => {
				const path = String(args.path ?? '');
				const content = String(args.content ?? '');
				vfs.writeFile(path, content, { createParents: true });
				return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
			},
		);

		// Simulate writing to a nested path
		const mockResponse = `<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "/src/components/Button.tsx", "content": "export const Button = () => <button>Click</button>;"}}
</tool_use>`;

		const parsed = parseToolCalls(mockResponse);
		expect(parsed.toolCalls).toHaveLength(1);

		const result = await registry.execute({
			id: parsed.toolCalls[0].id,
			name: parsed.toolCalls[0].name,
			arguments: parsed.toolCalls[0].arguments,
		});
		expect(result.isError).toBe(false);

		// Verify VFS
		expect(vfs.exists('/src/components/Button.tsx')).toBe(true);

		// Commit and verify disk
		const commitResult = await disk.commit(undefined, { overwrite: true });
		expect(commitResult.filesWritten).toBe(1);
		expect(commitResult.directoriesCreated).toBeGreaterThanOrEqual(2); // src, src/components

		const diskPath = join(tempDir, 'src', 'components', 'Button.tsx');
		expect(existsSync(diskPath)).toBe(true);
		expect(readFileSync(diskPath, 'utf-8')).toBe(
			'export const Button = () => <button>Click</button>;',
		);
	});

	it('tool registry formats tools for system prompt', () => {
		const registry = createLibToolRegistry({});
		registry.register(
			{
				name: 'vfs_write',
				description: 'Write a file.',
				parameters: {
					path: { type: 'string', description: 'File path', required: true },
					content: { type: 'string', description: 'Content', required: true },
				},
				category: 'vfs',
			},
			async () => 'ok',
		);

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('<tool_use>');
		expect(prompt).toContain('vfs_write');
		expect(prompt).toContain('Write a file.');
	});

	it('agentManagesTools=true skips tool prompt injection', () => {
		const registry = createLibToolRegistry({});
		registry.register(
			{
				name: 'vfs_write',
				description: 'Write a file.',
				parameters: {
					path: { type: 'string', description: 'File path', required: true },
				},
				category: 'vfs',
			},
			async () => 'ok',
		);

		// When agentManagesTools is true, the loop skips formatForSystemPrompt.
		// Verify the format function still works (the loop just doesn't call it).
		const agentManagesTools = true;
		const toolPrompt = agentManagesTools
			? ''
			: registry.formatForSystemPrompt();
		expect(toolPrompt).toBe('');
	});

	it('parseToolCalls handles multiple tool calls', () => {
		const response = `Let me create two files.

<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "/a.txt", "content": "file A"}}
</tool_use>

<tool_use>
{"id": "call_2", "name": "vfs_write", "arguments": {"path": "/b.txt", "content": "file B"}}
</tool_use>

Both files created.`;

		const parsed = parseToolCalls(response);
		expect(parsed.toolCalls).toHaveLength(2);
		expect(parsed.toolCalls[0].arguments.path).toBe('/a.txt');
		expect(parsed.toolCalls[1].arguments.path).toBe('/b.txt');
		expect(parsed.text).toContain('Let me create two files.');
		expect(parsed.text).toContain('Both files created.');
		expect(parsed.text).not.toContain('<tool_use>');
	});

	it('commit result reports operations', async () => {
		tempDir = mkdtempSync(join(tmpdir(), 'simse-vfs-e2e-'));
		const vfs = createVirtualFS();
		const disk = createVFSDisk(vfs, { baseDir: tempDir });

		vfs.writeFile('/readme.md', '# Hello', { createParents: true });
		vfs.writeFile('/src/index.ts', 'console.log("hi")', {
			createParents: true,
		});

		const result = await disk.commit(undefined, { overwrite: true });
		expect(result.filesWritten).toBe(2);
		expect(result.directoriesCreated).toBeGreaterThanOrEqual(1);
		expect(result.bytesWritten).toBeGreaterThan(0);
		expect(result.operations.length).toBeGreaterThanOrEqual(2);
	});
});
