import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal } from './simse-terminal.js';

describe('SimseTerminal', () => {
	let term: Awaited<ReturnType<typeof createSimseTerminal>> | undefined;

	afterEach(async () => {
		await term?.kill();
	});

	it('launches simse-code and shows the prompt', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });
		expect(term.getScreen()).toContain('>');
	}, 20_000);

	it('can type and submit a command', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		// Type first and verify text appears in input
		term.type('/clear');
		await term.waitForText('/clear', { timeout: 5_000 });

		// Then press enter to submit
		term.pressKey('enter');
		await term.waitForText('Conversation cleared', { timeout: 10_000 });
	}, 30_000);

	it('type() sends individual characters', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		term.type('/hel');
		await term.waitForText('/hel', { timeout: 5_000 });
	}, 25_000);

	it('pressKey sends special keys', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		term.type('abc');
		await term.waitForText('abc', { timeout: 5_000 });

		term.pressKey('backspace');
		// After backspace, 'abc' should become 'ab'
		await term.waitForNoText('abc', { timeout: 5_000 });
	}, 25_000);

	it('pressCtrl sends control characters', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		// Ctrl+C should not crash the app
		term.pressCtrl('c');
		// App should still be running
		await term.waitForPrompt({ timeout: 5_000 });
	}, 25_000);
});
