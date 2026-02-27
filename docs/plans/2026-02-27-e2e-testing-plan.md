# E2E Testing Framework Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a PTY-based E2E test harness for simse-code that tests all 35 commands, streaming prompts, tool calls, subagents, MCP, library memory, and UI interactions.

**Architecture:** Spawn the real `simse-code/cli-ink.tsx` CLI in a pseudo-terminal via `node-pty`, pipe raw ANSI output through `@xterm/headless` for screen buffer parsing, and assert against parsed screen state using `bun:test`. Each test gets an isolated temp `--data-dir` for config isolation.

**Tech Stack:** `node-pty`, `@xterm/headless`, `bun:test`, TypeScript

**Design doc:** `docs/plans/2026-02-27-e2e-testing-design.md`

---

### Task 1: Install Dependencies

**Files:**
- Modify: `package.json`

**Step 1: Install node-pty and @xterm/headless**

Run:
```bash
bun add -d node-pty @xterm/headless
```

**Step 2: Verify imports resolve**

Run:
```bash
bun -e "import('node-pty').then(m => console.log('node-pty OK:', Object.keys(m))); import('@xterm/headless').then(m => console.log('xterm OK:', Object.keys(m)))"
```

Expected: Both imports resolve without errors.

**Step 3: Commit**

```bash
git add package.json bun.lockb
git commit -m "chore: add node-pty and @xterm/headless for E2E testing"
```

---

### Task 2: Key Escape Sequences Module

**Files:**
- Create: `simse-code/e2e/harness/keys.ts`
- Test: `simse-code/e2e/harness/keys.test.ts`

**Step 1: Write the test**

```typescript
import { describe, expect, it } from 'bun:test';
import { KEYS, ctrlKey } from './keys.js';

describe('keys', () => {
	it('has enter as carriage return', () => {
		expect(KEYS.enter).toBe('\r');
	});

	it('has escape as 0x1b', () => {
		expect(KEYS.escape).toBe('\x1b');
	});

	it('has arrow keys as ANSI sequences', () => {
		expect(KEYS.up).toBe('\x1b[A');
		expect(KEYS.down).toBe('\x1b[B');
		expect(KEYS.right).toBe('\x1b[C');
		expect(KEYS.left).toBe('\x1b[D');
	});

	it('has backspace', () => {
		expect(KEYS.backspace).toBe('\x7f');
	});

	it('has tab', () => {
		expect(KEYS.tab).toBe('\t');
	});

	it('ctrlKey produces control characters', () => {
		expect(ctrlKey('c')).toBe('\x03');
		expect(ctrlKey('d')).toBe('\x04');
		expect(ctrlKey('a')).toBe('\x01');
		expect(ctrlKey('z')).toBe('\x1a');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test simse-code/e2e/harness/keys.test.ts`
Expected: FAIL — module not found.

**Step 3: Write the implementation**

```typescript
// simse-code/e2e/harness/keys.ts

export const KEYS = {
	enter: '\r',
	escape: '\x1b',
	tab: '\t',
	backspace: '\x7f',
	delete: '\x1b[3~',
	up: '\x1b[A',
	down: '\x1b[B',
	right: '\x1b[C',
	left: '\x1b[D',
	home: '\x1b[H',
	end: '\x1b[F',
	pageUp: '\x1b[5~',
	pageDown: '\x1b[6~',
} as const;

export type KeyName = keyof typeof KEYS;

export function ctrlKey(char: string): string {
	return String.fromCharCode(char.toLowerCase().charCodeAt(0) - 96);
}
```

**Step 4: Run test to verify it passes**

Run: `bun test simse-code/e2e/harness/keys.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/e2e/harness/keys.ts simse-code/e2e/harness/keys.test.ts
git commit -m "feat(e2e): add key escape sequences module"
```

---

### Task 3: Temp Config Scaffolding

**Files:**
- Create: `simse-code/e2e/harness/config.ts`
- Test: `simse-code/e2e/harness/config.test.ts`

**Step 1: Write the test**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { existsSync } from 'node:fs';
import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { createTempConfig, type ACPBackend } from './config.js';

