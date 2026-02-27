import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

/**
 * Wait for the app to be fully interactive. The status bar text
 * "? for shortcuts" only appears once the prompt is rendered and active.
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
	'Error States E2E',
	() => {
		let term: SimseTerminal | undefined;

		afterEach(async () => {
			await term?.kill();
			term = undefined;
		});

		// -----------------------------------------------------------------
		// Invalid command shows error
		// -----------------------------------------------------------------
		it(
			'invalid command shows "Unknown command" error',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/nonexistent');

				await term.waitForText('Unknown command', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// Empty prompt submission doesn't crash
		// -----------------------------------------------------------------
		it(
			'empty prompt submission keeps prompt active',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				// Press Enter without typing anything
				term.pressKey('enter');

				// Wait a moment for the app to process the empty submission
				await new Promise((r) => setTimeout(r, 2_000));

				// The prompt should still be active — the status bar hint
				// should still be visible
				const screen = term.getScreen();
				expect(screen).toContain('>');
				// The app should not have crashed — "? for shortcuts" should
				// still be rendered
				expect(screen).toContain('? for shortcuts');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// Very long input doesn't crash
		// -----------------------------------------------------------------
		it(
			'very long input (300+ chars) does not crash the app',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				// Generate a 320-character string
				const longInput = 'A'.repeat(320);
				term.type(longInput);

				// Give the terminal time to process all characters
				await new Promise((r) => setTimeout(r, 3_000));

				// The app should still be alive and rendering. The prompt
				// area should still show '>' and the status bar should be intact.
				const screen = term.getScreen();
				expect(screen).toContain('>');
				// At least a portion of the typed text should be visible
				// (the input may wrap or scroll, but some 'A' chars should appear)
				expect(screen).toContain('AAAA');
			},
			30_000,
		);
	},
	120_000,
);
