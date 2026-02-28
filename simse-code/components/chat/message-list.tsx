import { Box, Static, Text } from 'ink';
import { useRef } from 'react';
import type { OutputItem } from '../../ink-types.js';
import { ErrorBox } from '../shared/error-box.js';
import { Markdown } from './markdown.js';
import { ToolCallBox } from './tool-call-box.js';

interface MessageListProps {
	readonly items: readonly OutputItem[];
}

function OutputItemView({ item }: { item: OutputItem }) {
	switch (item.kind) {
		case 'message':
			if (item.role === 'user') {
				return (
					<Box>
						<Text color="cyan" bold>
							{'\u276F '}
						</Text>
						<Text bold>{item.text}</Text>
					</Box>
				);
			}
			return (
				<Box paddingLeft={2}>
					<Markdown text={item.text} />
				</Box>
			);
		case 'tool-call':
			return (
				<ToolCallBox
					name={item.name}
					args={item.args}
					status={item.status}
					duration={item.duration}
					summary={item.summary}
					error={item.error}
					diff={item.diff}
				/>
			);
		case 'command-result':
			return <Box>{item.element}</Box>;
		case 'error':
			return <ErrorBox message={item.message} />;
		case 'info':
			return (
				<Box paddingLeft={2}>
					<Text dimColor>{item.text}</Text>
				</Box>
			);
	}
}

interface KeyedItem {
	readonly item: OutputItem;
	readonly id: string;
}

export function MessageList({ items }: MessageListProps) {
	const nextId = useRef(0);
	const prevLen = useRef(0);
	const cached = useRef<KeyedItem[]>([]);

	// Assign stable IDs to new items, creating a NEW array reference
	// so Ink's <Static> detects changes via useMemo dependency check
	if (items.length !== prevLen.current) {
		while (cached.current.length < items.length) {
			const idx = cached.current.length;
			cached.current.push({
				// biome-ignore lint/style/noNonNullAssertion: index is bounds-checked by loop condition
				item: items[idx]!,
				id: `msg-${nextId.current++}`,
			});
		}
		// Create new array reference so <Static>'s useMemo sees a change
		cached.current = [...cached.current];
		prevLen.current = items.length;
	}

	return (
		<Static items={cached.current}>
			{(entry: KeyedItem) => (
				<Box key={entry.id}>
					<OutputItemView item={entry.item} />
				</Box>
			)}
		</Static>
	);
}
