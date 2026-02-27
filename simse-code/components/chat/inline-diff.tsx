import { Box, Text } from 'ink';
import React from 'react';

export interface DiffLine {
	readonly type: 'add' | 'remove' | 'context';
	readonly content: string;
	readonly oldLineNumber?: number;
	readonly newLineNumber?: number;
}

interface InlineDiffProps {
	readonly path: string;
	readonly lines: readonly DiffLine[];
	readonly maxLines?: number;
}

function lineColor(type: DiffLine['type']): string | undefined {
	switch (type) {
		case 'add':
			return 'green';
		case 'remove':
			return 'red';
		default:
			return undefined;
	}
}

function linePrefix(type: DiffLine['type']): string {
	switch (type) {
		case 'add':
			return '+';
		case 'remove':
			return '-';
		default:
			return ' ';
	}
}

export function InlineDiff({ path, lines, maxLines = 50 }: InlineDiffProps) {
	if (lines.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>{path}: No changes</Text>
			</Box>
		);
	}

	const visible = lines.slice(0, maxLines);
	const truncated = lines.length > maxLines ? lines.length - maxLines : 0;

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold dimColor>{path}</Text>
			{visible.map((line, i) => (
				<Text key={i} color={lineColor(line.type)}>
					{linePrefix(line.type)} {line.content}
				</Text>
			))}
			{truncated > 0 && (
				<Text dimColor>  ... {truncated} more lines</Text>
			)}
		</Box>
	);
}
