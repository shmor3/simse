import type { CommandDefinition } from '../../ink-types.js';

export const libraryCommands: readonly CommandDefinition[] = [
	{
		name: 'add',
		usage: '/add <topic> <text>',
		description: 'Add a volume to a topic',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /add <topic> <text>' };
			const spaceIdx = args.indexOf(' ');
			if (spaceIdx === -1) return { text: 'Usage: /add <topic> <text>' };
			return {
				text: 'Library not connected. Run /setup to configure an ACP server with library support.',
			};
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
			return {
				text: 'Library not connected. Run /setup to configure an ACP server with library support.',
			};
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
			return {
				text: 'Library not connected. Run /setup to configure an ACP server with library support.',
			};
		},
	},
	{
		name: 'topics',
		usage: '/topics',
		description: 'List all topics',
		category: 'library',
		execute: () => ({
			text: 'Library not connected. Run /setup to configure an ACP server with library support.',
		}),
	},
	{
		name: 'volumes',
		aliases: ['ls'],
		usage: '/volumes [topic]',
		description: 'List volumes (optionally filtered by topic)',
		category: 'library',
		execute: () => ({
			text: 'Library not connected. Run /setup to configure an ACP server with library support.',
		}),
	},
	{
		name: 'get',
		usage: '/get <id>',
		description: 'Get a volume by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /get <id>' };
			return {
				text: 'Library not connected. Run /setup to configure an ACP server with library support.',
			};
		},
	},
	{
		name: 'delete',
		aliases: ['rm'],
		usage: '/delete <id>',
		description: 'Delete a volume by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /delete <id>' };
			return {
				text: 'Library not connected. Run /setup to configure an ACP server with library support.',
			};
		},
	},
];
