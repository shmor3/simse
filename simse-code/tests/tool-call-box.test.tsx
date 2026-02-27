import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { ToolCallBox } from '../components/chat/tool-call-box.js';

describe('ToolCallBox', () => {
	test('renders active tool call', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/main.ts"}'
				status="active"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_read');
		expect(frame).toContain('/src/main.ts');
	});

	test('renders completed tool call with duration', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/main.ts"}'
				status="completed"
				duration={125}
				summary="150 lines"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_read');
		expect(frame).toContain('125ms');
		expect(frame).toContain('150 lines');
	});

	test('renders failed tool call with error', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_write"
				args='{"path": "/src/main.ts"}'
				status="failed"
				error="Permission denied"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_write');
		expect(frame).toContain('Permission denied');
	});
});
