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

describe('Tools Commands E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// /tools
	// -----------------------------------------------------------------
	it('/tools lists available tools', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/tools');

		await term.waitForText('Listing tools', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Listing tools');
		expect(screen).not.toContain('Unknown command');
	}, 30_000);

	// -----------------------------------------------------------------
	// /agents
	// -----------------------------------------------------------------
	it('/agents lists available agents', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/agents');

		await term.waitForText('Listing agents', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Listing agents');
		expect(screen).not.toContain('Unknown command');
	}, 30_000);

	// -----------------------------------------------------------------
	// /skills
	// -----------------------------------------------------------------
	it('/skills lists available skills', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/skills');

		await term.waitForText('Listing skills', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Listing skills');
		expect(screen).not.toContain('Unknown command');
	}, 30_000);
}, 120_000);
