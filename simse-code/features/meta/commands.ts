import React from 'react';
import type { CommandDefinition } from '../../ink-types.js';
import { ContextGrid, HelpView } from './components.js';

/** State accessors the meta commands need from the app layer. */
export interface MetaCommandContext {
	readonly getCommands: () => readonly CommandDefinition[];
	readonly setVerbose: (on: boolean) => void;
	readonly getVerbose: () => boolean;
	readonly setPlanMode: (on: boolean) => void;
	readonly getPlanMode: () => boolean;
	readonly clearConversation: () => void;
	readonly getContextUsage: () => { usedChars: number; maxChars: number };
}

function parseBoolArg(args: string, current: boolean): boolean {
	const trimmed = args.trim().toLowerCase();
	if (trimmed === 'on' || trimmed === 'true' || trimmed === '1') return true;
	if (trimmed === 'off' || trimmed === 'false' || trimmed === '0') return false;
	return !current;
}

export function createMetaCommands(
	ctx: MetaCommandContext,
): readonly CommandDefinition[] {
	return [
		{
			name: 'help',
			aliases: ['?'],
			usage: '/help',
			description: 'Show available commands',
			category: 'meta',
			execute: () => ({
				element: React.createElement(HelpView, {
					commands: ctx.getCommands(),
				}),
			}),
		},
		{
			name: 'clear',
			usage: '/clear',
			description: 'Clear conversation history',
			category: 'meta',
			execute: () => {
				ctx.clearConversation();
				return { text: 'Conversation cleared.' };
			},
		},
		{
			name: 'verbose',
			aliases: ['v'],
			usage: '/verbose [on|off]',
			description: 'Toggle verbose output',
			category: 'meta',
			execute: (args) => {
				const next = parseBoolArg(args, ctx.getVerbose());
				ctx.setVerbose(next);
				return { text: `Verbose mode: ${next ? 'on' : 'off'}` };
			},
		},
		{
			name: 'plan',
			usage: '/plan [on|off]',
			description: 'Toggle plan mode (read-only)',
			category: 'meta',
			execute: (args) => {
				const next = parseBoolArg(args, ctx.getPlanMode());
				ctx.setPlanMode(next);
				return {
					text: `Plan mode: ${next ? 'on' : 'off'}${next ? ' (write tools disabled)' : ''}`,
				};
			},
		},
		{
			name: 'context',
			usage: '/context',
			description: 'Show context window usage',
			category: 'meta',
			execute: () => {
				const { usedChars, maxChars } = ctx.getContextUsage();
				return {
					element: React.createElement(ContextGrid, {
						usedChars,
						maxChars,
					}),
				};
			},
		},
		{
			name: 'compact',
			usage: '/compact',
			description: 'Compact conversation to free context',
			category: 'meta',
			execute: () => {
				// Compaction is handled by the agentic loop — this signals intent
				return {
					text: 'Context compaction is automatic when usage exceeds threshold. Use /clear to start fresh.',
				};
			},
		},
		{
			name: 'exit',
			aliases: ['quit', 'q'],
			usage: '/exit',
			description: 'Exit the application',
			category: 'meta',
			execute: () => {
				process.exit(0);
			},
		},
	] as const;
}
