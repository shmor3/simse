import { Box, Text } from 'ink';
import React from 'react';

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly noteCount?: number;
	readonly toolCount?: number;
	readonly agentCount?: number;
}

export function Banner({
	version,
	workDir,
	dataDir,
	server,
	model,
	noteCount,
	toolCount,
	agentCount,
}: BannerProps) {
	return (
		<Box flexDirection="column" paddingX={1} marginBottom={1}>
			<Text bold color="cyan">
				simse <Text dimColor>v{version}</Text>
			</Text>

			<Box marginTop={1} flexDirection="column">
				<Text dimColor>  workDir  {workDir}</Text>
				<Text dimColor>  dataDir  {dataDir}</Text>
				{server && (
					<Text dimColor>
						  server   {server}
						{model ? ` (${model})` : ''}
					</Text>
				)}
			</Box>

			{(noteCount !== undefined || toolCount !== undefined) && (
				<Box marginTop={1} gap={2}>
					{noteCount !== undefined && (
						<Text dimColor>{noteCount} notes</Text>
					)}
					{toolCount !== undefined && (
						<Text dimColor>{toolCount} tools</Text>
					)}
					{agentCount !== undefined && (
						<Text dimColor>{agentCount} agents</Text>
					)}
				</Box>
			)}
		</Box>
	);
}
