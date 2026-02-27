import { describe, expect, it } from 'bun:test';
import type { CommandInfo, TermColors } from '../ui.js';
import {
	createColors,
	createMarkdownRenderer,
	renderAssistantMessage,
	renderBanner,
	renderDetailLine,
	renderError,
	renderHelp,
	renderServiceStatus,
	renderServiceStatusLine,
	renderSkillLoading,
	renderToolCall,
	renderToolResult,
} from '../ui.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Colors disabled — returns raw text (no ANSI). */
function noColors(): TermColors {
	return createColors({ enabled: false });
}

/** Colors enabled — wraps text in ANSI codes. */
function withColors(): TermColors {
	return createColors({ enabled: true });
}

// ---------------------------------------------------------------------------
// createColors
// ---------------------------------------------------------------------------

describe('createColors', () => {
	it('should return a frozen object', () => {
		const c = createColors({ enabled: false });
		expect(Object.isFrozen(c)).toBe(true);
	});

	it('should return identity functions when disabled', () => {
		const c = noColors();
		expect(c.enabled).toBe(false);
		expect(c.bold('test')).toBe('test');
		expect(c.dim('test')).toBe('test');
		expect(c.red('test')).toBe('test');
		expect(c.green('test')).toBe('test');
		expect(c.yellow('test')).toBe('test');
		expect(c.blue('test')).toBe('test');
		expect(c.magenta('test')).toBe('test');
		expect(c.cyan('test')).toBe('test');
		expect(c.gray('test')).toBe('test');
		expect(c.white('test')).toBe('test');
		expect(c.italic('test')).toBe('test');
		expect(c.underline('test')).toBe('test');
	});

	it('should wrap text in ANSI codes when enabled', () => {
		const c = withColors();
		expect(c.enabled).toBe(true);
		expect(c.bold('hi')).toBe('\x1b[1mhi\x1b[0m');
		expect(c.dim('hi')).toBe('\x1b[2mhi\x1b[0m');
		expect(c.red('hi')).toBe('\x1b[31mhi\x1b[0m');
		expect(c.green('hi')).toBe('\x1b[32mhi\x1b[0m');
		expect(c.yellow('hi')).toBe('\x1b[33mhi\x1b[0m');
		expect(c.blue('hi')).toBe('\x1b[34mhi\x1b[0m');
		expect(c.magenta('hi')).toBe('\x1b[35mhi\x1b[0m');
		expect(c.cyan('hi')).toBe('\x1b[36mhi\x1b[0m');
		expect(c.gray('hi')).toBe('\x1b[90mhi\x1b[0m');
		expect(c.white('hi')).toBe('\x1b[37mhi\x1b[0m');
	});

	it('should nest ANSI codes correctly', () => {
		const c = withColors();
		const nested = c.bold(c.cyan('hi'));
		expect(nested).toContain('\x1b[1m');
		expect(nested).toContain('\x1b[36m');
		expect(nested).toContain('hi');
	});
});

// ---------------------------------------------------------------------------
// renderToolCall — uses ● (U+25CF Black Circle) in Claude Code style
// ---------------------------------------------------------------------------

describe('renderToolCall', () => {
	const colors = noColors();

	it('should format a tool call with JSON args (verbose)', () => {
		const result = renderToolCall(
			'memory_search',
			'{"query": "auth flow"}',
			colors,
			{ verbose: true },
		);
		expect(result).toContain('●');
		expect(result).toContain('Search');
		expect(result).toContain('query: "auth flow"');
	});

	it('should format with empty args (no parens)', () => {
		const result = renderToolCall('vfs_tree', '{}', colors);
		expect(result).toContain('●');
		expect(result).not.toContain('()');
	});

	it('should format multiple parameters (verbose)', () => {
		const result = renderToolCall(
			'vfs_write',
			'{"path": "vfs:///app.py", "content": "print()"}',
			colors,
			{ verbose: true },
		);
		expect(result).toContain('path: "vfs:///app.py"');
		expect(result).toContain('content: "print()"');
	});

	it('should handle non-string values (verbose)', () => {
		const result = renderToolCall(
			'memory_search',
			'{"query": "test", "maxResults": 10}',
			colors,
			{ verbose: true },
		);
		expect(result).toContain('maxResults: 10');
	});

	it('should truncate very long tool call strings', () => {
		const longVal = 'x'.repeat(200);
		const result = renderToolCall(
			'test_tool',
			`{"data": "${longVal}"}`,
			colors,
		);
		expect(result).toContain('...');
	});

	it('should handle malformed JSON gracefully', () => {
		const result = renderToolCall('broken', 'not-json', colors);
		expect(result).toContain('●');
		expect(result).toContain('broken');
		expect(result).toContain('not-json');
	});

	it('should handle empty string args', () => {
		const result = renderToolCall('test', '', colors);
		expect(result).toContain('●');
		expect(result).toContain('test');
	});
});

