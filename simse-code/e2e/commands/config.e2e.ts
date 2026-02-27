import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

/**
 * Wait for the app to be fully interactive.
 */
async function waitForReady(term: SimseTerminal): Promise<void> {
	await term.waitForText('? for shortcuts', { timeout: 15_000 });
}

/**
 * Type a slash command and submit it.
 */
async function submitCommand(
	term: SimseTerminal,
	command: string,
): Promise<void> {
	term.type(command);
	await new Promise((r) => setTimeout(r, 500));
	term.pressKey('enter');
}

describe(
	'Config Commands E2E',
	() => {
		let term: SimseTerminal | undefined;

		afterEach(async () => {
			await term?.kill();
			term = undefined;
		});

		// -----------------------------------------------------------------
		// /config
		// -----------------------------------------------------------------
		it(
			'/config shows current configuration',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/config');

				await term.waitForText('Showing configuration', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Showing configuration');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /settings
		// -----------------------------------------------------------------
		it(
			'/settings shows all settings',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/settings');

				await term.waitForText('Showing all settings', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Showing all settings');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /settings <key> <value>
		// -----------------------------------------------------------------
		it(
			'/settings with key value sets a setting',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/settings verbose true');

				await term.waitForText('Setting:', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Setting:');
				expect(screen).toContain('verbose true');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /init
		// -----------------------------------------------------------------
		it(
			'/init shows setup instructions',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/init');

				await term.waitForText('/setup', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('/setup');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /setup (no args — lists presets)
		// -----------------------------------------------------------------
		it(
			'/setup lists available presets',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/setup');

				await term.waitForText('Available presets', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Available presets');
				expect(screen).toContain('claude-code');
				expect(screen).toContain('ollama');
				expect(screen).toContain('copilot');
			},
			30_000,
		);
	},
	180_000,
);
