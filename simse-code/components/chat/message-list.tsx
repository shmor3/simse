import { Box, Static, Text } from 'ink';
import React from 'react';
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

export function MessageList({ items }: MessageListProps) {
	return (
		<Static items={items.map((item, i) => ({ item, key: i }))}>
			{({ item, key }) => (
				<Box key={key}>
					<OutputItemView item={item} />
				</Box>
			)}
		</Static>
	);
}
