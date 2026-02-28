import React from 'react';
import { Box, Text } from 'ink';
import type { CommandDefinition } from '../../ink-types.js';
import type { SessionStore, SessionMeta } from '../../session-store.js';

/** State accessors for session commands. */
export interface SessionCommandContext {
	readonly sessionStore: SessionStore;
	readonly getSessionId: () => string;
	readonly getServerName: () => string | undefined;
	readonly getModelName: () => string | undefined;
	readonly resumeSession: (sessionId: string) => void;
}

function SessionListView({
	sessions,
	currentId,
}: {
	sessions: readonly SessionMeta[];
	currentId: string;
}) {
	if (sessions.length === 0) {
		return React.createElement(
			Text,
			{ dimColor: true },
			'No saved sessions.',
		);
	}

	return React.createElement(
		Box,
		{ flexDirection: 'column', paddingX: 1 },
		React.createElement(Text, { bold: true, color: 'cyan' }, 'Sessions'),
		React.createElement(Text, null, ''),
		...sessions.slice(0, 20).map((s) =>
			React.createElement(
				Box,
				{ key: s.id, gap: 1 },
				React.createElement(
					Text,
					{
						color: s.id === currentId ? 'green' : undefined,
						bold: s.id === currentId,
					},
					s.id === currentId ? '\u25CF' : ' ',
				),
				React.createElement(
					Text,
					{ dimColor: true },
					s.id.slice(0, 12),
				),
				React.createElement(Text, null, s.title),
				React.createElement(
					Text,
					{ dimColor: true },
					`(${s.messageCount} msgs, ${new Date(s.updatedAt).toLocaleDateString()})`,
				),
			),
		),
		sessions.length > 20
			? React.createElement(
					Text,
					{ dimColor: true },
					`  ... and ${sessions.length - 20} more`,
				)
			: null,
		React.createElement(Text, null, ''),
		React.createElement(
			Text,
			{ dimColor: true },
			'Use /resume <id-prefix> to resume a session',
		),
	);
}

export function createSessionCommands(
	ctx: SessionCommandContext,
): readonly CommandDefinition[] {
	return [
		{
			name: 'sessions',
			aliases: ['ls'],
			usage: '/sessions',
			description: 'List saved sessions',
			category: 'session',
			execute: () => {
				const sessions = ctx.sessionStore.list();
				return {
					element: React.createElement(SessionListView, {
						sessions,
						currentId: ctx.getSessionId(),
					}),
				};
			},
		},
		{
			name: 'resume',
			aliases: ['r'],
			usage: '/resume <id-prefix>',
			description: 'Resume a previous session',
			category: 'session',
			execute: (args) => {
				const prefix = args.trim();
				if (!prefix) {
					return { text: 'Usage: /resume <session-id-prefix>' };
				}
				const sessions = ctx.sessionStore.list();
				const match = sessions.find((s) => s.id.startsWith(prefix));
				if (!match) {
					return {
						text: `No session found matching "${prefix}". Use /sessions to list.`,
					};
				}
				ctx.resumeSession(match.id);
				return {
					text: `Resumed session: ${match.title} (${match.messageCount} messages)`,
				};
			},
		},
		{
			name: 'rename',
			usage: '/rename <title>',
			description: 'Rename the current session',
			category: 'session',
			execute: (args) => {
				const title = args.trim();
				if (!title) return { text: 'Usage: /rename <title>' };
				ctx.sessionStore.rename(ctx.getSessionId(), title);
				return { text: `Session renamed to: ${title}` };
			},
		},
		{
			name: 'server',
			usage: '/server',
			description: 'Show the active ACP server',
			category: 'session',
			execute: () => ({
				text: `Server: ${ctx.getServerName() ?? '(none configured)'}`,
			}),
		},
		{
			name: 'model',
			usage: '/model',
			description: 'Show the active model',
			category: 'session',
			execute: () => ({
				text: `Model: ${ctx.getModelName() ?? '(default)'}`,
			}),
		},
		{
			name: 'mcp',
			usage: '/mcp',
			description: 'Show MCP connection status',
			category: 'session',
			execute: () => ({ text: 'MCP: not connected' }),
		},
		{
			name: 'acp',
			usage: '/acp',
			description: 'Show ACP connection status',
			category: 'session',
			execute: () => ({
				text: `ACP: ${ctx.getServerName() ? 'connected' : 'not connected'}`,
			}),
		},
	] as const;
}
