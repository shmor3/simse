import chalk from 'chalk';
import { Box, Text } from 'ink';
import type React from 'react';

interface MarkdownProps {
	readonly text: string;
}

/**
 * Parse inline markdown (bold, italic, inline code) into Ink Text elements.
 */
function parseInline(line: string): React.ReactNode[] {
	const nodes: React.ReactNode[] = [];
	// Regex matches: `code`, **bold**, ~~strikethrough~~, [text](url), or *italic*
	const pattern =
		/`([^`]+)`|\*\*(.+?)\*\*|~~(.+?)~~|\[([^\]]+)\]\(([^)]+)\)|\*(.+?)\*/g;
	const matches = Array.from(line.matchAll(pattern));
	let lastIndex = 0;
	let key = 0;

	for (const match of matches) {
		// Push text before this match
		if (match.index > lastIndex) {
			nodes.push(<Text key={key++}>{line.slice(lastIndex, match.index)}</Text>);
		}

		if (match[1] !== undefined) {
			// Inline code
			nodes.push(
				<Text key={key++} color="cyan">
					{match[1]}
				</Text>,
			);
		} else if (match[2] !== undefined) {
			// Bold
			nodes.push(
				<Text key={key++} bold>
					{match[2]}
				</Text>,
			);
		} else if (match[3] !== undefined) {
			// Strikethrough
			nodes.push(
				<Text key={key++} strikethrough dimColor>
					{match[3]}
				</Text>,
			);
		} else if (match[4] !== undefined) {
			// Link [text](url) — show text underlined + dimmed url
			nodes.push(
				<Text key={key++} underline color="blue">
					{match[4]}
				</Text>,
			);
			nodes.push(
				<Text key={key++} dimColor>
					{` (${match[5]})`}
				</Text>,
			);
		} else if (match[6] !== undefined) {
			// Italic
			nodes.push(
				<Text key={key++} italic>
					{match[6]}
				</Text>,
			);
		}

		lastIndex = match.index + match[0].length;
	}

	// Push remaining text
	if (lastIndex < line.length) {
		nodes.push(<Text key={key++}>{line.slice(lastIndex)}</Text>);
	}

	// If no matches were found, return the raw text
	if (nodes.length === 0) {
		return [<Text key={0}>{line}</Text>];
	}

	return nodes;
}

interface BlockNode {
	readonly key: string;
	readonly type:
		| 'paragraph'
		| 'heading'
		| 'code-block'
		| 'list-item'
		| 'numbered-list-item'
		| 'task-list-item'
		| 'blockquote'
		| 'horizontal-rule'
		| 'table'
		| 'blank';
	readonly content: string;
	readonly level?: number;
	readonly language?: string;
	readonly checked?: boolean;
	readonly number?: number;
	readonly rows?: readonly string[][];
}

/**
 * Parse markdown text into block-level nodes.
 */
function parseBlocks(text: string): BlockNode[] {
	const lines = text.split('\n');
	const blocks: BlockNode[] = [];
	let i = 0;
	let blockId = 0;

	while (i < lines.length) {
		const line = lines[i]!;
		const key = `b${blockId++}`;

		// Fenced code block
		if (line.startsWith('```')) {
			const language = line.slice(3).trim() || undefined;
			const codeLines: string[] = [];
			i++;
			while (i < lines.length && !lines[i]!.startsWith('```')) {
				codeLines.push(lines[i]!);
				i++;
			}
			// Skip closing ```
			if (i < lines.length) i++;
			blocks.push({
				key,
				type: 'code-block',
				content: codeLines.join('\n'),
				language,
			});
			continue;
		}

		// Horizontal rule
		if (/^---+$/.test(line.trim())) {
			blocks.push({ key, type: 'horizontal-rule', content: '' });
			i++;
			continue;
		}

		// Headings
		const headingMatch = line.match(/^(#{1,3})\s+(.+)$/);
		if (headingMatch) {
			blocks.push({
				key,
				type: 'heading',
				content: headingMatch[2]!,
				level: headingMatch[1]!.length,
			});
			i++;
			continue;
		}

		// Task list items (- [ ] or - [x])
		const taskMatch = line.match(/^(\s*)-\s+\[([ x])\]\s+(.+)$/);
		if (taskMatch) {
			const indent = Math.floor(taskMatch[1]!.length / 2);
			blocks.push({
				key,
				type: 'task-list-item',
				content: taskMatch[3]!,
				level: indent,
				checked: taskMatch[2] === 'x',
			});
			i++;
			continue;
		}

		// List items (- bullets, with optional indentation)
		const listMatch = line.match(/^(\s*)-\s+(.+)$/);
		if (listMatch) {
			const indent = Math.floor(listMatch[1]!.length / 2);
			blocks.push({
				key,
				type: 'list-item',
				content: listMatch[2]!,
				level: indent,
			});
			i++;
			continue;
		}

		// Numbered list items (1. 2. etc.)
		const numMatch = line.match(/^(\s*)(\d+)\.\s+(.+)$/);
		if (numMatch) {
			const indent = Math.floor(numMatch[1]!.length / 2);
			blocks.push({
				key,
				type: 'numbered-list-item',
				content: numMatch[3]!,
				level: indent,
				number: Number.parseInt(numMatch[2]!, 10),
			});
			i++;
			continue;
		}

		// Table (header row followed by separator row)
		if (
			line.includes('|') &&
			i + 1 < lines.length &&
			/^\|?[\s-]+(\|[\s-]+)+\|?$/.test(lines[i + 1]!)
		) {
			const rows: string[][] = [];
			let j = i;
			while (j < lines.length && lines[j]!.includes('|')) {
				const row = lines[j]!.replace(/^\|/, '')
					.replace(/\|$/, '')
					.split('|')
					.map((cell) => cell.trim());
				// Skip separator rows
				if (!/^[\s-]+$/.test(row.join(''))) {
					rows.push(row);
				}
				j++;
			}
			if (rows.length > 0) {
				blocks.push({ key, type: 'table', content: '', rows });
				i = j;
				continue;
			}
		}

		// Blockquote
		const quoteMatch = line.match(/^>\s?(.*)$/);
		if (quoteMatch) {
			blocks.push({
				key,
				type: 'blockquote',
				content: quoteMatch[1]!,
			});
			i++;
			continue;
		}

		// Blank line
		if (line.trim() === '') {
			blocks.push({ key, type: 'blank', content: '' });
			i++;
			continue;
		}

		// Regular paragraph
		blocks.push({ key, type: 'paragraph', content: line });
		i++;
	}

	return blocks;
}

