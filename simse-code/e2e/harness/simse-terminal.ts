// simse-code/e2e/harness/simse-terminal.ts
import { resolve } from 'node:path';
import { type ACPBackend, createTempConfig } from './config.js';
import { ctrlKey, KEYS, type KeyName } from './keys.js';
import { createPtyTerminal } from './terminal.js';

export interface SimseTerminalOptions {
	cols?: number;
	rows?: number;
	acpBackend?: ACPBackend;
	bypassPermissions?: boolean;
	timeout?: number;
	env?: Record<string, string>;
}

export interface SimseTerminal {
	type(text: string): void;
	submit(text: string): void;
	pressKey(key: KeyName): void;
	pressCtrl(char: string): void;

	getScreen(): string;
	getLine(row: number): string;
	getCursorPosition(): { row: number; col: number };

	waitForText(text: string, opts?: { timeout?: number }): Promise<void>;
	waitForNoText(text: string, opts?: { timeout?: number }): Promise<void>;
	waitForPrompt(opts?: { timeout?: number }): Promise<void>;
	waitForIdle(opts?: { timeout?: number }): Promise<void>;

	hasToolCallBox(name?: string): boolean;
	hasErrorBox(): boolean;
	hasSpinner(): boolean;
	hasBanner(): boolean;
	hasStatusBar(): boolean;
	hasPermissionDialog(): boolean;

	kill(): Promise<void>;
}

export async function createSimseTerminal(
	options?: SimseTerminalOptions,
): Promise<SimseTerminal> {
	const acpBackend = options?.acpBackend ?? 'none';
	const bypassPermissions = options?.bypassPermissions ?? true;
	const cols = options?.cols ?? 120;
	const rows = options?.rows ?? 40;

	const config = await createTempConfig(acpBackend);

	// Resolve the CLI entry point relative to this file
	const cliPath = resolve(import.meta.dir, '../../cli-ink.tsx');

	const args = ['run', cliPath, '--data-dir', config.dataDir];
	if (bypassPermissions) {
		args.push('--bypass-permissions');
	}

	// On Windows, node-pty requires the .exe extension to find executables
	const bunCmd = process.platform === 'win32' ? 'bun.exe' : 'bun';

	const pty = await createPtyTerminal({
		command: bunCmd,
		args,
		cols,
		rows,
		cwd: process.cwd(),
		env: {
			...options?.env,
			FORCE_COLOR: '1',
			TERM: 'xterm-256color',
		},
	});

	function type(text: string): void {
		// Write characters individually so Ink's input parser treats each
		// as a separate event (its parser batches non-escape characters
		// from a single chunk into one event).
		for (const ch of text) {
			pty.write(ch);
		}
	}

	function submit(text: string): void {
		// Write text characters and enter separately so Ink's input parser
		// recognizes \r as a distinct 'return' keypress event.
		type(text);
		pty.write(KEYS.enter);
	}

	function pressKey(key: KeyName): void {
		pty.write(KEYS[key]);
	}

	function pressCtrl(char: string): void {
		pty.write(ctrlKey(char));
	}

	async function waitForPrompt(opts?: { timeout?: number }): Promise<void> {
		await pty.waitForText('>', opts);
	}

	async function waitForIdle(opts?: { timeout?: number }): Promise<void> {
		const timeout = opts?.timeout ?? 30_000;
		const start = Date.now();
		const pollMs = 200;

		while (Date.now() - start < timeout) {
			const screen = pty.getScreen();
			const hasPrompt = screen.includes('>');
			const spinnerChars = [
				'\u280B',
				'\u2819',
				'\u2839',
				'\u2838',
				'\u283C',
				'\u2834',
				'\u2826',
				'\u2827',
				'\u2807',
				'\u280F',
			];
			const hasSpinnerChar = spinnerChars.some((c) => screen.includes(c));
			if (hasPrompt && !hasSpinnerChar) return;
			await new Promise((r) => setTimeout(r, pollMs));
		}

		throw new Error(
			`waitForIdle timed out after ${timeout}ms.\nScreen:\n${pty.getScreen()}`,
		);
	}

	function hasToolCallBox(name?: string): boolean {
		const screen = pty.getScreen();
		if (name) return screen.includes(name);
		return (
			screen.includes('\u2502') &&
			(screen.includes('\u2713') || screen.includes('\u2717'))
		);
	}

	function hasErrorBox(): boolean {
		const screen = pty.getScreen();
		return screen.includes('Error') || screen.includes('\u2717');
	}

	function hasSpinner(): boolean {
		const screen = pty.getScreen();
		const spinnerChars = [
			'\u280B',
			'\u2819',
			'\u2839',
			'\u2838',
			'\u283C',
			'\u2834',
			'\u2826',
			'\u2827',
			'\u2807',
			'\u280F',
		];
		return spinnerChars.some((c) => screen.includes(c));
	}

	function hasBanner(): boolean {
		const screen = pty.getScreen();
		return screen.includes('simse') || screen.includes('\u256D');
	}

	function hasStatusBar(): boolean {
		const screen = pty.getScreen();
		return screen.includes('tokens') || screen.includes('server');
	}

	function hasPermissionDialog(): boolean {
		const screen = pty.getScreen();
		return screen.includes('Allow') && screen.includes('Deny');
	}

	async function kill(): Promise<void> {
		await pty.kill();
		await config.cleanup();
	}

	return {
		type,
		submit,
		pressKey,
		pressCtrl,
		getScreen: () => pty.getScreen(),
		getLine: (row: number) => pty.getLine(row),
		getCursorPosition: () => pty.getCursorPosition(),
		waitForText: (text, opts) => pty.waitForText(text, opts),
		waitForNoText: (text, opts) => pty.waitForNoText(text, opts),
		waitForPrompt,
		waitForIdle,
		hasToolCallBox,
		hasErrorBox,
		hasSpinner,
		hasBanner,
		hasStatusBar,
		hasPermissionDialog,
		kill,
	};
}
