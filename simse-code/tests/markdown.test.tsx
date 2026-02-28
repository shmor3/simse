import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { Markdown } from '../components/chat/markdown.js';

describe('Markdown', () => {
	test('empty text returns null', () => {
		const { lastFrame } = render(<Markdown text="" />);
		expect(lastFrame()).toBe('');
	});

	test('plain text renders unchanged', () => {
		const { lastFrame } = render(<Markdown text="Hello world" />);
		expect(lastFrame()).toBe('Hello world');
	});

	test('renders **bold** text', () => {
		const { lastFrame } = render(<Markdown text="Hello **bold** world" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Hello');
		expect(frame).toContain('bold');
		expect(frame).toContain('world');
	});

	test('renders *italic* text', () => {
		const { lastFrame } = render(<Markdown text="Hello *italic* world" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Hello');
		expect(frame).toContain('italic');
		expect(frame).toContain('world');
	});

	test('renders `inline code` with cyan color', () => {
		const { lastFrame } = render(<Markdown text="Use `console.log` here" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Use');
		expect(frame).toContain('console.log');
		expect(frame).toContain('here');
	});

	test('renders fenced code blocks with language label and gutter', () => {
		const text = '```typescript\nconst x = 1;\nconst y = 2;\n```';
		const { lastFrame } = render(<Markdown text={text} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('typescript');
		expect(frame).toContain('const x = 1;');
		expect(frame).toContain('const y = 2;');
		// Gutter character
		expect(frame).toContain('\u2502');
	});

	test('renders fenced code blocks without language', () => {
		const text = '```\nhello\n```';
		const { lastFrame } = render(<Markdown text={text} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('hello');
		expect(frame).toContain('\u2502');
	});

	test('renders # h1 header bold and cyan', () => {
		const { lastFrame } = render(<Markdown text="# Main Title" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Main Title');
	});

	test('renders ## h2 header bold', () => {
		const { lastFrame } = render(<Markdown text="## Sub Title" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Sub Title');
	});

	test('renders ### h3 header underlined', () => {
		const { lastFrame } = render(<Markdown text="### Section" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Section');
	});

	test('renders - list items with dash bullets', () => {
		const text = '- First item\n- Second item\n- Third item';
		const { lastFrame } = render(<Markdown text={text} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('First item');
		expect(frame).toContain('Second item');
		expect(frame).toContain('Third item');
	});

	test('renders > blockquotes with dim bar prefix', () => {
		const { lastFrame } = render(<Markdown text="> This is a quote" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('This is a quote');
		// Blockquote bar character
		expect(frame).toContain('\u2502');
	});

	test('renders --- as horizontal rule (dim repeated dash)', () => {
		const { lastFrame } = render(<Markdown text="---" />);
		const frame = lastFrame() ?? '';
		// Horizontal rule uses box-drawing horizontal char
		expect(frame).toContain('\u2500');
	});

	test('renders multi-line document with mixed elements', () => {
		const text = [
			'# Title',
			'',
			'Some **bold** and *italic* text.',
			'',
			'- Item one',
			'- Item two',
			'',
			'> A quote',
			'',
			'---',
			'',
			'```js',
			'const x = 1;',
			'```',
		].join('\n');
		const { lastFrame } = render(<Markdown text={text} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Title');
		expect(frame).toContain('bold');
		expect(frame).toContain('italic');
		expect(frame).toContain('Item one');
		expect(frame).toContain('Item two');
		expect(frame).toContain('A quote');
		expect(frame).toContain('\u2500');
		expect(frame).toContain('const x = 1;');
	});

	test('nested bold and italic in same line', () => {
		const { lastFrame } = render(<Markdown text="**bold** then *italic*" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('bold');
		expect(frame).toContain('then');
		expect(frame).toContain('italic');
	});

	test('multiple inline code spans', () => {
		const { lastFrame } = render(<Markdown text="Use `foo` and `bar`" />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('foo');
		expect(frame).toContain('bar');
	});

	test('indented list items', () => {
		const text = '- Top\n  - Nested';
		const { lastFrame } = render(<Markdown text={text} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Top');
		expect(frame).toContain('Nested');
	});
});