function HeadingBlock({ content, level }: { content: string; level: number }) {
	switch (level) {
		case 1:
			return (
				<Text bold color="cyan">
					{content}
				</Text>
			);
		case 2:
			return <Text bold>{content}</Text>;
		case 3:
			return <Text underline>{content}</Text>;
		default:
			return <Text bold>{content}</Text>;
	}
}

function keyedLines(content: string): { key: string; text: string }[] {
	let id = 0;
	return content.split('\n').map((text) => ({ key: `cl${id++}`, text }));
}

// ---------------------------------------------------------------------------
// Lightweight syntax highlighter (no external deps)
// ---------------------------------------------------------------------------

const JS_KEYWORDS =
	/\b(const|let|var|function|return|if|else|for|while|class|import|export|from|default|async|await|new|throw|try|catch|finally|typeof|instanceof|in|of|switch|case|break|continue|yield|type|interface|readonly|extends|implements)\b/g;

const PY_KEYWORDS =
	/\b(def|class|return|if|elif|else|for|while|import|from|as|with|try|except|finally|raise|yield|lambda|pass|break|continue|and|or|not|in|is|True|False|None|self|async|await)\b/g;

const BASH_KEYWORDS =
	/\b(if|then|else|elif|fi|for|while|do|done|case|esac|function|return|exit|export|source|echo|cd|ls|grep|sed|awk|cat|rm|cp|mv|mkdir|chmod|chown|sudo|apt|npm|bun|git|docker)\b/g;

type HighlightLang = 'js' | 'py' | 'bash' | 'json' | 'none';

function detectLang(language?: string): HighlightLang {
	if (!language) return 'none';
	const l = language.toLowerCase();
	if (
		l === 'js' ||
		l === 'javascript' ||
		l === 'ts' ||
		l === 'typescript' ||
		l === 'tsx' ||
		l === 'jsx'
	)
		return 'js';
	if (l === 'python' || l === 'py') return 'py';
	if (l === 'bash' || l === 'sh' || l === 'shell' || l === 'zsh') return 'bash';
	if (l === 'json' || l === 'jsonc') return 'json';
	return 'none';
}

function highlightLine(line: string, lang: HighlightLang): string {
	if (lang === 'none') return line;

	if (lang === 'json') {
		return line
			.replace(/"([^"]*)"(?=\s*:)/g, (m) => chalk.cyan(m)) // keys
			.replace(/:\s*"([^"]*)"/g, (m) => chalk.green(m)) // string values
			.replace(/:\s*(-?\d+\.?\d*)/g, (_, n) => `: ${chalk.yellow(n)}`) // numbers
			.replace(/\b(true|false|null)\b/g, (m) => chalk.magenta(m));
	}

	let result = line;

	// Strings (single and double quoted)
	const strings: string[] = [];
	result = result.replace(/(["'])(?:(?=(\\?))\2.)*?\1/g, (m) => {
		strings.push(m);
		return `\x00S${strings.length - 1}\x00`;
	});

	// Comments
	const commentIdx = lang === 'py' ? result.indexOf('#') : result.indexOf('//');
	let comment = '';
	if (commentIdx >= 0) {
		comment = result.slice(commentIdx);
		result = result.slice(0, commentIdx);
	}

	// Keywords
	const keywords =
		lang === 'py' ? PY_KEYWORDS : lang === 'bash' ? BASH_KEYWORDS : JS_KEYWORDS;
	result = result.replace(keywords, (m) => chalk.magenta(m));

	// Numbers
	result = result.replace(/\b(\d+\.?\d*)\b/g, (m) => chalk.yellow(m));

	// Restore strings
	result = result.replace(/\x00S(\d+)\x00/g, (_, i) =>
		chalk.green(strings[Number.parseInt(i, 10)]!),
	);

	// Append comment
	if (comment) {
		result += chalk.dim(comment);
	}

	return result;
}

