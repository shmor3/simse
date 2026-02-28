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
 * Type a slash command and submit it. After typing, we wait for the
 * autocomplete dropdown to appear (indicated by the command description),
 * then press enter to submit.
 */
async function submitCommand(
	term: SimseTerminal,
	command: string,
): Promise<void> {
	term.type(command);
	// Give Ink time to process the character-by-character input and
	// render the autocomplete dropdown or input display.
	await new Promise((r) => setTimeout(r, 500));
	term.pressKey('enter');
}

describe('Meta Commands E2E', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
		term = undefined;
	});

	// -----------------------------------------------------------------
	// /help
	// -----------------------------------------------------------------
	it('/help lists command categories and commands', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/help');

		// The help view renders category labels. "General" is the
		// meta commands category header.
		await term.waitForText('General', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('General');
		// Should show command entries (usage strings)
		expect(screen).toContain('/clear');
		expect(screen).toContain('/exit');
	}, 30_000);

	// -----------------------------------------------------------------
	// /help search (with argument)
	// -----------------------------------------------------------------
	it('/help search shows help output even with argument', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/help search');

		// The help command ignores the argument and shows all commands
		await term.waitForText('General', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('General');
	}, 30_000);

	// -----------------------------------------------------------------
	// /clear
	// -----------------------------------------------------------------
	it('/clear clears conversation history', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/clear');

		await term.waitForText('Conversation cleared', {
			timeout: 10_000,
		});

		const screen = term.getScreen();
		expect(screen).toContain('Conversation cleared');
	}, 30_000);

	// -----------------------------------------------------------------
	// /verbose
	// -----------------------------------------------------------------
	it('/verbose toggles verbose mode', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/verbose');

		await term.waitForText('Verbose mode', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Verbose mode');
		expect(screen).toContain('toggled');
	}, 30_000);

	// -----------------------------------------------------------------
	// /plan
	// -----------------------------------------------------------------
	it('/plan toggles plan mode', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/plan');

		await term.waitForText('Plan mode', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Plan mode');
		expect(screen).toContain('toggled');
	}, 30_000);

	// -----------------------------------------------------------------
	// /context
	// -----------------------------------------------------------------
	it('/context shows context stats', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/context');

		await term.waitForText('Context usage', { timeout: 10_000 });

		const screen = term.getScreen();
		expect(screen).toContain('Context usage');
		// The context grid shows percentage
		expect(screen).toContain('%');
	}, 30_000);

	// -----------------------------------------------------------------
	// /exit
	// -----------------------------------------------------------------
	it('/exit is a recognized command', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await waitForReady(term);

		await submitCommand(term, '/exit');

		// /exit returns undefined (no output), so we verify it does NOT
		// produce an "Unknown command" error message.
		await new Promise((r) => setTimeout(r, 2_000));

		const screen = term.getScreen();
		expect(screen).not.toContain('Unknown command');
	}, 30_000);
}, 120_000);