// ---------------------------------------------------------------------------
// renderToolResult
// ---------------------------------------------------------------------------

describe('renderToolResult', () => {
	const colors = noColors();

	it('should render a single-line result with ⎿', () => {
		const result = renderToolResult('3 results found', false, colors);
		expect(result).toContain('⎿');
		expect(result).toContain('3 results found');
	});

	it('should render multi-line results (up to 8 lines)', () => {
		const lines = Array.from({ length: 5 }, (_, i) => `Line ${i + 1}`);
		const result = renderToolResult(lines.join('\n'), false, colors);
		expect(result).toContain('Line 1');
		expect(result).toContain('Line 5');
	});

	it('should truncate results beyond 8 lines', () => {
		const lines = Array.from({ length: 12 }, (_, i) => `Line ${i + 1}`);
		const result = renderToolResult(lines.join('\n'), false, colors);
		expect(result).toContain('Line 1');
		expect(result).toContain('Line 8');
		expect(result).not.toContain('Line 9');
		expect(result).toContain('+4 more lines');
	});

	it('should truncate individual long lines at 200 chars', () => {
		const longLine = 'a'.repeat(250);
		const result = renderToolResult(longLine, false, colors);
		expect(result).toContain('...');
		expect(result).toContain('a'.repeat(197));
	});

	it('should color error results in red (when colors enabled)', () => {
		const c = withColors();
		const result = renderToolResult('Something failed', true, c);
		expect(result).toContain('\x1b[31m'); // red
		expect(result).toContain('Something failed');
	});

	it('should not color success results in red', () => {
		const c = withColors();
		const result = renderToolResult('Success', false, c);
		expect(result).not.toContain('\x1b[31m'); // no red on main content
	});

	it('should handle empty output', () => {
		const result = renderToolResult('', false, colors);
		expect(result).toContain('⎿');
	});

	it('should indent continuation lines at 6 spaces', () => {
		const result = renderToolResult('Line 1\nLine 2\nLine 3', false, colors);
		const lines = result.split('\n');
		// First line starts with `    ⎿ ` (4 spaces + ⎿ + space)
		expect(lines[0]).toMatch(/^\s+⎿/);
		// Continuation lines start with 6 spaces
		expect(lines[1]).toMatch(/^\s{6}/);
		expect(lines[2]).toMatch(/^\s{6}/);
	});
});

// ---------------------------------------------------------------------------
// renderBanner — Claude Code style: mascot + 3 text lines (no box)
// ---------------------------------------------------------------------------

