import { Box, Text } from 'ink';
import React from 'react';
import type { CommandDefinition } from '../../ink-types.js';

interface HelpViewProps {
	readonly commands: readonly CommandDefinition[];
}

const CATEGORY_LABELS: Record<string, string> = {
	ai: 'AI & Chains',
	library: 'Library',
	tools: 'Tools & Agents',
	session: 'Session',
	files: 'Files & VFS',
	config: 'Configuration',
	meta: 'General',
};

export function HelpView({ commands }: HelpViewProps) {
	const categories = new Map<string, CommandDefinition[]>();
	for (const cmd of commands) {
		const list = categories.get(cmd.category) ?? [];
		list.push(cmd);
		categories.set(cmd.category, list);
	}

	return (
		<Box flexDirection="column" paddingX={1}>
			{[...categories.entries()].map(([category, cmds]) => (
				<Box key={category} flexDirection="column" marginBottom={1}>
					<Text bold color="cyan">
						{CATEGORY_LABELS[category] ?? category}
					</Text>
					{cmds.map((cmd) => (
						<Box key={cmd.name} gap={2} paddingLeft={2}>
							<Text>{cmd.usage.padEnd(30)}</Text>
							<Text dimColor>{cmd.description}</Text>
						</Box>
					))}
				</Box>
			))}
		</Box>
	);
}

interface ContextGridProps {
	readonly usedChars: number;
	readonly maxChars: number;
}

export function ContextGrid({ usedChars, maxChars }: ContextGridProps) {
	const ratio = Math.min(1, usedChars / maxChars);
	const pct = Math.round(ratio * 100);
	const width = 40;
	const filled = Math.round(ratio * width);

	const color = pct < 60 ? 'green' : pct < 85 ? 'yellow' : 'red';

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text>
				Context usage:{' '}
				<Text color={color} bold>
					{pct}%
				</Text>{' '}
				<Text dimColor>
					({usedChars.toLocaleString()} / {maxChars.toLocaleString()} chars)
				</Text>
			</Text>
			<Text color={color}>
				{'█'.repeat(filled)}
				{'░'.repeat(width - filled)}
			</Text>
		</Box>
	);
}
