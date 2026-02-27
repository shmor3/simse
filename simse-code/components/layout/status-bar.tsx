import { Box, Text } from 'ink';
import React from 'react';
import { Badge } from '../shared/badge.js';

interface StatusBarProps {
	readonly server?: string;
	readonly model?: string;
	readonly tokens?: number;
	readonly cost?: string;
	readonly additions?: number;
	readonly deletions?: number;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
	readonly permissionMode?: string;
}

export function StatusBar({
	server,
	model,
	tokens = 0,
	cost,
	additions,
	deletions,
	planMode,
	verbose,
	permissionMode,
}: StatusBarProps) {
	const parts: string[] = [];
	if (server && model) parts.push(`${server}:${model}`);
	else if (server) parts.push(server);
	if (tokens > 0) parts.push(`${tokens} tokens`);
	if (cost) parts.push(cost);

	const changes: string[] = [];
	if (additions && additions > 0) changes.push(`+${additions}`);
	if (deletions && deletions > 0) changes.push(`-${deletions}`);

	return (
		<Box paddingX={1}>
			<Box flexGrow={1} gap={1}>
				<Text dimColor>{parts.join(' · ')}</Text>
				{changes.length > 0 && (
					<Text>
						<Text color="green">{changes[0]}</Text>
						{changes[1] && <Text color="red"> {changes[1]}</Text>}
					</Text>
				)}
			</Box>
			<Box gap={1}>
				{planMode && <Badge label="PLAN" />}
				{verbose && <Badge label="VERBOSE" />}
				{permissionMode === 'dontAsk' && <Badge label="YOLO" />}
			</Box>
		</Box>
	);
}