function CodeBlock({
	content,
	language,
}: {
	content: string;
	language?: string;
}) {
	const lang = detectLang(language);
	const lines = keyedLines(content);
	return (
		<Box flexDirection="column">
			{language && <Text dimColor>{language}</Text>}
			{lines.map((line) => (
				<Text key={line.key}>
					<Text dimColor>{'\u2502'} </Text>
					{highlightLine(line.text, lang)}
				</Text>
			))}
		</Box>
	);
}

function ListItem({ content, level = 0 }: { content: string; level?: number }) {
	const indent = '  '.repeat(level);
	return (
		<Text>
			{indent}- {parseInline(content)}
		</Text>
	);
}

function NumberedListItem({
	content,
	level = 0,
	number = 1,
}: {
	content: string;
	level?: number;
	number?: number;
}) {
	const indent = '  '.repeat(level);
	return (
		<Text>
			{indent}
			{number}. {parseInline(content)}
		</Text>
	);
}

function TaskListItem({
	content,
	level = 0,
	checked = false,
}: {
	content: string;
	level?: number;
	checked?: boolean;
}) {
	const indent = '  '.repeat(level);
	const checkbox = checked ? '\u2611' : '\u2610';
	return (
		<Text>
			{indent}
			{checkbox}{' '}
			{checked ? <Text dimColor>{content}</Text> : parseInline(content)}
		</Text>
	);
}

function TableBlock({ rows }: { rows: readonly string[][] }) {
	if (rows.length === 0) return null;

	// Calculate column widths
	const colCount = Math.max(...rows.map((r) => r.length));
	const colWidths: number[] = Array.from({ length: colCount }, () => 0);
	for (const row of rows) {
		for (let c = 0; c < colCount; c++) {
			colWidths[c] = Math.max(colWidths[c]!, (row[c] ?? '').length);
		}
	}

	const renderRow = (row: string[], rowIdx: number, isHeader: boolean) => {
		const cells = Array.from({ length: colCount }, (_, c) => {
			const cell = (row[c] ?? '').padEnd(colWidths[c]!);
			return cell;
		});
		return (
			<Text key={`tr${rowIdx}`}>
				<Text dimColor>{'\u2502'} </Text>
				{cells.map((cell, c) => (
					<Text key={`tc${rowIdx}-${c}`}>
						{isHeader ? <Text bold>{cell}</Text> : <>{parseInline(cell)}</>}
						{c < colCount - 1 ? <Text dimColor> {'\u2502'} </Text> : null}
					</Text>
				))}
			</Text>
		);
	};

	const separator = colWidths
		.map((w) => '\u2500'.repeat(w))
		.join('\u2500\u253c\u2500');

	return (
		<Box flexDirection="column">
			{rows.map((row, idx) => (
				<Box key={`tg${idx}`} flexDirection="column">
					{renderRow(row, idx, idx === 0)}
					{idx === 0 && (
						<Text dimColor>
							{'\u2502'} {separator}
						</Text>
					)}
				</Box>
			))}
		</Box>
	);
}

function Blockquote({ content }: { content: string }) {
	return (
		<Text>
			<Text dimColor>{'\u2502'} </Text>
			{parseInline(content)}
		</Text>
	);
}

function HorizontalRule() {
	return <Text dimColor>{'\u2500'.repeat(40)}</Text>;
}

function BlockView({ block }: { block: BlockNode }) {
	switch (block.type) {
		case 'heading':
			return <HeadingBlock content={block.content} level={block.level ?? 1} />;
		case 'code-block':
			return <CodeBlock content={block.content} language={block.language} />;
		case 'list-item':
			return <ListItem content={block.content} level={block.level} />;
		case 'numbered-list-item':
			return (
				<NumberedListItem
					content={block.content}
					level={block.level}
					number={block.number}
				/>
			);
		case 'task-list-item':
			return (
				<TaskListItem
					content={block.content}
					level={block.level}
					checked={block.checked}
				/>
			);
		case 'blockquote':
			return <Blockquote content={block.content} />;
		case 'table':
			return <TableBlock rows={block.rows ?? []} />;
		case 'horizontal-rule':
			return <HorizontalRule />;
		case 'blank':
			return <Text> </Text>;
		case 'paragraph':
			return <Text>{parseInline(block.content)}</Text>;
	}
}

export function Markdown({ text }: MarkdownProps) {
	if (!text) return null;

	const blocks = parseBlocks(text);

	// Single paragraph with no special blocks: render inline only
	if (blocks.length === 1 && blocks[0]!.type === 'paragraph') {
		return <Text>{parseInline(blocks[0]!.content)}</Text>;
	}

	return (
		<Box flexDirection="column">
			{blocks.map((block) => (
				<BlockView key={block.key} block={block} />
			))}
		</Box>
	);
}
