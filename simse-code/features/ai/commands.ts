import type { CommandDefinition } from '../../ink-types.js';

export const aiCommands: readonly CommandDefinition[] = [
	{
		name: 'chain',
		aliases: ['prompt'],
		usage: '/chain <name> [args]',
		description: 'Run a named chain or prompt template',
		category: 'ai',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /chain <name> [args]' };
			return { text: `Running chain "${args}"...` };
		},
	},
	{
		name: 'prompts',
		usage: '/prompts',
		description: 'List available prompt templates',
		category: 'ai',
		execute: () => ({ text: 'Listing prompt templates...' }),
	},
];
