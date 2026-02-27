import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from './harness/index.js';

describe(
	'Startup E2E',
	() => {
		let term: SimseTerminal | undefined;

		afterEach(async () => {
			await term?.kill();
			term = undefined;
		});

		it(
			'starts without ACP and shows the banner',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await term.waitForPrompt({ timeout: 15_000 });

				expect(term.hasBanner()).toBe(true);

				// The banner contains "simse-code" title text
				const screen = term.getScreen();
				expect(screen).toContain('simse-code');
			},
			30_000,
		);

		it(
			'shows prompt ready for input',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await term.waitForPrompt({ timeout: 15_000 });

				// The prompt area contains the > symbol (chevron)
				const screen = term.getScreen();
				expect(screen).toContain('>');
			},
			30_000,
		);

		it(
			'shows info about missing ACP when none configured',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				// Wait for the status bar hint, which appears only when Ink is
				// fully rendered and the prompt is interactive.
				await term.waitForText('? for shortcuts', { timeout: 15_000 });

				// Send a non-command message to trigger the "no ACP" error.
				// Use type + pressKey separately for reliability.
				term.type('hi');
				await term.waitForText('hi', { timeout: 5_000 });
				term.pressKey('enter');

				await term.waitForText('No ACP server configured', {
					timeout: 15_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('No ACP server configured');
				expect(screen).toContain('/setup');
			},
			30_000,
		);
	},
	120_000,
);
