/**
 * SimSE Code — Keybinding Manager
 *
 * Centralized keypress handler registry for hotkeys like Ctrl+O, Ctrl+B,
 * Shift+Tab, Double-Escape, etc. Hooks into process.stdin raw mode.
 * No external deps.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface KeyCombo {
	readonly name: string;
	readonly ctrl?: boolean;
	readonly shift?: boolean;
	readonly meta?: boolean;
}

export type KeyHandler = () => void | Promise<void>;

export interface KeybindingEntry {
	readonly combo: KeyCombo;
	readonly label: string;
	readonly handler: KeyHandler;
}

export interface KeybindingManager {
	/** Register a handler for a key combo. Returns unregister function. */
	readonly register: (
		combo: KeyCombo,
		label: string,
		handler: KeyHandler,
	) => () => void;
	/** Start listening for keypresses on stdin. */
	readonly attach: (stdin: NodeJS.ReadStream) => void;
	/** Stop listening. */
	readonly detach: () => void;
	/** List all registered bindings (for help display). */
	readonly list: () => readonly KeybindingEntry[];
}

export interface KeybindingManagerOptions {
	/** Timeout (ms) for double-tap detection (e.g. double-Escape). Default: 400 */
	readonly doubleTapMs?: number;
}

// ---------------------------------------------------------------------------
// Key matching
// ---------------------------------------------------------------------------

interface KeypressInfo {
	readonly name?: string;
	readonly ctrl?: boolean;
	readonly shift?: boolean;
	readonly meta?: boolean;
	readonly sequence?: string;
}

function matchesCombo(combo: KeyCombo, key: KeypressInfo): boolean {
	if (combo.name !== key.name) return false;
	if ((combo.ctrl ?? false) !== (key.ctrl ?? false)) return false;
	if ((combo.shift ?? false) !== (key.shift ?? false)) return false;
	if ((combo.meta ?? false) !== (key.meta ?? false)) return false;
	return true;
}

function comboToString(combo: KeyCombo): string {
	const parts: string[] = [];
	if (combo.ctrl) parts.push('Ctrl');
	if (combo.shift) parts.push('Shift');
	if (combo.meta) parts.push('Meta');
	parts.push(combo.name.charAt(0).toUpperCase() + combo.name.slice(1));
	return parts.join('+');
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createKeybindingManager(
	options?: KeybindingManagerOptions,
): KeybindingManager {
	const doubleTapMs = options?.doubleTapMs ?? 400;
	const entries: KeybindingEntry[] = [];
	let listener: ((_ch: string, key: KeypressInfo) => void) | undefined;
	let attachedStdin: NodeJS.ReadStream | undefined;

	// Double-tap tracking: key name → last press timestamp
	const lastPress = new Map<string, number>();

	const register = (
		combo: KeyCombo,
		label: string,
		handler: KeyHandler,
	): (() => void) => {
		const entry: KeybindingEntry = { combo, label, handler };
		entries.push(entry);
		return () => {
			const idx = entries.indexOf(entry);
			if (idx >= 0) entries.splice(idx, 1);
		};
	};

	const handleKeypress = (_ch: string, key: KeypressInfo): void => {
		if (!key) return;

		for (const entry of entries) {
			if (matchesCombo(entry.combo, key)) {
				// Check double-tap for matching combos
				const comboStr = comboToString(entry.combo);
				const now = Date.now();
				const last = lastPress.get(comboStr);

				// If the combo label contains "double", require double-tap
				if (entry.label.toLowerCase().includes('double')) {
					if (last && now - last < doubleTapMs) {
						lastPress.delete(comboStr);
						Promise.resolve(entry.handler()).catch(() => {});
					} else {
						lastPress.set(comboStr, now);
					}
				} else {
					Promise.resolve(entry.handler()).catch(() => {});
				}
			}
		}
	};

	const attach = (stdin: NodeJS.ReadStream): void => {
		if (attachedStdin) detach();
		attachedStdin = stdin;
		listener = handleKeypress;
		stdin.on('keypress', listener);
	};

	const detach = (): void => {
		if (attachedStdin && listener) {
			attachedStdin.removeListener('keypress', listener);
		}
		attachedStdin = undefined;
		listener = undefined;
	};

	const list = (): readonly KeybindingEntry[] => [...entries];

	return Object.freeze({ register, attach, detach, list });
}