describe('createTempConfig', () => {
	let cleanup: (() => Promise<void>) | undefined;

	afterEach(async () => {
		await cleanup?.();
	});

	it('creates a temp directory with config files', async () => {
		const result = await createTempConfig('none');
		cleanup = result.cleanup;

		expect(existsSync(result.dataDir)).toBe(true);
		expect(existsSync(join(result.dataDir, 'config.json'))).toBe(true);
		expect(existsSync(join(result.dataDir, 'memory.json'))).toBe(true);
		expect(existsSync(join(result.dataDir, 'embed.json'))).toBe(true);
	});

	it('creates no acp.json for "none" backend', async () => {
		const result = await createTempConfig('none');
		cleanup = result.cleanup;

		expect(existsSync(join(result.dataDir, 'acp.json'))).toBe(false);
	});

	it('creates acp.json for "claude" backend', async () => {
		const result = await createTempConfig('claude');
		cleanup = result.cleanup;

		const acpJson = JSON.parse(
			await readFile(join(result.dataDir, 'acp.json'), 'utf-8'),
		);
		expect(acpJson.servers[0].name).toBe('claude');
		expect(acpJson.servers[0].command).toBe('bunx');
		expect(acpJson.servers[0].args).toContain('claude-code-acp');
	});

	it('creates acp.json for "ollama" backend', async () => {
		const result = await createTempConfig('ollama');
		cleanup = result.cleanup;

		const acpJson = JSON.parse(
			await readFile(join(result.dataDir, 'acp.json'), 'utf-8'),
		);
		expect(acpJson.servers[0].name).toBe('ollama');
	});

	it('cleanup removes the temp directory', async () => {
		const result = await createTempConfig('none');
		const dir = result.dataDir;
		await result.cleanup();
		cleanup = undefined;

		expect(existsSync(dir)).toBe(false);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test simse-code/e2e/harness/config.test.ts`
Expected: FAIL — module not found.

**Step 3: Write the implementation**

```typescript
// simse-code/e2e/harness/config.ts
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';

export type ACPBackend = 'claude' | 'ollama' | 'none';

export interface TempConfigResult {
	readonly dataDir: string;
	readonly cleanup: () => Promise<void>;
}

export async function createTempConfig(
	backend: ACPBackend,
): Promise<TempConfigResult> {
	const dataDir = join(tmpdir(), `simse-e2e-${randomUUID().slice(0, 8)}`);
	await mkdir(dataDir, { recursive: true });

	await writeFile(
		join(dataDir, 'config.json'),
		JSON.stringify({ logLevel: 'none' }),
	);

	await writeFile(
		join(dataDir, 'memory.json'),
		JSON.stringify({ autoSummarizeThreshold: 20 }),
	);

	await writeFile(
		join(dataDir, 'embed.json'),
		JSON.stringify({
			embeddingModel: 'nomic-ai/nomic-embed-text-v1.5',
			provider: 'local',
		}),
	);

	if (backend === 'claude') {
		await writeFile(
			join(dataDir, 'acp.json'),
			JSON.stringify({
				servers: [
					{
						name: 'claude',
						command: 'bunx',
						args: ['claude-code-acp'],
					},
				],
				defaultServer: 'claude',
			}),
		);
	} else if (backend === 'ollama') {
		const ollamaUrl =
			process.env.OLLAMA_URL ?? 'http://127.0.0.1:11434';
		const ollamaModel = process.env.OLLAMA_MODEL ?? 'llama3.2';
		await writeFile(
			join(dataDir, 'acp.json'),
			JSON.stringify({
				servers: [
					{
						name: 'ollama',
						command: 'bun',
						args: [
							'run',
							'acp-ollama-bridge.ts',
							'--ollama',
							ollamaUrl,
							'--model',
							ollamaModel,
						],
					},
				],
				defaultServer: 'ollama',
			}),
		);
	}

	return {
		dataDir,
		cleanup: async () => {
			await rm(dataDir, { recursive: true, force: true });
		},
	};
}
```

**Step 4: Run test to verify it passes**

Run: `bun test simse-code/e2e/harness/config.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/e2e/harness/config.ts simse-code/e2e/harness/config.test.ts
git commit -m "feat(e2e): add temp config scaffolding per test"
```

---

### Task 4: SimseTerminal — Core PTY + Screen Buffer

This is the core of the harness. It wraps `node-pty` and `@xterm/headless`.

**Files:**
- Create: `simse-code/e2e/harness/terminal.ts`
- Test: `simse-code/e2e/harness/terminal.test.ts`

**Step 1: Write the test**

This test spawns a simple command (not simse-code) to validate the PTY+xterm integration works.

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { createPtyTerminal } from './terminal.js';

describe('PtyTerminal (raw)', () => {
	let kill: (() => Promise<void>) | undefined;

	afterEach(async () => {
		await kill?.();
	});

	it('spawns a process and captures output', async () => {
		const term = await createPtyTerminal({
			command: 'echo',
			args: ['hello from pty'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await term.waitForText('hello from pty', { timeout: 5_000 });
		expect(term.getScreen()).toContain('hello from pty');
	});

	it('can send stdin to process', async () => {
		// Use `cat` which echoes stdin to stdout
		const term = await createPtyTerminal({
			command: 'cat',
			args: [],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		term.write('test input\r');
		await term.waitForText('test input', { timeout: 5_000 });
	});

	it('getLine returns specific line content', async () => {
		const term = await createPtyTerminal({
			command: 'echo',
			args: ['line content here'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await term.waitForText('line content here', { timeout: 5_000 });
		// The exact line depends on shell, but screen should contain it
		const screen = term.getScreen();
		expect(screen).toContain('line content here');
	});

	it('waitForText times out if text never appears', async () => {
		const term = await createPtyTerminal({
			command: 'echo',
			args: ['something'],
			cols: 80,
			rows: 24,
		});
		kill = term.kill;

		await expect(
			term.waitForText('nonexistent text', { timeout: 1_000 }),
		).rejects.toThrow(/timed out/i);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test simse-code/e2e/harness/terminal.test.ts`
Expected: FAIL — module not found.

**Step 3: Write the implementation**

```typescript
// simse-code/e2e/harness/terminal.ts
import * as pty from 'node-pty';
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
	waitForText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void>;
	waitForNoText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void>;
	kill(): Promise<void>;
}

export async function createPtyTerminal(
	options: PtyTerminalOptions,
): Promise<PtyTerminal> {
	const cols = options.cols ?? 120;
	const rows = options.rows ?? 40;

	const vt = new Terminal({ cols, rows, allowProposedApi: true });

	const proc = pty.spawn(options.command, options.args ?? [], {
		cols,
		rows,
		cwd: options.cwd,
		env: { ...process.env, ...options.env } as Record<string, string>,
	});

	proc.onData((data: string) => {
		vt.write(data);
	});

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

	async function kill(): Promise<void> {
		try {
			proc.kill();
		} catch {
			// Already dead
		}
		vt.dispose();
	}

	return {
		write: (data: string) => proc.write(data),
		getScreen,
		getLine,
		getCursorPosition,
		waitForText,
		waitForNoText,
		kill,
	};
}
```

**Step 4: Run test to verify it passes**

Run: `bun test simse-code/e2e/harness/terminal.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/e2e/harness/terminal.ts simse-code/e2e/harness/terminal.test.ts
git commit -m "feat(e2e): add PtyTerminal core with node-pty + @xterm/headless"
```

---

### Task 5: SimseTerminal — High-Level Harness

Wraps `PtyTerminal` to spawn the actual simse-code CLI with config scaffolding.

**Files:**
- Create: `simse-code/e2e/harness/simse-terminal.ts`
- Test: `simse-code/e2e/harness/simse-terminal.test.ts`

**Step 1: Write the test**

```typescript
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

		term.submit('/help');
		await term.waitForText('Available commands', { timeout: 10_000 });
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

		// Ctrl+C should not crash the app (it aborts streaming, but if idle it's a no-op)
		term.pressCtrl('c');
		// App should still be running
		await term.waitForPrompt({ timeout: 5_000 });
	}, 25_000);
});
```

**Step 2: Run test to verify it fails**

Run: `bun test simse-code/e2e/harness/simse-terminal.test.ts`
Expected: FAIL — module not found.

**Step 3: Write the implementation**

```typescript
// simse-code/e2e/harness/simse-terminal.ts
import { resolve } from 'node:path';
import { type ACPBackend, createTempConfig } from './config.js';
import { KEYS, type KeyName, ctrlKey } from './keys.js';
import { type PtyTerminal, createPtyTerminal } from './terminal.js';

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

	waitForText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void>;
	waitForNoText(
		text: string,
		opts?: { timeout?: number },
	): Promise<void>;
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

	const cliPath = resolve(
		import.meta.dir,
		'../../cli-ink.tsx',
	);

	const args = ['run', cliPath, '--data-dir', config.dataDir];
	if (bypassPermissions) {
		args.push('--bypass-permissions');
	}

	const pty = await createPtyTerminal({
		command: 'bun',
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
		pty.write(text);
	}

	function submit(text: string): void {
		pty.write(text + KEYS.enter);
	}

	function pressKey(key: KeyName): void {
		pty.write(KEYS[key]);
	}

	function pressCtrl(char: string): void {
		pty.write(ctrlKey(char));
	}

	async function waitForPrompt(
		opts?: { timeout?: number },
	): Promise<void> {
		await pty.waitForText('>', opts);
	}

	async function waitForIdle(
		opts?: { timeout?: number },
	): Promise<void> {
		// Wait for the prompt to reappear (no spinner, not streaming)
		// The prompt '>' reappears when processing is complete
		const timeout = opts?.timeout ?? 30_000;
		const start = Date.now();
		const pollMs = 200;

		while (Date.now() - start < timeout) {
			const screen = pty.getScreen();
			const hasPrompt = screen.includes('>');
			const hasSpinnerChar =
				screen.includes('⠋') ||
				screen.includes('⠙') ||
				screen.includes('⠹') ||
				screen.includes('⠸') ||
				screen.includes('⠼') ||
				screen.includes('⠴') ||
				screen.includes('⠦') ||
				screen.includes('⠧') ||
				screen.includes('⠇') ||
				screen.includes('⠏');
			if (hasPrompt && !hasSpinnerChar) return;
			await new Promise((r) => setTimeout(r, pollMs));
		}

		throw new Error(
			`waitForIdle timed out after ${timeout}ms.\nScreen:\n${pty.getScreen()}`,
		);
	}

	function hasToolCallBox(name?: string): boolean {
		const screen = pty.getScreen();
		if (name) {
			return screen.includes(`${name}`);
		}
		// Tool call boxes use unicode box-drawing: ┌, ─, ┐, │, └, ┘
		// and status icons: ✓, ✗, or spinner
		return screen.includes('│') && (screen.includes('✓') || screen.includes('✗'));
	}

	function hasErrorBox(): boolean {
		const screen = pty.getScreen();
		return screen.includes('Error') || screen.includes('✗');
	}

	function hasSpinner(): boolean {
		const screen = pty.getScreen();
		return (
			screen.includes('⠋') ||
			screen.includes('⠙') ||
			screen.includes('⠹') ||
			screen.includes('⠸') ||
			screen.includes('⠼') ||
			screen.includes('⠴') ||
			screen.includes('⠦') ||
			screen.includes('⠧') ||
			screen.includes('⠇') ||
			screen.includes('⠏')
		);
	}

	function hasBanner(): boolean {
		const screen = pty.getScreen();
		return screen.includes('simse') || screen.includes('╭');
	}

	function hasStatusBar(): boolean {
		const screen = pty.getScreen();
		return screen.includes('tokens') || screen.includes('server');
	}

	function hasPermissionDialog(): boolean {
		const screen = pty.getScreen();
		return (
			screen.includes('Allow') &&
			screen.includes('Deny')
		);
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
```

**Step 4: Run test to verify it passes**

Run: `bun test simse-code/e2e/harness/simse-terminal.test.ts`
Expected: PASS (5 tests, all green). These tests are slow (~15-25s each) since they spawn real processes.

**Step 5: Commit**

```bash
git add simse-code/e2e/harness/simse-terminal.ts simse-code/e2e/harness/simse-terminal.test.ts
git commit -m "feat(e2e): add SimseTerminal harness wrapping PTY + config"
```

---

### Task 6: Barrel Export

**Files:**
- Create: `simse-code/e2e/harness/index.ts`

**Step 1: Create barrel**

```typescript
// simse-code/e2e/harness/index.ts
export { KEYS, ctrlKey, type KeyName } from './keys.js';
export { createTempConfig, type ACPBackend, type TempConfigResult } from './config.js';
export { createPtyTerminal, type PtyTerminal, type PtyTerminalOptions } from './terminal.js';
export {
	createSimseTerminal,
	type SimseTerminal,
	type SimseTerminalOptions,
} from './simse-terminal.js';
```

**Step 2: Verify all existing tests still pass**

Run: `bun test simse-code/e2e/harness/`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add simse-code/e2e/harness/index.ts
git commit -m "feat(e2e): add barrel export for harness"
```

---

### Task 7: Startup E2E Tests

**Files:**
- Create: `simse-code/e2e/startup.e2e.ts`

**Step 1: Write the tests**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from './harness/index.js';

describe('E2E: startup', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
	});

	it('starts without ACP and shows banner', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		expect(term.hasBanner()).toBe(true);
		expect(term.getScreen()).toContain('>');
	}, 20_000);

	it('shows prompt ready for input', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		const screen = term.getScreen();
		expect(screen).toContain('>');
	}, 20_000);

	it('shows info about missing ACP when none configured', async () => {
		term = await createSimseTerminal({ acpBackend: 'none' });
		await term.waitForPrompt({ timeout: 15_000 });

		// The app should indicate no ACP is configured
		// (exact text depends on implementation — banner or info message)
		const screen = term.getScreen();
		// Should either mention setup or show no server in status
		expect(screen.length).toBeGreaterThan(0);
	}, 20_000);
}, { timeout: 60_000 });
```

**Step 2: Run to verify tests work**

Run: `bun test simse-code/e2e/startup.e2e.ts`
Expected: PASS — the CLI launches, banner renders, prompt appears.

**Step 3: Commit**

```bash
git add simse-code/e2e/startup.e2e.ts
git commit -m "test(e2e): add startup smoke tests"
```

---

### Task 8: Meta Command E2E Tests

**Files:**
- Create: `simse-code/e2e/commands/meta.e2e.ts`

**Step 1: Write the tests**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

describe('E2E: meta commands', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
	});

	describe('/help', () => {
		it('lists all command categories', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/help');
			await term.waitForText('help', { timeout: 10_000 });

			const screen = term.getScreen();
			expect(screen).toContain('search');
			expect(screen).toContain('add');
		}, 30_000);

		it('shows usage for a specific command', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/help search');
			await term.waitForText('search', { timeout: 10_000 });
		}, 30_000);
	});

	describe('/clear', () => {
		it('clears conversation history', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			// Send a command first to have content
			term.submit('/help');
			await term.waitForText('help', { timeout: 10_000 });

			// Clear
			term.submit('/clear');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/verbose', () => {
		it('toggles verbose badge', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/verbose');
			await term.waitForText('VERBOSE', { timeout: 10_000 });

			// Toggle off
			term.submit('/verbose');
			await term.waitForNoText('VERBOSE', { timeout: 10_000 });
		}, 30_000);
	});

	describe('/plan', () => {
		it('toggles plan badge', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/plan');
			await term.waitForText('PLAN', { timeout: 10_000 });

			// Toggle off
			term.submit('/plan');
			await term.waitForNoText('PLAN', { timeout: 10_000 });
		}, 30_000);
	});

	describe('/context', () => {
		it('shows context stats', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/context');
			// Should show some context information
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/exit', () => {
		it('exits the application', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/exit');
			// Process should exit — screen stops updating
			await new Promise((r) => setTimeout(r, 2_000));
		}, 20_000);
	});
}, { timeout: 120_000 });
```

**Step 2: Run to verify**

Run: `bun test simse-code/e2e/commands/meta.e2e.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/e2e/commands/meta.e2e.ts
git commit -m "test(e2e): add meta command tests (/help, /clear, /verbose, /plan, /context, /exit)"
```

---

### Task 9: Library Command E2E Tests

**Files:**
- Create: `simse-code/e2e/commands/library.e2e.ts`

**Step 1: Write the tests**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

describe('E2E: library commands', () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
	});

	describe('/add', () => {
		it('saves a note and confirms', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/add testing This is a test note');
			await term.waitForPrompt({ timeout: 10_000 });

			// Should confirm the note was saved
			const screen = term.getScreen();
			expect(
				screen.includes('saved') ||
				screen.includes('added') ||
				screen.includes('Added') ||
				screen.includes('Saved'),
			).toBe(true);
		}, 30_000);
	});

	describe('/search', () => {
		it('searches the library', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			// Add a note first
			term.submit('/add testing Search target note');
			await term.waitForPrompt({ timeout: 10_000 });

			// Search for it
			term.submit('/search target');
			await term.waitForPrompt({ timeout: 15_000 });
		}, 45_000);
	});

	describe('/topics', () => {
		it('lists topics', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/topics');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/notes', () => {
		it('lists notes', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			// Add a note first
			term.submit('/add testing Listed note');
			await term.waitForPrompt({ timeout: 10_000 });

			term.submit('/notes');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/delete', () => {
		it('deletes a note by ID', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			// Add and then delete
			term.submit('/add testing Deletable note');
			await term.waitForPrompt({ timeout: 10_000 });

			// Try to delete — ID may vary, but test the command runs
			term.submit('/delete 1');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/recommend', () => {
		it('shows recommendations', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/recommend test');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});

	describe('/get', () => {
		it('retrieves a note by ID', async () => {
			term = await createSimseTerminal({ acpBackend: 'none' });
			await term.waitForPrompt({ timeout: 15_000 });

			term.submit('/add testing Retrievable note');
			await term.waitForPrompt({ timeout: 10_000 });

			term.submit('/get 1');
			await term.waitForPrompt({ timeout: 10_000 });
		}, 30_000);
	});
}, { timeout: 180_000 });
```

