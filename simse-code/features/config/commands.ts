import type { CommandDefinition } from '../../ink-types.js';

export const configCommands: readonly CommandDefinition[] = [
	{
		name: 'config',
		usage: '/config',
		description: 'Show current configuration',
		category: 'config',
		execute: () => ({ text: 'Showing configuration...' }),
	},
	{
		name: 'settings',
		aliases: ['set'],
		usage: '/settings [key] [value]',
		description: 'View or update settings',
		category: 'config',
		execute: (args) => {
			if (!args.trim()) return { text: 'Showing all settings...' };
			return { text: `Setting: ${args}` };
		},
	},
	{
		name: 'init',
		usage: '/init',
		description: 'Initialize a new simse project',
		category: 'config',
		execute: () => ({ text: 'Initializing project...' }),
	},
];
