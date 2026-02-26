/**
 * SimSE Code — Permission Manager
 *
 * Wraps the library's ToolPermissionResolver with CLI-specific
 * permission modes (default, acceptEdits, plan, dontAsk) and
 * persistent rule storage.
 * No external deps.
 */

import { join } from 'node:path';
import type { PermissionMode } from './app-context.js';
import { readJsonFile, writeJsonFile } from './json-io.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { PermissionMode };

export type PermissionDecision = 'allow' | 'deny' | 'ask';

export interface PermissionRule {
	readonly tool: string;
	readonly pattern?: string;
	readonly policy: PermissionDecision;
}

export interface PermissionManager {
	/** Check if a tool call should be allowed, denied, or needs user prompt. */
	readonly check: (
		toolName: string,
		args?: Readonly<Record<string, unknown>>,
	) => PermissionDecision;
	/** Get the current permission mode. */
	readonly getMode: () => PermissionMode;
	/** Set the permission mode. */
	readonly setMode: (mode: PermissionMode) => void;
	/** Cycle to the next permission mode. */
	readonly cycleMode: () => PermissionMode;
	/** Add a persistent permission rule. */
	readonly addRule: (rule: PermissionRule) => void;
	/** Remove a rule by tool name. */
	readonly removeRule: (toolName: string) => void;
	/** Get all rules. */
	readonly getRules: () => readonly PermissionRule[];
	/** Save rules to disk. */
	readonly save: () => void;
	/** Load rules from disk. */
	readonly load: () => void;
	/** Format mode for display. */
	readonly formatMode: () => string;
}

export interface PermissionManagerOptions {
	readonly dataDir: string;
	readonly initialMode?: PermissionMode;
}

// ---------------------------------------------------------------------------
// Tool categories for permission logic
// ---------------------------------------------------------------------------

const WRITE_TOOLS = new Set([
	'vfs_write',
	'vfs_delete',
	'vfs_rename',
	'vfs_mkdir',
	'file_write',
	'file_edit',
	'file_create',
]);

const BASH_TOOLS = new Set(['bash', 'shell', 'exec', 'execute', 'run_command']);

const READ_ONLY_TOOLS = new Set([
	'vfs_read',
	'vfs_list',
	'vfs_stat',
	'vfs_search',
	'vfs_diff',
	'file_read',
	'glob',
	'grep',
	'memory_search',
	'memory_list',
	'task_list',
	'task_get',
]);

// ---------------------------------------------------------------------------
// Mode labels
// ---------------------------------------------------------------------------

const MODE_ORDER: readonly PermissionMode[] = [
	'default',
	'acceptEdits',
	'plan',
	'dontAsk',
];

const MODE_LABELS: Readonly<Record<PermissionMode, string>> = {
	default: 'Default',
	acceptEdits: 'Auto-Edit',
	plan: 'Plan (read-only)',
	dontAsk: 'YOLO',
};

const MODE_DESCRIPTIONS: Readonly<Record<PermissionMode, string>> = {
	default: 'Ask for writes & bash',
	acceptEdits: 'Auto-allow file edits, ask for bash',
	plan: 'Read-only — deny writes & bash',
	dontAsk: 'Allow everything without asking',
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createPermissionManager(
	options: PermissionManagerOptions,
): PermissionManager {
	const configPath = join(options.dataDir, 'permissions.json');
	let mode: PermissionMode = options.initialMode ?? 'default';
	const rules: PermissionRule[] = [];

	// Load saved rules
	const loadRules = (): void => {
		const saved = readJsonFile<{
			mode?: PermissionMode;
			rules?: PermissionRule[];
		}>(configPath);
		if (saved) {
			if (saved.mode && MODE_ORDER.includes(saved.mode)) {
				mode = saved.mode;
			}
			if (saved.rules) {
				rules.length = 0;
				rules.push(...saved.rules);
			}
		}
	};

	loadRules();

	const check = (
		toolName: string,
		_args?: Readonly<Record<string, unknown>>,
	): PermissionDecision => {
		// Check explicit rules first (highest priority)
		for (const rule of rules) {
			if (rule.tool === toolName || matchGlob(rule.tool, toolName)) {
				return rule.policy;
			}
		}

		// Mode-based decisions
		switch (mode) {
			case 'dontAsk':
				return 'allow';

			case 'plan':
				// Read-only mode: allow reads, deny writes/bash
				if (READ_ONLY_TOOLS.has(toolName)) return 'allow';
				if (WRITE_TOOLS.has(toolName) || BASH_TOOLS.has(toolName))
					return 'deny';
				return 'ask';

			case 'acceptEdits':
				// Auto-allow file edits, ask for bash
				if (READ_ONLY_TOOLS.has(toolName)) return 'allow';
				if (WRITE_TOOLS.has(toolName)) return 'allow';
				if (BASH_TOOLS.has(toolName)) return 'ask';
				return 'allow';
			default:
				// Allow reads, ask for writes/bash
				if (READ_ONLY_TOOLS.has(toolName)) return 'allow';
				if (WRITE_TOOLS.has(toolName) || BASH_TOOLS.has(toolName)) return 'ask';
				return 'allow';
		}
	};

	const getMode = (): PermissionMode => mode;

	const setMode = (newMode: PermissionMode): void => {
		mode = newMode;
	};

	const cycleMode = (): PermissionMode => {
		const idx = MODE_ORDER.indexOf(mode);
		const nextIdx = (idx + 1) % MODE_ORDER.length;
		mode = MODE_ORDER[nextIdx];
		return mode;
	};

	const addRule = (rule: PermissionRule): void => {
		// Remove existing rule for same tool
		const idx = rules.findIndex((r) => r.tool === rule.tool);
		if (idx >= 0) rules.splice(idx, 1);
		rules.push(rule);
	};

	const removeRule = (toolName: string): void => {
		const idx = rules.findIndex((r) => r.tool === toolName);
		if (idx >= 0) rules.splice(idx, 1);
	};

	const getRules = (): readonly PermissionRule[] => [...rules];

	const save = (): void => {
		writeJsonFile(configPath, { mode, rules });
	};

	const load = (): void => {
		loadRules();
	};

	const formatMode = (): string => {
		return `${MODE_LABELS[mode]} — ${MODE_DESCRIPTIONS[mode]}`;
	};

	return Object.freeze({
		check,
		getMode,
		setMode,
		cycleMode,
		addRule,
		removeRule,
		getRules,
		save,
		load,
		formatMode,
	});
}

// ---------------------------------------------------------------------------
// Simple glob matching (supports * and ?)
// ---------------------------------------------------------------------------

function matchGlob(pattern: string, value: string): boolean {
	const regex = pattern
		.replace(/[.+^${}()|[\]\\]/g, '\\$&')
		.replace(/\*/g, '.*')
		.replace(/\?/g, '.');
	return new RegExp(`^${regex}$`).test(value);
}
