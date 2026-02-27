import { describe, expect, it, mock } from 'bun:test';
import { createToolPermissionResolver } from '../src/ai/tools/permissions.js';
import type { ToolCallRequest } from '../src/ai/tools/types.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeRequest(
	name: string,
	args: Record<string, unknown> = {},
): ToolCallRequest {
	return { id: 'call-1', name, arguments: args };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createToolPermissionResolver', () => {
	it('allows by default with allow policy', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [],
		});
		const result = await resolver.check(makeRequest('any_tool'));
		expect(result).toBe(true);
	});

	it('denies by default with deny policy', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [],
		});
		const result = await resolver.check(makeRequest('any_tool'));
		expect(result).toBe(false);
	});

	it('matches tool name glob pattern', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'fs_*', policy: 'allow' }],
		});

		expect(await resolver.check(makeRequest('fs_read'))).toBe(true);
		expect(await resolver.check(makeRequest('fs_write'))).toBe(true);
		expect(await resolver.check(makeRequest('library_search'))).toBe(false);
	});

	it('matches exact tool name', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'library_search', policy: 'allow' }],
		});

		expect(await resolver.check(makeRequest('library_search'))).toBe(true);
		expect(await resolver.check(makeRequest('library_shelve'))).toBe(false);
	});

	it('matches bash command pattern', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'bash', pattern: 'git *', policy: 'allow' }],
		});

		expect(
			await resolver.check(makeRequest('bash', { command: 'git status' })),
		).toBe(true);
		expect(
			await resolver.check(
				makeRequest('bash', { command: 'git push origin main' }),
			),
		).toBe(true);
		expect(
			await resolver.check(makeRequest('bash', { command: 'rm -rf /' })),
		).toBe(false);
	});

	it('skips command pattern rule when no command argument', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'bash', pattern: 'git *', policy: 'allow' }],
		});

		// No command argument at all — rule should not match
		expect(await resolver.check(makeRequest('bash', {}))).toBe(false);
	});

	it('last matching rule wins', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [
				{ tool: 'fs_*', policy: 'allow' },
				{ tool: 'fs_delete', policy: 'deny' },
			],
		});

		expect(await resolver.check(makeRequest('fs_read'))).toBe(true);
		expect(await resolver.check(makeRequest('fs_delete'))).toBe(false);
	});

	it('last matching rule wins with multiple overlapping patterns', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [
				{ tool: '*', policy: 'deny' },
				{ tool: 'safe_*', policy: 'allow' },
			],
		});

		expect(await resolver.check(makeRequest('safe_read'))).toBe(true);
		expect(await resolver.check(makeRequest('dangerous_exec'))).toBe(false);
	});

	it('ask policy calls onPermissionRequest callback', async () => {
		const onPermissionRequest = mock(async () => true);
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'exec_*', policy: 'ask' }],
			onPermissionRequest,
		});

		const request = makeRequest('exec_shell');
		const result = await resolver.check(request);

		expect(result).toBe(true);
		expect(onPermissionRequest).toHaveBeenCalledTimes(1);
		expect(onPermissionRequest).toHaveBeenCalledWith(request);
	});

	it('ask policy returns false when callback denies', async () => {
		const onPermissionRequest = mock(async () => false);
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'exec_*', policy: 'ask' }],
			onPermissionRequest,
		});

		const result = await resolver.check(makeRequest('exec_shell'));
		expect(result).toBe(false);
	});

	it('ask policy defaults to false when no callback provided', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'exec_*', policy: 'ask' }],
		});

		const result = await resolver.check(makeRequest('exec_shell'));
		expect(result).toBe(false);
	});

	it('matches single-character wildcard with ?', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'fs_?', policy: 'allow' }],
		});

		expect(await resolver.check(makeRequest('fs_r'))).toBe(true);
		expect(await resolver.check(makeRequest('fs_read'))).toBe(false);
	});

	it('escapes special regex characters in glob', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: 'tool.name', policy: 'allow' }],
		});

		expect(await resolver.check(makeRequest('tool.name'))).toBe(true);
		expect(await resolver.check(makeRequest('toolXname'))).toBe(false);
	});
});

// ---------------------------------------------------------------------------
// Category-based permission rules
// ---------------------------------------------------------------------------

describe('category-based permission rules', () => {
	it('allows all tools in a category', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: '*', category: 'read', policy: 'allow' }],
		});

		const readDef = {
			name: 'fs_read',
			description: '',
			parameters: {},
			category: 'read' as const,
		};
		const editDef = {
			name: 'fs_write',
			description: '',
			parameters: {},
			category: 'edit' as const,
		};

		expect(await resolver.check(makeRequest('fs_read'), readDef)).toBe(true);
		expect(await resolver.check(makeRequest('fs_write'), editDef)).toBe(false);
	});

	it('blocks destructive tools via annotation', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [
				{ tool: '*', annotations: { destructive: true }, policy: 'deny' },
			],
		});

		const safeDef = {
			name: 'fs_read',
			description: '',
			parameters: {},
			annotations: { readOnly: true },
		};
		const dangerDef = {
			name: 'fs_delete',
			description: '',
			parameters: {},
			annotations: { destructive: true },
		};

		expect(await resolver.check(makeRequest('fs_read'), safeDef)).toBe(true);
		expect(await resolver.check(makeRequest('fs_delete'), dangerDef)).toBe(
			false,
		);
	});

	it('works without definition (backwards compatible)', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [{ tool: 'bash', policy: 'deny' }],
		});

		expect(await resolver.check(makeRequest('bash'))).toBe(false);
		expect(await resolver.check(makeRequest('fs_read'))).toBe(true);
	});

	it('category rule skipped when definition has no category', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: '*', category: 'read', policy: 'allow' }],
		});

		const noCatDef = {
			name: 'some_tool',
			description: '',
			parameters: {},
		};

		expect(await resolver.check(makeRequest('some_tool'), noCatDef)).toBe(
			false,
		);
	});

	it('annotation rule skipped when definition has no annotations', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [
				{ tool: '*', annotations: { destructive: true }, policy: 'deny' },
			],
		});

		const noAnnotDef = {
			name: 'safe_tool',
			description: '',
			parameters: {},
		};

		expect(await resolver.check(makeRequest('safe_tool'), noAnnotDef)).toBe(
			true,
		);
	});
});