describe('renderBanner', () => {
	const colors = noColors();

	it('should include version', () => {
		const result = renderBanner(
			{
				version: '1.2.3',
				dataDir: '/tmp/data',
				workDir: '/home/user',
			},
			colors,
		);
		expect(result).toContain('v1.2.3');
		expect(result).toContain('simse');
	});

	it('should include cwd', () => {
		const result = renderBanner(
			{
				version: '1.0.0',
				dataDir: '/data',
				workDir: '/home/user/project',
			},
			colors,
		);
		expect(result).toContain('/home/user/project');
	});

	it('should include model when provided', () => {
		const result = renderBanner(
			{
				version: '1.0.0',
				dataDir: '/data',
				workDir: '/cwd',
				model: 'llama3:8b',
			},
			colors,
		);
		// New banner displays model directly (no "Model:" label)
		expect(result).toContain('llama3:8b');
	});

	it('should render mascot characters', () => {
		const result = renderBanner(
			{
				version: '1.0.0',
				dataDir: '/data',
				workDir: '/cwd',
			},
			colors,
		);
		// Mascot uses Unicode box-drawing characters
		expect(result).toContain('╭──╮');
		expect(result).toContain('╰─╮│');
		expect(result).toContain('╰╯');
	});

	it('should handle all optional fields omitted', () => {
		const result = renderBanner(
			{
				version: '0.0.1',
				dataDir: '/data',
				workDir: '/cwd',
			},
			colors,
		);
		expect(result).toContain('simse');
		expect(result).toContain('v0.0.1');
		expect(result).toContain('/cwd');
	});

	it('should render mascot and text side-by-side', () => {
		const result = renderBanner(
			{
				version: '1.0.0',
				dataDir: '/data',
				workDir: '/cwd',
				model: 'llama3 · ollama',
			},
			colors,
		);
		const lines = result.split('\n');
		// Should have at least 3 lines (mascot is 3 lines, text is 3 lines)
		expect(lines.length).toBeGreaterThanOrEqual(3);
		// First line should contain both mascot and version
		expect(lines[0]).toContain('╭──╮');
		expect(lines[0]).toContain('simse-code');
	});
});

// ---------------------------------------------------------------------------
// renderServiceStatus — label + detail only (no bullet), for spinner use
// ---------------------------------------------------------------------------

describe('renderServiceStatus', () => {
	const colors = noColors();

	it('should render label and detail without bullet', () => {
		const result = renderServiceStatus('ACP', 'ok', 'Connected', colors);
		expect(result).not.toContain('●');
		expect(result).toContain('ACP');
		expect(result).toContain('Connected');
	});
});

// ---------------------------------------------------------------------------
// renderServiceStatusLine — standalone line with ● for non-spinner contexts
// ---------------------------------------------------------------------------

describe('renderServiceStatusLine', () => {
	const colors = noColors();

	it('should render ok status with ● icon', () => {
		const result = renderServiceStatusLine('ACP', 'ok', 'Connected', colors);
		expect(result).toContain('●');
		expect(result).toContain('ACP');
		expect(result).toContain('Connected');
	});

	it('should render warn status with ● icon', () => {
		const result = renderServiceStatusLine('MCP', 'warn', 'Slow', colors);
		expect(result).toContain('●');
		expect(result).toContain('MCP');
	});

	it('should render fail status with ● icon', () => {
		const result = renderServiceStatusLine('Memory', 'fail', 'Offline', colors);
		expect(result).toContain('●');
		expect(result).toContain('Memory');
		expect(result).toContain('Offline');
	});
});

// ---------------------------------------------------------------------------
// renderHelp
// ---------------------------------------------------------------------------

describe('renderHelp', () => {
	const colors = noColors();

	it('should group commands by category', () => {
		const commands: CommandInfo[] = [
			{
				name: 'clear',
				usage: '/clear',
				description: 'Clear conversation',
				category: 'session',
			},
			{
				name: 'help',
				usage: '/help',
				description: 'Show help',
				category: 'info',
			},
		];

		const result = renderHelp(
			commands,
			{ session: 'Session', info: 'Info' },
			colors,
		);
		expect(result).toContain('Session:');
		expect(result).toContain('Info:');
		expect(result).toContain('/clear');
		expect(result).toContain('/help');
	});

	it('should show aliases when present', () => {
		const commands: CommandInfo[] = [
			{
				name: 'help',
				aliases: ['/h', '/?'],
				usage: '/help',
				description: 'Show help',
				category: 'info',
			},
		];

		const result = renderHelp(commands, { info: 'Info' }, colors);
		expect(result).toContain('/h');
		expect(result).toContain('/?');
	});

	it('should pad usage column for alignment', () => {
		const commands: CommandInfo[] = [
			{
				name: 'a',
				usage: '/a',
				description: 'Short',
				category: 'x',
			},
			{
				name: 'longer',
				usage: '/longer-command',
				description: 'Long',
				category: 'x',
			},
		];

		const result = renderHelp(commands, { x: 'Commands' }, colors);
		const lines = result.split('\n');
		const cmdLines = lines.filter((l) => l.includes('/'));
		expect(cmdLines).toHaveLength(2);
	});
});

