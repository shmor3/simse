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
			return { text: 'No chains configured. Define chains in your simse config to use this command.' };
		},
	},
	{
		name: 'prompts',
		usage: '/prompts',
		description: 'List available prompt templates',
		category: 'ai',
		execute: () => ({ text: 'No prompt templates configured. Define templates in your simse config.' }),
	},
];
