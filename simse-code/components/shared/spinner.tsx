import InkSpinner from 'ink-spinner';
import { Box, Text } from 'ink';
import React from 'react';

interface ThinkingSpinnerProps {
	readonly label?: string;
	readonly tokens?: number;
	readonly server?: string;
	readonly elapsed?: number;
}

export function ThinkingSpinner({
	label = 'Thinking',
	tokens,
	server,
	elapsed,
}: ThinkingSpinnerProps) {
	const parts: string[] = [];
	if (elapsed !== undefined) parts.push(`${(elapsed / 1000).toFixed(1)}s`);
	if (tokens !== undefined) parts.push(`\u2193 ${tokens}`);
	if (server) parts.push(server);

	const suffix = parts.length > 0 ? ` (${parts.join(' \u00b7 ')})` : '';

	return (
		<Box>
			<Text color="cyan">
				<InkSpinner type="dots" />
			</Text>
			<Text dimColor> {label}{suffix}</Text>
		</Box>
	);
}
