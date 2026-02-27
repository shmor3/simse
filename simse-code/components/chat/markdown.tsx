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
	// Regex matches: `code`, **bold**, or *italic*
	const pattern = /`([^`]+)`|\*\*(.+?)\*\*|\*(.+?)\*/g;
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
			// Italic
			nodes.push(
				<Text key={key++} italic>
					{match[3]}
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
		| 'blockquote'
		| 'horizontal-rule'
		| 'blank';
	readonly content: string;
	readonly level?: number;
	readonly language?: string;
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

function CodeBlock({
	content,
	language,
}: {
	content: string;
	language?: string;
}) {
	const lines = keyedLines(content);
	return (
		<Box flexDirection="column">
			{language && <Text dimColor>{language}</Text>}
			{lines.map((line) => (
				<Text key={line.key}>
					<Text dimColor>{'\u2502'} </Text>
					{line.text}
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
		case 'blockquote':
			return <Blockquote content={block.content} />;
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
