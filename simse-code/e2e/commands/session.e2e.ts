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
	'Session Commands E2E',
	() => {
		let term: SimseTerminal | undefined;

		afterEach(async () => {
			await term?.kill();
			term = undefined;
		});

		// -----------------------------------------------------------------
		// /mcp
		// -----------------------------------------------------------------
		it(
			'/mcp shows MCP connection status',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/mcp');

				await term.waitForText('MCP status', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('MCP status');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /acp
		// -----------------------------------------------------------------
		it(
			'/acp shows ACP connection status',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/acp');

				await term.waitForText('ACP status', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('ACP status');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /library
		// -----------------------------------------------------------------
		it(
			'/library toggles library integration',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/library');

				await term.waitForText('Library:', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Library:');
				expect(screen).toContain('toggled');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /memory (alias for /library)
		// -----------------------------------------------------------------
		it(
			'/memory is an alias for /library',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/memory');

				await term.waitForText('Library:', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Library:');
				expect(screen).toContain('toggled');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /bypass-permissions
		// -----------------------------------------------------------------
		it(
			'/bypass-permissions toggles bypass mode',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/bypass-permissions');

				await term.waitForText('Bypass permissions:', {
					timeout: 10_000,
				});

				const screen = term.getScreen();
				expect(screen).toContain('Bypass permissions:');
				expect(screen).toContain('toggled');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /server
		// -----------------------------------------------------------------
		it(
			'/server shows current server',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/server');

				await term.waitForText('Current server', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Current server');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /agent
		// -----------------------------------------------------------------
		it(
			'/agent shows current agent',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/agent');

				await term.waitForText('Current agent', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Current agent');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);

		// -----------------------------------------------------------------
		// /model
		// -----------------------------------------------------------------
		it(
			'/model shows current model',
			async () => {
				term = await createSimseTerminal({ acpBackend: 'none' });
				await waitForReady(term);

				await submitCommand(term, '/model');

				await term.waitForText('Current model', { timeout: 10_000 });

				const screen = term.getScreen();
				expect(screen).toContain('Current model');
				expect(screen).not.toContain('Unknown command');
			},
			30_000,
		);
	},
	300_000,
);
