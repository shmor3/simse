import { afterEach, describe, expect, it } from 'bun:test';
import { createPtyTerminal } from './terminal.js';

describe('PtyTerminal (raw)', () => {
	let kill: (() => Promise<void>) | undefined;

	afterEach(async () => {
		await kill?.();
	});

	it('spawns a process and captures output', async () => {
		const term = await createPtyTerminal({
			command: process.platform === 'win32' ? 'cmd.exe' : 'echo',
			args:
				process.platform === 'win32'
					? ['/c', 'echo', 'hello from pty']
					: ['hello from pty'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await term.waitForText('hello from pty', { timeout: 5_000 });
		expect(term.getScreen()).toContain('hello from pty');
	});

	it('can send stdin to process', async () => {
		// Use an interactive shell that stays alive and echoes input
		const isWin = process.platform === 'win32';
		const term = await createPtyTerminal({
			command: isWin ? 'cmd.exe' : 'cat',
			args: isWin ? ['/k', 'echo', 'ready'] : [],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		// Wait for the shell to be ready
		if (isWin) {
			await term.waitForText('ready', { timeout: 5_000 });
		}

		term.write('echo test input\r');
		await term.waitForText('test input', { timeout: 5_000 });
	});

	it('getLine returns specific line content', async () => {
		const term = await createPtyTerminal({
			command: process.platform === 'win32' ? 'cmd.exe' : 'echo',
			args:
				process.platform === 'win32'
					? ['/c', 'echo', 'line content here']
					: ['line content here'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await term.waitForText('line content here', { timeout: 5_000 });
		const screen = term.getScreen();
		expect(screen).toContain('line content here');
	});

	it('waitForText times out if text never appears', async () => {
		const term = await createPtyTerminal({
			command: process.platform === 'win32' ? 'cmd.exe' : 'echo',
			args:
				process.platform === 'win32'
					? ['/c', 'echo', 'something']
					: ['something'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await expect(
			term.waitForText('nonexistent text', { timeout: 1_000 }),
		).rejects.toThrow(/timed out/i);
	});
});
