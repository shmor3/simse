/**
 * SimSE Code — Hooks CLI Integration
 *
 * Configuration and management for the hook system.
 * Hooks are shell commands that run in response to events
 * like tool calls and prompt transforms.
 * No external deps.
 */

import { join } from 'node:path';
import { readJsonFile, writeJsonFile } from './json-io.js';
import type { TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type HookType =
	| 'tool.execute.before'
	| 'tool.execute.after'
	| 'prompt.system.transform'
	| 'session.compacting';

export interface HookConfig {
	readonly type: HookType;
	readonly command: string;
	readonly args?: readonly string[];
	readonly timeoutMs?: number;
	readonly enabled?: boolean;
}

export interface HooksFileConfig {
	readonly hooks: readonly HookConfig[];
}

export interface HooksManager {
	readonly list: () => readonly HookConfig[];
	readonly add: (hook: HookConfig) => void;
	readonly remove: (index: number) => boolean;
	readonly toggle: (index: number) => boolean;
	readonly save: () => void;
	readonly load: () => void;
}

export interface HooksManagerOptions {
	readonly dataDir: string;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createHooksManager(options: HooksManagerOptions): HooksManager {
	const configPath = join(options.dataDir, 'hooks.json');
	const hooks: HookConfig[] = [];

	const loadHooks = (): void => {
		const saved = readJsonFile<HooksFileConfig>(configPath);
		if (saved?.hooks) {
			hooks.length = 0;
			hooks.push(...saved.hooks);
		}
	};

	// Load on init
	loadHooks();

	const list = (): readonly HookConfig[] => [...hooks];

	const add = (hook: HookConfig): void => {
		hooks.push(hook);
	};

	const remove = (index: number): boolean => {
		if (index < 0 || index >= hooks.length) return false;
		hooks.splice(index, 1);
		return true;
	};

	const toggle = (index: number): boolean => {
		if (index < 0 || index >= hooks.length) return false;
		const current = hooks[index];
		hooks[index] = { ...current, enabled: !(current.enabled ?? true) };
		return true;
	};

	const save = (): void => {
		writeJsonFile(configPath, { hooks });
	};

	const load = (): void => {
		loadHooks();
	};

	return Object.freeze({ list, add, remove, toggle, save, load });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const HOOK_TYPE_LABELS: Readonly<Record<HookType, string>> = {
	'tool.execute.before': 'Before tool',
	'tool.execute.after': 'After tool',
	'prompt.system.transform': 'System prompt',
	'session.compacting': 'On compact',
};

/**
 * Render the hooks list for display.
 */
export function renderHooksList(
	hooks: readonly HookConfig[],
	colors: TermColors,
): string {
	if (hooks.length === 0) {
		return `  ${colors.dim('No hooks configured.')}`;
	}

	const lines: string[] = [];
	for (let i = 0; i < hooks.length; i++) {
		const hook = hooks[i];
		const enabled = hook.enabled !== false;
		const status = enabled ? colors.green('●') : colors.red('○');
		const label = HOOK_TYPE_LABELS[hook.type] ?? hook.type;
		const cmd = hook.command + (hook.args ? ` ${hook.args.join(' ')}` : '');

		lines.push(
			`  ${status} ${colors.dim(`${i + 1}.`)} ${colors.bold(label)} ${colors.dim('→')} ${cmd}`,
		);
	}

	return lines.join('\n');
}
