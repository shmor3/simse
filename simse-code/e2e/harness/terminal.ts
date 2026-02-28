// simse-code/e2e/harness/terminal.ts

import { resolve } from 'node:path';
import { Terminal } from '@xterm/headless';

export interface PtyTerminalOptions {
	command: string;
	args?: string[];
	cols?: number;
	rows?: number;
	cwd?: string;
	env?: Record<string, string>;
}

export interface PtyTerminal {
	write(data: string): void;
	getScreen(): string;
	getLine(row: number): string;
	getCursorPosition(): { row: number; col: number };
	waitForText(text: string, opts?: { timeout?: number }): Promise<void>;
	waitForNoText(text: string, opts?: { timeout?: number }): Promise<void>;
	kill(): Promise<void>;
}

/**
 * Creates a PTY terminal by spawning a process in a real pseudo-terminal
 * and connecting it to an xterm.js headless terminal emulator.
 *
 * On Windows under Bun, node-pty's write pipe is broken (ERR_SOCKET_CLOSED),
 * so we use a Node.js bridge process that runs node-pty in a compatible
 * runtime and communicates via NDJSON over stdio.
 */
export async function createPtyTerminal(
	options: PtyTerminalOptions,
): Promise<PtyTerminal> {
	const cols = options.cols ?? 120;
	const rows = options.rows ?? 40;

	const vt = new Terminal({ cols, rows, allowProposedApi: true });

	// Use the bridge approach on Windows to work around Bun + node-pty
	// write pipe incompatibility.
	const useBridge = process.platform === 'win32';

	let writeFn: (data: string) => void;
	let killFn: () => Promise<void>;
	let exited = false;

	if (useBridge) {
		const bridgePath = resolve(import.meta.dir, 'pty-bridge.cjs');

		const config = JSON.stringify({
			command: options.command,
			args: options.args ?? [],
			cols,
			rows,
			cwd: options.cwd,
			env: options.env,
		});

		const proc = Bun.spawn(['node', bridgePath, config], {
			stdin: 'pipe',
			stdout: 'pipe',
			stderr: 'inherit',
		});

		// Read NDJSON from bridge stdout
		const reader = proc.stdout.getReader();
		let lineBuf = '';

		const readLoop = async () => {
			try {
				while (true) {
					const { done, value } = await reader.read();
					if (done) break;
					lineBuf += new TextDecoder().decode(value);
					let nl: number;
					while ((nl = lineBuf.indexOf('\n')) >= 0) {
						const line = lineBuf.slice(0, nl);
						lineBuf = lineBuf.slice(nl + 1);
						if (!line.trim()) continue;
						try {
							const msg = JSON.parse(line);
							if (msg.type === 'data') {
								vt.write(msg.data);
							} else if (msg.type === 'exit') {
								exited = true;
							}
						} catch {
							// Ignore parse errors
						}
					}
				}
			} catch {
				// Reader closed
			}
		};
		readLoop();

		// Wait for the bridge to signal ready
		await new Promise<void>((resolve, reject) => {
			const timeout = setTimeout(() => {
				reject(new Error('PTY bridge did not become ready in 10s'));
			}, 10_000);

			const check = setInterval(() => {
				// The bridge sends a "ready" message; we check if we got
				// any data from the terminal (which means the bridge is up)
				if (getScreen().trim().length > 0 || exited) {
					clearTimeout(timeout);
					clearInterval(check);
					resolve();
				}
			}, 50);
		});

		writeFn = (data: string) => {
			if (exited) return;
			try {
				proc.stdin.write(JSON.stringify({ type: 'write', data }) + '\n');
			} catch {
				// Stdin may be closed
			}
		};

		killFn = async () => {
			if (!exited) {
				try {
					proc.stdin.write(JSON.stringify({ type: 'kill' }) + '\n');
				} catch {
					// Ignore
				}
				// Give the bridge time to clean up
				await new Promise((r) => setTimeout(r, 200));
				try {
					proc.kill();
				} catch {
					// Already dead
				}
			}
			vt.dispose();
		};
	} else {
		// On non-Windows platforms, use node-pty directly
		const pty = await import('node-pty');

		const proc = pty.spawn(options.command, options.args ?? [], {
			cols,
			rows,
			cwd: options.cwd,
			env: { ...process.env, ...options.env } as Record<string, string>,
		});

		proc.onData((data: string) => {
			vt.write(data);
		});

		proc.onExit(() => {
			exited = true;
		});

		writeFn = (data: string) => {
			if (exited) return;
			try {
				proc.write(data);
			} catch {
				// Socket may already be closed
			}
		};

		killFn = async () => {
			if (!exited) {
				try {
					proc.kill();
				} catch {
					// Already dead
				}
			}
			vt.dispose();
		};
	}

	function getScreen(): string {
		const lines: string[] = [];
		const buf = vt.buffer.active;
		for (let i = 0; i < rows; i++) {
			const line = buf.getLine(i);
			lines.push(line ? line.translateToString(true) : '');
		}
		return lines.join('\n');
	}

	function getLine(row: number): string {
		const line = vt.buffer.active.getLine(row);
		return line ? line.translateToString(true) : '';
	}

	function getCursorPosition(): { row: number; col: number } {
		return {
			row: vt.buffer.active.cursorY,
			col: vt.buffer.active.cursorX,
		};
	}

	async function waitForText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void> {
		const timeout = opts?.timeout ?? 10_000;
		const start = Date.now();
		const pollMs = 100;

		while (Date.now() - start < timeout) {
			if (getScreen().includes(text)) return;
			await new Promise((r) => setTimeout(r, pollMs));
		}

		const screen = getScreen();
		throw new Error(
			`waitForText timed out after ${timeout}ms waiting for "${text}".\nScreen:\n${screen}`,
		);
	}

	async function waitForNoText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void> {
		const timeout = opts?.timeout ?? 10_000;
		const start = Date.now();
		const pollMs = 100;

		while (Date.now() - start < timeout) {
			if (!getScreen().includes(text)) return;
			await new Promise((r) => setTimeout(r, pollMs));
		}

		throw new Error(
			`waitForNoText timed out after ${timeout}ms — "${text}" still present.`,
		);
	}

	return {
		write: writeFn,
		getScreen,
		getLine,
		getCursorPosition,
		waitForText,
		waitForNoText,
		kill: killFn,
	};
}
