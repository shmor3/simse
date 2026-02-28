import { Box, Text } from 'ink';
import React from 'react';

export function formatTokens(tokens: number): string {
	return tokens >= 1000 ? `${(tokens / 1000).toFixed(1)}k` : `${tokens}`;
}

interface StatusBarProps {
	readonly isProcessing?: boolean;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
	readonly bypassPermissions?: boolean;
	readonly totalTokens?: number;
	readonly contextPercent?: number;
	readonly permissionMode?: string;
}

const SEP = ' \u00b7 ';

export function StatusBar({
	isProcessing,
	planMode,
	verbose,
	bypassPermissions,
	totalTokens,
	contextPercent,
	permissionMode,
}: StatusBarProps) {
	const hints: string[] = [];

	if (permissionMode) {
		hints.push(`${permissionMode} (shift+tab to cycle)`);
	} else if (bypassPermissions) {
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

	// Right-aligned stats
	const stats: string[] = [];
	if (totalTokens && totalTokens > 0) {
		stats.push(`${formatTokens(totalTokens)} tokens`);
	}
	if (contextPercent !== undefined && contextPercent > 0) {
		stats.push(`${contextPercent}% context`);
	}

	return (
		<Box paddingX={1} justifyContent="space-between">
			<Text dimColor>{hints.join(SEP)}</Text>
			{stats.length > 0 && <Text dimColor>{stats.join(SEP)}</Text>}
		</Box>
	);
}
