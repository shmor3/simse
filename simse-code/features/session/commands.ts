import type { CommandDefinition } from '../../ink-types.js';

export const sessionCommands: readonly CommandDefinition[] = [
	{
		name: 'server',
		usage: '/server [name]',
		description: 'Show or set the active ACP server',
		category: 'session',
		execute: (args) => ({
			text: args ? `Switching server to "${args}"...` : 'Current server: (none)',
		}),
	},
	{
		name: 'agent',
		usage: '/agent [name]',
		description: 'Show or set the active agent',
		category: 'session',
		execute: (args) => ({
			text: args ? `Switching agent to "${args}"...` : 'Current agent: (none)',
		}),
	},
	{
		name: 'model',
		usage: '/model [name]',
		description: 'Show or set the active model',
		category: 'session',
		execute: (args) => ({
			text: args ? `Switching model to "${args}"...` : 'Current model: (default)',
		}),
	},
	{
		name: 'mcp',
		usage: '/mcp',
		description: 'Show MCP connection status',
		category: 'session',
		execute: () => ({ text: 'MCP status: not connected' }),
	},
	{
		name: 'acp',
		usage: '/acp',
		description: 'Show ACP connection status',
		category: 'session',
		execute: () => ({ text: 'ACP status: not connected' }),
	},
	{
		name: 'library',
		aliases: ['memory'],
		usage: '/library [on|off]',
		description: 'Toggle library (memory) integration',
		category: 'session',
		execute: (args) => ({
			text: args ? `Library: ${args}` : 'Library: toggled',
		}),
	},
	{
		name: 'bypass-permissions',
		usage: '/bypass-permissions [on|off]',
		description: 'Toggle permission bypass mode',
		category: 'session',
		execute: (args) => ({
			text: args ? `Bypass permissions: ${args}` : 'Bypass permissions: toggled',
		}),
	},
	{
		name: 'embed',
		usage: '/embed <text>',
		description: 'Generate embeddings for text',
		category: 'session',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /embed <text>' };
			return { text: `Generating embeddings for "${args.slice(0, 50)}"...` };
		},
	},
];