// ---------------------------------------------------------------------------
// renderDetailLine — uses ⎿ prefix in CC style
// ---------------------------------------------------------------------------

describe('renderDetailLine', () => {
	const colors = noColors();

	it('should render with ⎿ prefix', () => {
		const result = renderDetailLine('stored as abc123', colors);
		expect(result).toContain('⎿');
		expect(result).toContain('stored as abc123');
	});

	it('should indent with 4 spaces before ⎿', () => {
		const result = renderDetailLine('test', colors);
		expect(result).toBe('    ⎿ test');
	});
});

// ---------------------------------------------------------------------------
// renderError — uses ● (red) in CC style
// ---------------------------------------------------------------------------

describe('renderError', () => {
	const colors = noColors();

	it('should show ● and message', () => {
		const result = renderError('Connection refused', colors);
		expect(result).toContain('●');
		expect(result).toContain('Connection refused');
	});

	it('should indent with 2 spaces', () => {
		const result = renderError('fail', colors);
		expect(result.startsWith('  ')).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// renderSkillLoading — uses ● prefix in CC style
// ---------------------------------------------------------------------------

describe('renderSkillLoading', () => {
	const colors = noColors();

	it('should show skill name with ● icon', () => {
		const result = renderSkillLoading('commit', colors);
		expect(result).toContain('●');
		expect(result).toContain('commit');
		expect(result).toContain('Loading skill:');
	});
});

// ---------------------------------------------------------------------------
// renderAssistantMessage
// ---------------------------------------------------------------------------

describe('renderAssistantMessage', () => {
	const colors = noColors();
	const md = createMarkdownRenderer(colors);

	it('should indent all lines by 2 spaces', () => {
		const result = renderAssistantMessage('Hello\nWorld', md, colors);
		const lines = result.split('\n');
		for (const line of lines) {
			expect(line.startsWith('  ')).toBe(true);
		}
	});
});

// ---------------------------------------------------------------------------
// createMarkdownRenderer
// ---------------------------------------------------------------------------

describe('createMarkdownRenderer', () => {
	const colors = noColors();
	const md = createMarkdownRenderer(colors);

	it('should render H1 headers', () => {
		const result = md.render('# Title');
		expect(result).toContain('Title');
	});

	it('should render H2 headers', () => {
		const result = md.render('## Subtitle');
		expect(result).toContain('Subtitle');
	});

	it('should render H3 headers', () => {
		const result = md.render('### Section');
		expect(result).toContain('Section');
	});

	it('should render code blocks with language hint', () => {
		const input = '```typescript\nconst x = 1;\n```';
		const result = md.render(input);
		expect(result).toContain('typescript');
		expect(result).toContain('const x = 1;');
	});

	it('should render code blocks without language', () => {
		const input = '```\ncode here\n```';
		const result = md.render(input);
		expect(result).toContain('code here');
	});

	it('should render blockquotes', () => {
		const result = md.render('> This is a quote');
		expect(result).toContain('│');
		expect(result).toContain('This is a quote');
	});

	it('should render unordered lists with dash bullet', () => {
		const result = md.render('- Item one\n- Item two');
		expect(result).toContain('-');
		expect(result).toContain('Item one');
		expect(result).toContain('Item two');
	});

	it('should render ordered lists', () => {
		const result = md.render('1. First\n2. Second');
		expect(result).toContain('First');
		expect(result).toContain('Second');
	});

	it('should render horizontal rules', () => {
		const result = md.render('---');
		expect(result).toContain('─');
	});

	it('should handle bold inline formatting', () => {
		const result = md.render('This is **bold** text');
		expect(result).toContain('bold');
	});

	it('should handle italic inline formatting', () => {
		const result = md.render('This is *italic* text');
		expect(result).toContain('italic');
	});

	it('should handle inline code formatting', () => {
		const result = md.render('Use `console.log` here');
		expect(result).toContain('console.log');
	});

	it('should handle empty input', () => {
		const result = md.render('');
		expect(result).toBe('');
	});

	it('should handle plain text passthrough', () => {
		const result = md.render('Just some text');
		expect(result).toBe('Just some text');
	});
});
