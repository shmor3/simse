import { Box, Text } from 'ink';
import React from 'react';
import { Badge } from '../shared/badge.js';

export function formatTokens(tokens: number): string {
	return tokens >= 1000
		? `${(tokens / 1000).toFixed(1)}k tokens`
		: `${tokens} tokens`;
}

interface StatusBarProps {
	readonly server?: string;
	readonly model?: string;
	readonly tokens?: number;
	readonly cost?: string;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
	readonly permissionMode?: string;
}

export function StatusBar({
	server,
	model,
	tokens = 0,
	cost,
	planMode,
	verbose,
	permissionMode,
}: StatusBarProps) {
	const parts: string[] = [];
	if (server && model) parts.push(`${server}:${model}`);
	else if (server) parts.push(server);
	else if (model) parts.push(model);
	if (tokens > 0) parts.push(formatTokens(tokens));
	if (cost) parts.push(cost);

	const info =
		parts.length > 0 ? parts.join(' \u00b7 ') : 'no server configured';

	return (
		<Box paddingX={1}>
			<Box flexGrow={1} gap={1}>
				<Text dimColor>{info}</Text>
			</Box>
			<Box gap={1}>
				{planMode && <Badge label="PLAN" />}
				{verbose && <Badge label="VERBOSE" />}
				{permissionMode === 'dontAsk' && <Badge label="YOLO" />}
			</Box>
		</Box>
	);
}
