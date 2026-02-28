import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

/**
 * Wait for the app to be fully interactive. The status bar text
 * "? for shortcuts" only appears once the prompt is rendered and active.
 */
async function waitForReady(term: SimseTerminal): Promise<void> {
	await term.waitForText('? for shortcuts', { timeout: 15_000 });
}

describe('Banner E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// Banner at default width (120 cols)
	// -----------------------------------------------------------------
	it('banner renders at default width and contains "simse" text', async () => {
		term = await createSimseTerminal({
			acpBackend: 'none',
			cols: 120,
		});
		await waitForReady(term);

		expect(term.hasBanner()).toBe(true);

		const screen = term.getScreen();
		expect(screen).toContain('simse');
	}, 30_000);

	// -----------------------------------------------------------------
	// Banner at narrow width (80 cols)
	// -----------------------------------------------------------------
	it('banner renders at narrow width (80 cols) without crashing', async () => {
		term = await createSimseTerminal({
			acpBackend: 'none',
			cols: 80,
		});
		await waitForReady(term);

		// The app should start successfully at narrow width
		expect(term.hasBanner()).toBe(true);

		const screen = term.getScreen();
		// Should still contain the title text
		expect(screen).toContain('simse');
	}, 30_000);

	// -----------------------------------------------------------------
	// Banner contains mascot ASCII art
	// -----------------------------------------------------------------
	it('banner contains mascot ASCII art box character', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		const screen = term.getScreen();
		// The banner box uses ╭ (U+256D) as the top-left corner
		expect(screen).toContain('\u256D');
	}, 30_000);
}, 120_000);
