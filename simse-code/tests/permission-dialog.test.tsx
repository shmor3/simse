import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { PermissionDialog } from '../components/input/permission-dialog.js';

describe('PermissionDialog', () => {
	test('renders tool name and args', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="vfs_write"
				args={{ path: '/src/main.ts' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_write');
		expect(frame).toContain('/src/main.ts');
	});

	test('shows action labels', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="vfs_write"
				args={{}}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Allow');
		expect(frame).toContain('Deny');
	});
});
