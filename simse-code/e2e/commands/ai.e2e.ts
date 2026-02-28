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

describe('AI Commands E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// /prompts
	// -----------------------------------------------------------------
	it('/prompts lists prompt templates', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/prompts');

		await term.waitForText('Listing prompt templates', {
			timeout: 10_000,
		});

		const screen = term.getScreen();
		expect(screen).toContain('Listing prompt templates');
		expect(screen).not.toContain('Unknown command');
	}, 30_000);

	// -----------------------------------------------------------------
	// /chain without args shows usage
	// -----------------------------------------------------------------
	it('/chain without args shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/chain');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/chain');
	}, 30_000);

	// -----------------------------------------------------------------
	// /chain <name> runs a chain
	// -----------------------------------------------------------------
	it('/chain with name runs the chain', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/chain my-chain');

		await term.waitForText('Running chain', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Running chain');
		expect(screen).toContain('my-chain');
	}, 30_000);
}, 120_000);
