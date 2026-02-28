import { Box, Text } from 'ink';
import InkSpinner from 'ink-spinner';
import React from 'react';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ToolCallBoxProps {
	readonly name: string;
	readonly args: string;
	readonly status: 'active' | 'completed' | 'failed';
	readonly duration?: number;
	readonly summary?: string;
	readonly error?: string;
	readonly diff?: string;
}

// ---------------------------------------------------------------------------
// Tool display name map — Claude Code style
// ---------------------------------------------------------------------------

const TOOL_DISPLAY_NAMES: Readonly<Record<string, string>> = {
	bash: 'Bash',
	shell: 'Bash',
	exec: 'Bash',
	execute: 'Bash',
	run_command: 'Bash',
	vfs_write: 'Write',
	vfs_read: 'Read',
	vfs_delete: 'Delete',
	vfs_rename: 'Rename',
	vfs_list: 'List',
	vfs_stat: 'Stat',
	vfs_search: 'Search',
	vfs_diff: 'Diff',
	vfs_mkdir: 'Mkdir',
	file_write: 'Write',
	file_read: 'Read',
	file_edit: 'Update',
	file_create: 'Write',
	glob: 'Search',
	grep: 'Search',
	library_search: 'Search',
	library_shelve: 'Shelve',
	library_list: 'List',
	library_withdraw: 'Withdraw',
	task_list: 'TaskList',
	task_create: 'TaskCreate',
	task_update: 'TaskUpdate',
	task_get: 'TaskGet',
	task_delete: 'TaskDelete',
};

function getToolDisplayName(name: string): string {
	return (
		TOOL_DISPLAY_NAMES[name] ?? name.charAt(0).toUpperCase() + name.slice(1)
	);
}

// ---------------------------------------------------------------------------
// Primary arg extraction
// ---------------------------------------------------------------------------

const PRIMARY_ARG_KEYS = [
	'path',
	'file_path',
	'filePath',
	'filename',
	'command',
	'query',
	'pattern',
	'name',
	'url',
] as const;

function extractPrimaryArg(argsStr: string): string {
	try {
		const parsed = JSON.parse(argsStr) as Record<string, unknown>;
		for (const key of PRIMARY_ARG_KEYS) {
			const val = parsed[key];
			if (typeof val === 'string' && val.length > 0) {
				return val.length > 80 ? `${val.slice(0, 77)}...` : val;
			}
		}
		// Fall back to first string value
		for (const val of Object.values(parsed)) {
			if (typeof val === 'string' && val.length > 0) {
				return val.length > 60 ? `${val.slice(0, 57)}...` : val;
			}
		}
	} catch {
		if (argsStr && argsStr !== '{}') {
			return argsStr.length > 60 ? `${argsStr.slice(0, 57)}...` : argsStr;
		}
	}
	return '';
}

// ---------------------------------------------------------------------------
// Duration formatting
// ---------------------------------------------------------------------------

function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
	const mins = Math.floor(ms / 60_000);
	const secs = Math.round((ms % 60_000) / 1000);
	return `${mins}m${secs}s`;
}

// ---------------------------------------------------------------------------
// Status indicator
// ---------------------------------------------------------------------------

function StatusIndicator({ status }: { status: ToolCallBoxProps['status'] }) {
	switch (status) {
		case 'active':
			return (
				<Text color="magenta">
					<InkSpinner type="dots" />
				</Text>
			);
		case 'completed':
			return <Text color="magenta">⏺</Text>;
		case 'failed':
			return <Text color="red">⏺</Text>;
	}
}

// ---------------------------------------------------------------------------
// Diff rendering
// ---------------------------------------------------------------------------

function DiffLines({ diff }: { diff: string }) {
	const lines = diff.split('\n');
	return (
		<>
			{lines.map((line, i) => {
				if (line.startsWith('+')) {
					return (
						<Text key={i}>
							{'    '}
							<Text dimColor>{'⎿'}</Text> <Text color="green">{line}</Text>
						</Text>
					);
				}
				if (line.startsWith('-')) {
					return (
						<Text key={i}>
							{'    '}
							<Text dimColor>{'⎿'}</Text> <Text color="red">{line}</Text>
						</Text>
					);
				}
				return (
					<Text key={i}>
						{'    '}
						<Text dimColor>{'⎿'}</Text> {line}
					</Text>
				);
			})}
		</>
	);
}

// ---------------------------------------------------------------------------
// ToolCallBox component
// ---------------------------------------------------------------------------

export function ToolCallBox({
	name,
	args,
	status,
	duration,
	summary,
	error,
	diff,
}: ToolCallBoxProps) {
	const displayName = getToolDisplayName(name);
	const primaryArg = extractPrimaryArg(args);
	const toolLabel = primaryArg ? `${displayName}(${primaryArg})` : displayName;

	// Build result meta parts for the ⎿ line
	const resultParts: string[] = [];
	if (summary) resultParts.push(summary);
	if (duration !== undefined) resultParts.push(formatDuration(duration));
	const resultText = resultParts.length > 0 ? resultParts.join(' ') : undefined;

	return (
		<Box flexDirection="column" paddingLeft={2}>
			{/* Main line: status indicator + tool label */}
			<Box gap={1}>
				<StatusIndicator status={status} />
				<Text bold>{toolLabel}</Text>
			</Box>

			{/* Result summary line */}
			{status === 'completed' && resultText && (
				<Text>
					{'    '}
					<Text dimColor>{'⎿'}</Text>
					{'  '}
					<Text dimColor>{resultText}</Text>
				</Text>
			)}

			{/* Diff lines */}
			{diff && <DiffLines diff={diff} />}

			{/* Error line */}
			{error && (
				<Text>
					{'    '}
					<Text dimColor>{'⎿'}</Text>
					{'  '}
					<Text color="red">{error}</Text>
				</Text>
			)}
		</Box>
	);
}
