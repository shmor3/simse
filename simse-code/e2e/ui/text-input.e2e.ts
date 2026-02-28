import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

/**
 * Wait for the app to be fully interactive. The status bar text
 * "? for shortcuts" only appears once the prompt is rendered and active.
 */
async function waitForReady(term: SimseTerminal): Promise<void> {
	await term.waitForText('? for shortcuts', { timeout: 15_000 });
}

describe('TextInput E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// Basic typing
	// -----------------------------------------------------------------
	it('typed text appears on screen', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		term.type('hello');
		await term.waitForText('hello', { timeout: 5_000 });

		const screen = term.getScreen();
		expect(screen).toContain('hello');
	}, 30_000);

	// -----------------------------------------------------------------
	// Arrow keys move cursor for insertion
	// -----------------------------------------------------------------
	it('arrow keys move cursor to allow mid-text insertion', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		// Type "hllo" (missing the 'e')
		term.type('hllo');
		await term.waitForText('hllo', { timeout: 5_000 });

		// Move left 3 positions to place cursor after 'h'
		term.pressKey('left');
		term.pressKey('left');
		term.pressKey('left');

		// Small delay for cursor movement to register
		await new Promise((r) => setTimeout(r, 300));

		// Insert 'e'
		term.type('e');
		await term.waitForText('hello', { timeout: 5_000 });

		const screen = term.getScreen();
		expect(screen).toContain('hello');
	}, 30_000);

	// -----------------------------------------------------------------
	// Backspace deletes a character
	// -----------------------------------------------------------------
	it('backspace deletes the character before the cursor', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		term.type('helloo');
		await term.waitForText('helloo', { timeout: 5_000 });

		term.pressKey('backspace');

		// After deleting the extra 'o', "hello" should remain.
		// Wait a moment for the deletion to render.
		await new Promise((r) => setTimeout(r, 500));

		const screen = term.getScreen();
		expect(screen).toContain('hello');
		// The extra 'o' should be gone — "helloo" should no longer appear
		expect(screen).not.toContain('helloo');
	}, 30_000);

	// -----------------------------------------------------------------
	// Rapid consecutive typing
	// -----------------------------------------------------------------
	it('rapid consecutive typing renders all characters', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		const longText = 'the quick brown fox jumps over the lazy dog 1234567890';
		term.type(longText);
		await term.waitForText(longText, { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain(longText);
	}, 30_000);
}, 120_000);
