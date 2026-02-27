import type { CommandDefinition } from '../../ink-types.js';

export const libraryCommands: readonly CommandDefinition[] = [
	{
		name: 'add',
		usage: '/add <topic> <text>',
		description: 'Add a note to a topic',
		category: 'library',
		execute: (args) => {
			const spaceIdx = args.indexOf(' ');
			if (spaceIdx === -1) return { text: 'Usage: /add <topic> <text>' };
			return { text: `Adding to "${args.slice(0, spaceIdx)}"...` };
		},
	},
	{
		name: 'search',
		aliases: ['s'],
		usage: '/search <query>',
		description: 'Semantic search across library',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /search <query>' };
			return { text: `Searching for "${args}"...` };
		},
	},
	{
		name: 'recommend',
		aliases: ['rec'],
		usage: '/recommend <query>',
		description: 'Get recommendations weighted by recency/frequency',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /recommend <query>' };
			return { text: `Recommending for "${args}"...` };
		},
	},
	{
		name: 'topics',
		usage: '/topics',
		description: 'List all topics',
		category: 'library',
		execute: () => ({ text: 'Listing topics...' }),
	},
	{
		name: 'notes',
		aliases: ['ls'],
		usage: '/notes [topic]',
		description: 'List notes (optionally filtered by topic)',
		category: 'library',
		execute: (args) => ({ text: args ? `Notes in "${args}"...` : 'Listing all notes...' }),
	},
	{
		name: 'get',
		usage: '/get <id>',
		description: 'Get a note by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /get <id>' };
			return { text: `Getting note ${args}...` };
		},
	},
	{
		name: 'delete',
		aliases: ['rm'],
		usage: '/delete <id>',
		description: 'Delete a note by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /delete <id>' };
			return { text: `Deleting note ${args}...` };
		},
	},
];