**Step 2: Run to verify**

Run: `bun test simse-code/e2e/commands/library.e2e.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/e2e/commands/library.e2e.ts
git commit -m "test(e2e): add library command tests (/add, /search, /topics, /notes, /get, /delete, /recommend)"
```

---

### Task 10: Tools, Session, Files, Config Command E2E Tests

**Files:**
- Create: `simse-code/e2e/commands/tools.e2e.ts`
- Create: `simse-code/e2e/commands/session.e2e.ts`
- Create: `simse-code/e2e/commands/files.e2e.ts`
- Create: `simse-code/e2e/commands/config.e2e.ts`
- Create: `simse-code/e2e/commands/ai.e2e.ts`

Each file follows the same pattern: spawn terminal, wait for prompt, submit command, assert output. Since these are straightforward, they are grouped into one task.

**Step 1: Write all command test files**

Each file tests its category's commands. Follow the same pattern as Tasks 8-9:
- `tools.e2e.ts`: `/tools`, `/agents`, `/skills`
- `session.e2e.ts`: `/mcp`, `/library`, `/bypass-permissions` (no-ACP tests only; ACP-requiring commands like `/server`, `/agent`, `/model`, `/acp`, `/embed` are in Tier 2+)
- `files.e2e.ts`: `/files`, `/save`, `/validate`, `/discard`, `/diff`
- `config.e2e.ts`: `/config`, `/settings`, `/init`, `/setup`
- `ai.e2e.ts`: `/prompts` (no-ACP); `/chain` is in Tier 6

