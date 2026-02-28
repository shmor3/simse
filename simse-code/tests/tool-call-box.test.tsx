import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import React from 'react';
import { ToolCallBox } from '../components/chat/tool-call-box.js';

describe('ToolCallBox', () => {
	test('active tool call shows display name + primary arg, no border chars', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/main.ts"}'
				status="active"
			/>,
		);
		const frame = lastFrame()!;
		// Should show mapped display name, not raw tool name
		expect(frame).toContain('Read');
		expect(frame).toContain('/src/main.ts');
		// No border characters
		expect(frame).not.toContain('╭');
		expect(frame).not.toContain('╰');
		expect(frame).not.toContain('╮');
		expect(frame).not.toContain('╯');
	});

	test('completed tool call shows display name + arg + summary + formatted duration', () => {
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
		expect(frame).toContain('Read');
		expect(frame).toContain('/src/main.ts');
		expect(frame).toContain('150 lines');
		expect(frame).toContain('125ms');
		// Result line uses tree connector
		expect(frame).toContain('⎿');
		// Completed status shows magenta dot (⏺)
		expect(frame).toContain('⏺');
	});

	test('failed tool call shows error with tree connector', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_write"
				args='{"path": "/src/main.ts"}'
				status="failed"
				error="Permission denied"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Write');
		expect(frame).toContain('Permission denied');
		// Error uses tree connector
		expect(frame).toContain('⎿');
		// Failed status shows red dot (⏺)
		expect(frame).toContain('⏺');
		// No border chars
		expect(frame).not.toContain('╭');
		expect(frame).not.toContain('╰');
	});

	test('maps tool names to display names', () => {
		const cases: Array<[string, string]> = [
			['vfs_read', 'Read'],
			['file_edit', 'Update'],
			['bash', 'Bash'],
			['shell', 'Bash'],
			['exec', 'Bash'],
			['glob', 'Search'],
			['grep', 'Search'],
			['vfs_write', 'Write'],
			['library_search', 'Search'],
			['task_create', 'TaskCreate'],
		];

		for (const [toolName, expectedDisplay] of cases) {
			const { lastFrame } = render(
				<ToolCallBox name={toolName} args="{}" status="completed" />,
			);
			const frame = lastFrame()!;
			expect(frame).toContain(expectedDisplay);
		}
	});

	test('extracts primary arg from common keys', () => {
		// path
		const { lastFrame: f1 } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/lib.ts"}'
				status="completed"
			/>,
		);
		expect(f1()!).toContain('Read(/src/lib.ts)');

		// file_path
		const { lastFrame: f2 } = render(
			<ToolCallBox
				name="file_edit"
				args='{"file_path": "/src/app.ts", "content": "hello"}'
				status="completed"
			/>,
		);
		expect(f2()!).toContain('Update(/src/app.ts)');

		// command
		const { lastFrame: f3 } = render(
			<ToolCallBox
				name="bash"
				args='{"command": "ls -la"}'
				status="completed"
			/>,
		);
		expect(f3()!).toContain('Bash(ls -la)');

		// query
		const { lastFrame: f4 } = render(
			<ToolCallBox
				name="library_search"
				args='{"query": "find something"}'
				status="completed"
			/>,
		);
		expect(f4()!).toContain('Search(find something)');
	});

	test('formats duration correctly', () => {
		// < 1000ms → Xms
		const { lastFrame: f1 } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "a.ts"}'
				status="completed"
				duration={42}
				summary="10 lines"
			/>,
		);
		expect(f1()!).toContain('42ms');

		// >= 1000ms < 60s → X.Xs
		const { lastFrame: f2 } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "b.ts"}'
				status="completed"
				duration={2500}
				summary="50 lines"
			/>,
		);
		expect(f2()!).toContain('2.5s');

		// >= 60s → XmYs
		const { lastFrame: f3 } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "c.ts"}'
				status="completed"
				duration={125000}
				summary="1000 lines"
			/>,
		);
		expect(f3()!).toContain('2m5s');
	});

	test('diff lines show with tree connector, + green / - red', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="file_edit"
				args='{"file_path": "/src/app.ts"}'
				status="completed"
				diff={'+added line\n-removed line'}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('⎿');
		expect(frame).toContain('+added line');
		expect(frame).toContain('-removed line');
	});

	test('shows tool name without arg when args is empty JSON', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="task_list"
				args="{}"
				status="completed"
				summary="3 tasks"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('TaskList');
		expect(frame).toContain('3 tasks');
	});

	test('unknown tool name gets capitalized', () => {
		const { lastFrame } = render(
			<ToolCallBox name="custom_tool" args="{}" status="completed" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Custom_tool');
	});
});
