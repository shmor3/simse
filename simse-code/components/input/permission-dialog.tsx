import { Box, Text, useInput } from 'ink';
import React from 'react';

interface PermissionDialogProps {
	readonly toolName: string;
	readonly args: Record<string, unknown>;
	readonly onAllow: () => void;
	readonly onDeny: () => void;
	readonly onAllowAlways?: () => void;
}

const PRIMARY_ARG_KEYS = ['command', 'path', 'file_path', 'query', 'name'];

function extractPrimaryArg(args: Record<string, unknown>): string | undefined {
	for (const key of PRIMARY_ARG_KEYS) {
		const value = args[key];
		if (typeof value === 'string' && value.length > 0) {
			return value;
		}
	}
	return undefined;
}

export function PermissionDialog({
	toolName,
	args,
	onAllow,
	onDeny,
	onAllowAlways,
}: PermissionDialogProps) {
	useInput((input) => {
		if (input === 'y') onAllow();
		else if (input === 'n') onDeny();
		else if (input === 'a' && onAllowAlways) onAllowAlways();
	});

	const primaryArg = extractPrimaryArg(args);
	const toolDisplay = primaryArg ? `${toolName}(${primaryArg})` : toolName;

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Box>
				<Text color="yellow">{'⚠  '}</Text>
				<Text>simse wants to run </Text>
				<Text bold>{toolDisplay}</Text>
			</Box>
			<Text> </Text>
			<Text dimColor>
				{'   '}Allow? [y]es / [n]o{onAllowAlways ? ' / [a]lways' : ''}
			</Text>
		</Box>
	);
}
