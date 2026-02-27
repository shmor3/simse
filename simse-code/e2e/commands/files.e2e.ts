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
	'Files Commands E2E',
	() => {
		let term: SimseTerminal | undefined;

		afterEach(async () => {
			await term?.kill();
			term = undefined;
		});

		// -----------------------------------------------------------------
		// /files
		// -----------------------------------------------------------------
		it(
			'/files lists virtual filesystem files',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/files');

				await term.waitForText('Listing files', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Listing files');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /files <path>
		// -----------------------------------------------------------------
		it(
			'/files with path lists files in that path',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/files src');

				await term.waitForText('Listing files', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Listing files');
				expect(screen).toContain('src');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /save
		// -----------------------------------------------------------------
		it(
			'/save saves VFS files to disk',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/save');

				await term.waitForText('Saving all files', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Saving all files');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /validate
		// -----------------------------------------------------------------
		it(
			'/validate validates VFS file contents',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/validate');

				await term.waitForText('Validating all files', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Validating all files');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /discard
		// -----------------------------------------------------------------
		it(
			'/discard discards VFS changes',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/discard');

				await term.waitForText('Discarding all changes', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Discarding all changes');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /diff
		// -----------------------------------------------------------------
		it(
			'/diff shows VFS file diffs',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/diff');

				await term.waitForText('Showing all diffs', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Showing all diffs');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);
	},
	240_000,
);