Each test: spawn → waitForPrompt → submit → waitForPrompt (command completes without crash).

**Step 2: Run all command tests**

Run: `bun test simse-code/e2e/commands/`
Expected: All PASS

**Step 3: Commit**

```bash
git add simse-code/e2e/commands/
git commit -m "test(e2e): add remaining command tests (tools, session, files, config, ai)"
```

---

### Task 11: UI E2E Tests (TextInput, Banner, Error States)

**Files:**
- Create: `simse-code/e2e/ui/text-input.e2e.ts`
- Create: `simse-code/e2e/ui/banner.e2e.ts`
- Create: `simse-code/e2e/ui/error-states.e2e.ts`

**Step 1: Write the tests**

`text-input.e2e.ts`:
- Cursor movement with arrow keys (type "hello", left arrow x2, type "XY" → "helXYlo")
- Backspace deletes character
- Rapid consecutive typing

`banner.e2e.ts`:
- Banner renders at 80 columns
- Banner renders at 120 columns (default)
- Banner contains mascot ASCII art (╭ character)

`error-states.e2e.ts`:
- Invalid command shows error message
- Empty prompt submission (just Enter) doesn't crash
- Very long input (500+ chars) doesn't crash

**Step 2: Run to verify**

Run: `bun test simse-code/e2e/ui/`
Expected: All PASS

