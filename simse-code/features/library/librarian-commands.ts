import type { CommandDefinition } from '../../ink-types.js';

export function createLibrarianCommands(
	onShowLibrarianExplorer: () => Promise<void>,
): readonly CommandDefinition[] {
	return [
		{
			name: 'librarians',
			aliases: ['libs'],
			usage: '/librarians',
			description: 'Browse and manage librarians interactively',
			category: 'library',
			execute: async () => {
				await onShowLibrarianExplorer();
				return { text: '' };
			},
		},
	];
}
