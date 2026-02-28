import type { CommandDefinition } from '../../ink-types.js';

export function createSettingsCommands(
	dataDir: string,
	onShowSettingsExplorer?: () => Promise<void>,
): readonly CommandDefinition[] {
	return [
		{
			name: 'config',
			usage: '/config',
			description: 'Show current configuration',
			category: 'config',
			execute: () => ({ text: 'Showing configuration...' }),
		},
		{
			name: 'settings',
			aliases: ['set'],
			usage: '/settings',
			description: 'Browse and edit settings interactively',
			category: 'config',
			execute: async () => {
				if (onShowSettingsExplorer) {
					await onShowSettingsExplorer();
					return { text: '' };
				}
				return {
					text:
						'Interactive settings not available. Edit config files directly in ' +
						dataDir,
				};
			},
		},
	];
}