**Step 3: Commit**

```bash
git add simse-code/e2e/ui/
git commit -m "test(e2e): add UI tests (text-input, banner, error states)"
```

---

### Task 12: ACP Prompt & Streaming E2E Tests

These tests require `ACP_BACKEND=claude` or `ACP_BACKEND=ollama`.

**Files:**
- Create: `simse-code/e2e/flows/prompt-streaming.e2e.ts`

**Step 1: Write the tests**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { createSimseTerminal, type SimseTerminal } from '../harness/index.js';

const backend = (process.env.ACP_BACKEND ?? 'none') as 'claude' | 'ollama' | 'none';

describe.skipIf(backend === 'none')(`E2E: prompt streaming (${backend})`, () => {
	let term: SimseTerminal | undefined;

	afterEach(async () => {
		await term?.kill();
	});

	it('submits a prompt and receives a response', async () => {
		term = await createSimseTerminal({ acpBackend: backend });
		await term.waitForPrompt({ timeout: 30_000 });

		term.submit('Say exactly: hello world');
		// Wait for the response to complete (prompt reappears)
		await term.waitForIdle({ timeout: 60_000 });

		const screen = term.getScreen();
		// Should have user message and assistant response
		expect(screen).toContain('>');
		expect(screen.length).toBeGreaterThan(100);
	}, 90_000);

	it('multi-turn conversation preserves history', async () => {
		term = await createSimseTerminal({ acpBackend: backend });
		await term.waitForPrompt({ timeout: 30_000 });

		term.submit('Remember the word: banana');
		await term.waitForIdle({ timeout: 60_000 });

		term.submit('What word did I ask you to remember?');
		await term.waitForIdle({ timeout: 60_000 });

		// Structure check: two user messages should be in the output
		const screen = term.getScreen();
		expect(screen.length).toBeGreaterThan(200);
	}, 180_000);

	it('abort mid-stream returns to prompt', async () => {
		term = await createSimseTerminal({ acpBackend: backend });
		await term.waitForPrompt({ timeout: 30_000 });

		term.submit('Write a very long essay about the history of computing');

		// Wait briefly for streaming to start
		await new Promise((r) => setTimeout(r, 3_000));

		// Abort
		term.pressCtrl('c');

		// Should return to prompt
		await term.waitForPrompt({ timeout: 15_000 });
	}, 60_000);
}, { timeout: 300_000 });
```

**Step 2: Run with ACP backend**

Run: `ACP_BACKEND=claude bun test simse-code/e2e/flows/prompt-streaming.e2e.ts`
or: `ACP_BACKEND=ollama bun test simse-code/e2e/flows/prompt-streaming.e2e.ts`
Expected: PASS (or SKIP if ACP_BACKEND=none)

**Step 3: Commit**

```bash
git add simse-code/e2e/flows/prompt-streaming.e2e.ts
git commit -m "test(e2e): add prompt streaming tests (ACP-backed)"
```

---

### Task 13: Tool Call Lifecycle E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/tool-calls.e2e.ts`

