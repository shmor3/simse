import type { CommandDefinition } from '../../ink-types.js';

export const filesCommands: readonly CommandDefinition[] = [
	{
		name: 'files',
		usage: '/files [path]',
		description: 'List files in the virtual filesystem',
		category: 'files',
		execute: () => ({
			text: 'VFS not initialized. Files will be available when an ACP agent uses the virtual filesystem.',
		}),
	},
	{
		name: 'save',
		usage: '/save [path]',
		description: 'Save VFS files to disk',
		category: 'files',
		execute: () => ({
			text: 'VFS not initialized. Nothing to save.',
		}),
	},
	{
		name: 'validate',
		usage: '/validate [path]',
		description: 'Validate VFS file contents',
		category: 'files',
		execute: () => ({
			text: 'VFS not initialized. Nothing to validate.',
		}),
	},
	{
		name: 'discard',
		usage: '/discard [path]',
		description: 'Discard VFS changes',
		category: 'files',
		execute: () => ({
			text: 'VFS not initialized. Nothing to discard.',
		}),
	},
	{
		name: 'diff',
		usage: '/diff [path]',
		description: 'Show VFS file diffs',
		category: 'files',
		execute: () => ({
			text: 'VFS not initialized. No diffs available.',
		}),
	},
];
