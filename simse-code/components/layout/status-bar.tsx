import { Box, Text } from 'ink';
import React from 'react';

export function formatTokens(tokens: number): string {
	return tokens >= 1000
		? `${(tokens / 1000).toFixed(1)}k tokens`
		: `${tokens} tokens`;
}

interface StatusBarProps {
	readonly isProcessing?: boolean;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
	readonly bypassPermissions?: boolean;
}

const SEP = ' \u00b7 ';

export function StatusBar({
	isProcessing,
	planMode,
	verbose,
	bypassPermissions,
}: StatusBarProps) {
	const hints: string[] = [];

	if (bypassPermissions) {
		hints.push('bypass permissions on (shift+tab to cycle)');
	}

	if (isProcessing) {
		hints.push('esc to interrupt');
	}

	if (planMode) {
		hints.push('plan mode');
	}

	if (verbose) {
		hints.push('verbose on');
	}

	hints.push('? for shortcuts');

	return (
		<Box paddingX={1}>
			<Text dimColor>{hints.join(SEP)}</Text>
		</Box>
	);
}
