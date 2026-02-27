import { Box, Static, Text } from 'ink';
import React, { useRef } from 'react';
import type { OutputItem } from '../../ink-types.js';
import { ErrorBox } from '../shared/error-box.js';
import { ToolCallBox } from './tool-call-box.js';

interface MessageListProps {
	readonly items: readonly OutputItem[];
}

function OutputItemView({ item }: { item: OutputItem }) {
	switch (item.kind) {
		case 'message':
			return (
				<Box paddingLeft={item.role === 'user' ? 0 : 2}>
					<Text
						bold={item.role === 'user'}
						color={item.role === 'user' ? 'white' : undefined}
					>
						{item.text}
					</Text>
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
	readonly key: string;
}

export function MessageList({ items }: MessageListProps) {
	const nextId = useRef(0);
	const cached = useRef<KeyedItem[]>([]);

	// Assign stable keys to new items only
	while (cached.current.length < items.length) {
		const idx = cached.current.length;
		cached.current.push({
			item: items[idx]!,
			key: `msg-${nextId.current++}`,
		});
	}

	return (
		<Static items={cached.current}>
			{({ item, key }) => (
				<Box key={key}>
					<OutputItemView item={item} />
				</Box>
			)}
		</Static>
	);
}
