import type { CommandDefinition } from '../../ink-types.js';

export const filesCommands: readonly CommandDefinition[] = [
	{
		name: 'files',
		usage: '/files [path]',
		description: 'List files in the virtual filesystem',
		category: 'files',
		execute: (args) => ({
			text: args ? `Listing files in "${args}"...` : 'Listing files in /...',
		}),
	},
	{
		name: 'save',
		usage: '/save [path]',
		description: 'Save VFS files to disk',
		category: 'files',
		execute: (args) => ({
			text: args ? `Saving "${args}" to disk...` : 'Saving all files to disk...',
		}),
	},
	{
		name: 'validate',
		usage: '/validate [path]',
		description: 'Validate VFS file contents',
		category: 'files',
		execute: (args) => ({
			text: args ? `Validating "${args}"...` : 'Validating all files...',
		}),
	},
	{
		name: 'discard',
		usage: '/discard [path]',
		description: 'Discard VFS changes',
		category: 'files',
		execute: (args) => ({
			text: args ? `Discarding changes to "${args}"...` : 'Discarding all changes...',
		}),
	},
	{
		name: 'diff',
		usage: '/diff [path]',
		description: 'Show VFS file diffs',
		category: 'files',
		execute: (args) => ({
			text: args ? `Diff for "${args}"...` : 'Showing all diffs...',
		}),
	},
];
