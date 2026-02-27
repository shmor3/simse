import { Box, Text, useInput } from 'ink';
import React from 'react';

interface PermissionDialogProps {
	readonly toolName: string;
	readonly args: Record<string, unknown>;
	readonly onAllow: () => void;
	readonly onDeny: () => void;
	readonly onAllowAlways?: () => void;
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

	const argsStr = JSON.stringify(args, null, 2);
	const truncated =
		argsStr.length > 500 ? `${argsStr.slice(0, 500)}...` : argsStr;

	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor="yellow"
			paddingX={1}
			marginLeft={2}
		>
			<Text bold color="yellow">
				⚠ Permission requested
			</Text>
			<Box marginTop={1}>
				<Text>
					Allow <Text bold>{toolName}</Text>?
				</Text>
			</Box>
			<Box marginTop={1}>
				<Text dimColor>{truncated}</Text>
			</Box>
			<Box marginTop={1} gap={2}>
				<Text color="green">[y] Allow</Text>
				<Text color="red">[n] Deny</Text>
				{onAllowAlways && <Text color="blue">[a] Always</Text>}
			</Box>
		</Box>
	);
}
