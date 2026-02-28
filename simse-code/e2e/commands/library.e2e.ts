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

describe('Library Commands E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// /add <topic> <text>
	// -----------------------------------------------------------------
	it('/add adds a volume to a topic', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/add testing Some volume text');

		await term.waitForText('Adding to', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Adding to');
		expect(screen).toContain('testing');
	}, 30_000);

	// -----------------------------------------------------------------
	// /add without text shows usage
	// -----------------------------------------------------------------
	it('/add without text shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/add topiconly');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/add');
	}, 30_000);

	// -----------------------------------------------------------------
	// /search <query>
	// -----------------------------------------------------------------
	it('/search performs semantic search', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/search test query');

		await term.waitForText('Searching for', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Searching for');
		expect(screen).toContain('test query');
	}, 30_000);

	// -----------------------------------------------------------------
	// /search without query shows usage
	// -----------------------------------------------------------------
	it('/search without query shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/search');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/search');
	}, 30_000);

	// -----------------------------------------------------------------
	// /topics
	// -----------------------------------------------------------------
	it('/topics lists all topics', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/topics');

		await term.waitForText('Listing topics', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Listing topics');
	}, 30_000);

	// -----------------------------------------------------------------
	// /volumes
	// -----------------------------------------------------------------
	it('/volumes lists all volumes', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/volumes');

		await term.waitForText('Listing all volumes', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Listing all volumes');
	}, 30_000);

	// -----------------------------------------------------------------
	// /volumes <topic>
	// -----------------------------------------------------------------
	it('/volumes with topic filters by topic', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/volumes testing');

		await term.waitForText('Volumes in', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Volumes in');
		expect(screen).toContain('testing');
	}, 30_000);

	// -----------------------------------------------------------------
	// /get <id>
	// -----------------------------------------------------------------
	it('/get retrieves a volume by ID', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/get 1');

		await term.waitForText('Getting volume', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Getting volume');
		expect(screen).toContain('1');
	}, 30_000);

	// -----------------------------------------------------------------
	// /get without ID shows usage
	// -----------------------------------------------------------------
	it('/get without ID shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/get');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/get');
	}, 30_000);

	// -----------------------------------------------------------------
	// /delete <id>
	// -----------------------------------------------------------------
	it('/delete deletes a volume by ID', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/delete 1');

		await term.waitForText('Deleting volume', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Deleting volume');
		expect(screen).toContain('1');
	}, 30_000);

	// -----------------------------------------------------------------
	// /delete without ID shows usage
	// -----------------------------------------------------------------
	it('/delete without ID shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/delete');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/delete');
	}, 30_000);

	// -----------------------------------------------------------------
	// /recommend <query>
	// -----------------------------------------------------------------
	it('/recommend gives recommendations', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/recommend test query');

		await term.waitForText('Recommending for', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Recommending for');
		expect(screen).toContain('test query');
	}, 30_000);

	// -----------------------------------------------------------------
	// /recommend without query shows usage
	// -----------------------------------------------------------------
	it('/recommend without query shows usage', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/recommend');

		await term.waitForText('Usage:', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Usage:');
		expect(screen).toContain('/recommend');
	}, 30_000);
}, 300_000);
