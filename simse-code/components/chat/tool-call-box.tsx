import InkSpinner from 'ink-spinner';
import { Box, Text } from 'ink';
import React from 'react';

interface ToolCallBoxProps {
	readonly name: string;
	readonly args: string;
	readonly status: 'active' | 'completed' | 'failed';
	readonly duration?: number;
	readonly summary?: string;
	readonly error?: string;
	readonly diff?: string;
}

function formatArgs(argsStr: string): string {
	try {
		const parsed = JSON.parse(argsStr);
		if (typeof parsed === 'object' && parsed !== null) {
			return Object.entries(parsed)
				.map(([k, v]) => `${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`)
				.join(', ');
		}
	} catch {
		// fallback to raw string
	}
	return argsStr.length > 200 ? `${argsStr.slice(0, 200)}...` : argsStr;
}

function StatusIcon({ status }: { status: ToolCallBoxProps['status'] }) {
	switch (status) {
		case 'active':
			return (
				<Text color="cyan">
					<InkSpinner type="dots" />
				</Text>
			);
		case 'completed':
			return <Text color="green">✓</Text>;
		case 'failed':
			return <Text color="red">✗</Text>;
	}
}

function borderColor(status: ToolCallBoxProps['status']): string {
	switch (status) {
		case 'active':
			return 'cyan';
		case 'completed':
			return 'green';
		case 'failed':
			return 'red';
	}
}

export function ToolCallBox({
	name,
	args,
	status,
	duration,
	summary,
	error,
	diff,
}: ToolCallBoxProps) {
	const meta: string[] = [];
	if (duration !== undefined) meta.push(`${duration}ms`);
	if (summary) meta.push(summary);

	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={borderColor(status)}
			paddingX={1}
			marginLeft={2}
		>
			<Box gap={1}>
				<StatusIcon status={status} />
				<Text bold>{name}</Text>
				{meta.length > 0 && <Text dimColor>({meta.join(', ')})</Text>}
			</Box>

			<Text dimColor>{formatArgs(args)}</Text>

			{diff && (
				<Box marginTop={1}>
					<Text>{diff}</Text>
				</Box>
			)}

			{error && (
				<Box marginTop={1}>
					<Text color="red">{error}</Text>
				</Box>
			)}
		</Box>
	);
}
