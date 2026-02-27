import type { CommandDefinition } from '../../ink-types.js';

export const toolsCommands: readonly CommandDefinition[] = [
	{
		name: 'tools',
		usage: '/tools',
		description: 'List available tools',
		category: 'tools',
		execute: () => ({ text: 'Listing tools...' }),
	},
	{
		name: 'agents',
		usage: '/agents',
		description: 'List available agents',
		category: 'tools',
		execute: () => ({ text: 'Listing agents...' }),
	},
	{
		name: 'skills',
		usage: '/skills',
		description: 'List available skills',
		category: 'tools',
		execute: () => ({ text: 'Listing skills...' }),
	},
];