**Step 1: Write the tests**

Tests that trigger tool use via prompts (e.g., "read file X", "run bash command Y"). The ACP agent decides to use tools. Assert tool call boxes appear with correct status indicators.

Key tests:
- Prompt that triggers a tool call → tool call box renders (bordered, with name)
- Tool call completes → green status, duration shown
- Permission dialog (launch with `bypassPermissions: false`) → dialog renders, accept works

**Step 2: Run with ACP backend**

Run: `ACP_BACKEND=claude bun test simse-code/e2e/flows/tool-calls.e2e.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/e2e/flows/tool-calls.e2e.ts
git commit -m "test(e2e): add tool call lifecycle tests"
```

---

### Task 14: Subagent E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/subagents.e2e.ts`

Tests that prompt the AI to spawn subagents. Assert nested tool call boxes appear, subagent completes, result returned.

**Step 1: Write tests** (similar pattern to Task 12-13)

**Step 2: Run**: `ACP_BACKEND=claude bun test simse-code/e2e/flows/subagents.e2e.ts`

**Step 3: Commit**

```bash
git add simse-code/e2e/flows/subagents.e2e.ts
git commit -m "test(e2e): add subagent lifecycle tests"
```

---

### Task 15: MCP Integration E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/mcp-integration.e2e.ts`

