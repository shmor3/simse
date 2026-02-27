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
		aliases: ['initialize'],
		usage: '/init',
		description: 'Initialize a new simse project (alias for /setup)',
		category: 'config',
		execute: () => ({
			text: 'Use /setup to configure simse.\n  Examples: /setup claude-code, /setup ollama, /setup copilot\n  Run /setup with no args to see all options.',
		}),
	},
];
