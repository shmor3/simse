import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import React from 'react';
import { PermissionDialog } from '../components/input/permission-dialog.js';

describe('PermissionDialog', () => {
	test('renders warning icon', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'rm -rf node_modules' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('⚠');
	});

	test('renders tool name with primary arg', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'rm -rf node_modules' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Bash');
		expect(frame).toContain('rm -rf node_modules');
	});

	test('renders tool name alone when no recognized primary arg', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="custom_tool"
				args={{ foo: 'bar' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('custom_tool');
	});

	test('does not render bordered box', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'echo hello' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).not.toContain('╭');
	});

	test('shows keyboard shortcuts [y] and [n]', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'echo hello' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('[y]');
		expect(frame).toContain('[n]');
	});

	test('shows [a]lways when onAllowAlways is provided', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'echo hello' }}
				onAllow={() => {}}
				onDeny={() => {}}
				onAllowAlways={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('[a]');
	});

	test('does not show [a]lways when onAllowAlways is not provided', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="Bash"
				args={{ command: 'echo hello' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).not.toContain('[a]');
	});

	test('extracts primary arg from known keys', () => {
		for (const [key, value] of [
			['command', 'ls -la'],
			['path', '/src/main.ts'],
			['file_path', '/etc/hosts'],
			['query', 'SELECT * FROM users'],
			['name', 'my-resource'],
		] as const) {
			const { lastFrame } = render(
				<PermissionDialog
					toolName="SomeTool"
					args={{ [key]: value }}
					onAllow={() => {}}
					onDeny={() => {}}
				/>,
			);
			const frame = lastFrame()!;
			expect(frame).toContain(`SomeTool(${value})`);
		}
	});
});