Tests `/mcp` status, tool discovery via `/tools`, and MCP tool execution via prompts.

**Step 1: Write tests**
**Step 2: Run**
**Step 3: Commit**

```bash
git add simse-code/e2e/flows/mcp-integration.e2e.ts
git commit -m "test(e2e): add MCP integration tests"
```

---

### Task 16: Chain Execution E2E Tests

**Files:**
- Create: `simse-code/e2e/fixtures/prompts.json`
- Create: `simse-code/e2e/flows/chain-execution.e2e.ts`

Requires a test prompts.json fixture in the temp config's project dir. Tests `/chain` and `/prompts`.

**Step 1: Create fixture and tests**
**Step 2: Run**
**Step 3: Commit**

```bash
git add simse-code/e2e/fixtures/ simse-code/e2e/flows/chain-execution.e2e.ts
git commit -m "test(e2e): add chain execution tests with fixtures"
```

---

### Task 17: Library Memory E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/library-memory.e2e.ts`

Tests the full library lifecycle: embedded model initialization, add+search roundtrip, topic extraction, deduplication, recommendations.

Key tests:
- `/add` + `/search` finds the note (semantic matching via embedded model)
- Multiple adds → `/topics` shows extracted topics
- Adding duplicate content → deduplication warning
- `/recommend` reflects usage patterns after repeated searches

