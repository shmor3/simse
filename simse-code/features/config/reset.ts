import { rmSync } from 'node:fs';
import { join } from 'node:path';
import type { CommandDefinition } from '../../ink-types.js';

export function createResetCommands(
	dataDir: string,
	workDir: string,
	onConfirm: (message: string) => Promise<boolean>,
): readonly CommandDefinition[] {
	return [
		{
			name: 'factory-reset',
			usage: '/factory-reset',
			description: 'Delete all global configs, sessions, and memories',
			category: 'config',
			execute: async () => {
				const confirmed = await onConfirm(
					`This will permanently delete everything in ${dataDir}`,
				);
				if (!confirmed) {
					return { text: 'Factory reset cancelled.' };
				}
				rmSync(dataDir, { recursive: true, force: true });
				return {
					text: `Factory reset complete. Deleted ${dataDir}\nRestart simse to begin fresh.`,
				};
			},
		},
		{
			name: 'factory-reset-project',
			usage: '/factory-reset-project',
			description: 'Delete project-level .simse/ config and SIMSE.md',
			category: 'config',
			execute: async () => {
				const simseDir = join(workDir, '.simse');
				const simseMd = join(workDir, 'SIMSE.md');
				const confirmed = await onConfirm(
					`This will permanently delete ${simseDir} and ${simseMd}`,
				);
				if (!confirmed) {
					return { text: 'Project reset cancelled.' };
				}
				rmSync(simseDir, { recursive: true, force: true });
				rmSync(simseMd, { force: true });
				return {
					text: 'Project reset complete. Deleted .simse/ and SIMSE.md',
				};
			},
		},
	];
}
