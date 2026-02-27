import React from 'react';
import type { CommandDefinition } from '../../ink-types.js';
import { ContextGrid, HelpView } from './components.js';

export function createMetaCommands(
	getCommands: () => readonly CommandDefinition[],
): readonly CommandDefinition[] {
	return [
		{
			name: 'help',
			aliases: ['?'],
			usage: '/help',
			description: 'Show available commands',
			category: 'meta',
			execute: () => ({
				element: React.createElement(HelpView, { commands: getCommands() }),
			}),
		},
		{
			name: 'clear',
			usage: '/clear',
			description: 'Clear conversation history',
			category: 'meta',
			execute: () => ({ text: 'Conversation cleared.' }),
		},
		{
			name: 'verbose',
			aliases: ['v'],
			usage: '/verbose [on|off]',
			description: 'Toggle verbose output',
			category: 'meta',
			execute: (args) => ({ text: `Verbose mode: ${args || 'toggled'}` }),
		},
		{
			name: 'plan',
			usage: '/plan [on|off]',
			description: 'Toggle plan mode',
			category: 'meta',
			execute: (args) => ({ text: `Plan mode: ${args || 'toggled'}` }),
		},
		{
			name: 'context',
			usage: '/context',
			description: 'Show context window usage',
			category: 'meta',
			execute: () => ({
				element: React.createElement(ContextGrid, {
					usedChars: 0,
					maxChars: 200000,
				}),
			}),
		},
		{
			name: 'exit',
			aliases: ['quit', 'q'],
			usage: '/exit',
			description: 'Exit the application',
			category: 'meta',
			execute: () => undefined,
		},
	] as const;
}