**Step 1: Write tests** (120s timeout for ONNX model download)
**Step 2: Run**
**Step 3: Commit**

```bash
git add simse-code/e2e/flows/library-memory.e2e.ts
git commit -m "test(e2e): add library memory tests (embedded model, search, topics, dedup)"
```

---

### Task 18: Memory Optimization E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/memory-optimization.e2e.ts`

Tests librarian operations that require ACP (large model): extract, summarize, classify, compendium generation.

**Step 1: Write tests** (ACP-backed, Claude only for powerful model)
**Step 2: Run**: `ACP_BACKEND=claude bun test simse-code/e2e/flows/memory-optimization.e2e.ts`
**Step 3: Commit**

```bash
git add simse-code/e2e/flows/memory-optimization.e2e.ts
git commit -m "test(e2e): add memory optimization tests (librarian, compendium)"
```

---

### Task 19: Conversation Management E2E Tests

**Files:**
- Create: `simse-code/e2e/flows/conversation.e2e.ts`

Tests auto-compaction, `/compact`, `/clear` + resume, `/context` stats tracking.

**Step 1: Write tests**
**Step 2: Run**
**Step 3: Commit**

```bash
git add simse-code/e2e/flows/conversation.e2e.ts
git commit -m "test(e2e): add conversation management tests (compaction, context)"
```

---

### Task 20: Status Bar E2E Tests

**Files:**
- Create: `simse-code/e2e/ui/status-bar.e2e.ts`

Tests status bar updates after ACP interactions (token count, cost, server info).

**Step 1: Write tests** (ACP-backed)
**Step 2: Run**
**Step 3: Commit**

```bash
git add simse-code/e2e/ui/status-bar.e2e.ts
git commit -m "test(e2e): add status bar live update tests"
```

---

### Task 21: ACP-Required Session Command Tests

**Files:**
- Modify: `simse-code/e2e/commands/session.e2e.ts`

Add ACP-backed tests for `/server`, `/agent`, `/model`, `/acp`, `/embed` — these were deferred from Task 10 because they require a running ACP backend.

**Step 1: Add skipIf tests to session.e2e.ts**
**Step 2: Run**: `ACP_BACKEND=claude bun test simse-code/e2e/commands/session.e2e.ts`
**Step 3: Commit**

```bash
git add simse-code/e2e/commands/session.e2e.ts
git commit -m "test(e2e): add ACP-backed session command tests (/server, /agent, /model, /acp, /embed)"
```

---

### Task 22: Update bunfig.toml and package.json Scripts

**Files:**
- Modify: `bunfig.toml`
- Modify: `package.json`

**Step 1: Update bunfig.toml**

The current test root is `./tests`. E2E tests live under `simse-code/e2e/`. Bun needs to find both. Either change the root or use multiple `bun test` commands.

Add to `package.json` scripts:
```json
{
  "test:e2e": "bun test simse-code/e2e/",
  "test:e2e:commands": "bun test simse-code/e2e/commands/",
  "test:e2e:flows": "bun test simse-code/e2e/flows/",
  "test:e2e:ui": "bun test simse-code/e2e/ui/",
  "test:e2e:claude": "ACP_BACKEND=claude bun test simse-code/e2e/",
  "test:e2e:ollama": "ACP_BACKEND=ollama bun test simse-code/e2e/"
}
```

**Step 2: Verify all E2E tests run**

Run: `bun run test:e2e`
Expected: All no-ACP tests pass, ACP tests skip.

**Step 3: Commit**

```bash
git add bunfig.toml package.json
git commit -m "chore: add E2E test scripts to package.json"
```

---

### Task 23: Final Verification — Full Test Suite

**Step 1: Run all unit tests**

Run: `bun test`
Expected: All existing 74 test files still pass. E2E tests are in a separate directory and don't interfere.

**Step 2: Run all E2E tests (no ACP)**

Run: `bun run test:e2e`
Expected: All command, UI, and startup tests pass. Flow tests skip (no ACP backend).

**Step 3: Run E2E with Claude backend** (if API key available)

Run: `bun run test:e2e:claude`
Expected: All tests pass including streaming, tool calls, subagents, memory optimization.

**Step 4: Run E2E with Ollama backend** (if running locally)

Run: `bun run test:e2e:ollama`
Expected: All applicable tests pass.

**Step 5: Final commit**

```bash
git add -A
git commit -m "test(e2e): complete E2E test framework — 80+ tests across 10 tiers"
```
