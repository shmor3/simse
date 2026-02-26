/**
 * SimSE Code — Theme System
 *
 * Built-in themes with ANSI 256-color codes. Themes control colors for
 * UI elements, diff display, and syntax highlighting.
 * No external deps.
 */

import { join } from 'node:path';
import { readJsonFile, writeJsonFile } from './json-io.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ThemeDefinition {
	readonly name: string;
	readonly ui: {
		readonly primary: number;
		readonly secondary: number;
		readonly accent: number;
		readonly muted: number;
		readonly success: number;
		readonly warning: number;
		readonly error: number;
		readonly info: number;
	};
	readonly diff: {
		readonly add: number;
		readonly remove: number;
		readonly context: number;
		readonly hunkHeader: number;
	};
	readonly syntax: {
		readonly keyword: number;
		readonly string: number;
		readonly number: number;
		readonly comment: number;
		readonly function: number;
		readonly type: number;
	};
}

export interface ThemeManager {
	readonly getActive: () => ThemeDefinition;
	readonly setActive: (name: string) => boolean;
	readonly list: () => readonly string[];
	readonly get: (name: string) => ThemeDefinition | undefined;
}

export interface ThemeManagerOptions {
	readonly dataDir: string;
	readonly initialTheme?: string;
}

// ---------------------------------------------------------------------------
// ANSI 256-color helper
// ---------------------------------------------------------------------------

export function ansi256(code: number): (s: string) => string {
	return (s: string) => `\x1b[38;5;${code}m${s}\x1b[0m`;
}

export function ansi256Bg(code: number): (s: string) => string {
	return (s: string) => `\x1b[48;5;${code}m${s}\x1b[0m`;
}

// ---------------------------------------------------------------------------
// Built-in themes
// ---------------------------------------------------------------------------

const DEFAULT_THEME: ThemeDefinition = Object.freeze({
	name: 'default',
	ui: Object.freeze({
		primary: 255, // white
		secondary: 248, // light gray
		accent: 39, // blue
		muted: 242, // dim gray
		success: 78, // green
		warning: 220, // yellow
		error: 196, // red
		info: 39, // cyan-blue
	}),
	diff: Object.freeze({
		add: 78, // green
		remove: 196, // red
		context: 242, // dim
		hunkHeader: 39, // cyan
	}),
	syntax: Object.freeze({
		keyword: 171, // purple
		string: 113, // green
		number: 208, // orange
		comment: 242, // gray
		function: 75, // blue
		type: 222, // yellow
	}),
});

const DARK_THEME: ThemeDefinition = Object.freeze({
	name: 'dark',
	ui: Object.freeze({
		primary: 255,
		secondary: 250,
		accent: 75,
		muted: 240,
		success: 114,
		warning: 221,
		error: 203,
		info: 117,
	}),
	diff: Object.freeze({
		add: 114,
		remove: 203,
		context: 240,
		hunkHeader: 117,
	}),
	syntax: Object.freeze({
		keyword: 176,
		string: 114,
		number: 209,
		comment: 240,
		function: 75,
		type: 222,
	}),
});

const LIGHT_THEME: ThemeDefinition = Object.freeze({
	name: 'light',
	ui: Object.freeze({
		primary: 235, // near black
		secondary: 238,
		accent: 25, // dark blue
		muted: 245,
		success: 28, // dark green
		warning: 130, // dark yellow
		error: 124, // dark red
		info: 25,
	}),
	diff: Object.freeze({
		add: 28,
		remove: 124,
		context: 245,
		hunkHeader: 25,
	}),
	syntax: Object.freeze({
		keyword: 127,
		string: 28,
		number: 166,
		comment: 245,
		function: 25,
		type: 130,
	}),
});

const NORD_THEME: ThemeDefinition = Object.freeze({
	name: 'nord',
	ui: Object.freeze({
		primary: 255,
		secondary: 249,
		accent: 110, // nord blue
		muted: 243,
		success: 108, // nord green
		warning: 222, // nord yellow
		error: 174, // nord red
		info: 110,
	}),
	diff: Object.freeze({
		add: 108,
		remove: 174,
		context: 243,
		hunkHeader: 110,
	}),
	syntax: Object.freeze({
		keyword: 176,
		string: 108,
		number: 208,
		comment: 243,
		function: 110,
		type: 222,
	}),
});

const GRUVBOX_THEME: ThemeDefinition = Object.freeze({
	name: 'gruvbox',
	ui: Object.freeze({
		primary: 223, // fg
		secondary: 250,
		accent: 109, // blue
		muted: 245,
		success: 142, // green
		warning: 214, // yellow
		error: 167, // red
		info: 109,
	}),
	diff: Object.freeze({
		add: 142,
		remove: 167,
		context: 245,
		hunkHeader: 109,
	}),
	syntax: Object.freeze({
		keyword: 175, // purple
		string: 142,
		number: 208,
		comment: 245,
		function: 109,
		type: 214,
	}),
});

const THEMES: readonly ThemeDefinition[] = [
	DEFAULT_THEME,
	DARK_THEME,
	LIGHT_THEME,
	NORD_THEME,
	GRUVBOX_THEME,
];

// ---------------------------------------------------------------------------
// Config persistence
// ---------------------------------------------------------------------------

interface ThemeConfig {
	readonly activeTheme?: string;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createThemeManager(options: ThemeManagerOptions): ThemeManager {
	const configPath = join(options.dataDir, 'theme.json');
	const themeMap = new Map<string, ThemeDefinition>();

	for (const theme of THEMES) {
		themeMap.set(theme.name, theme);
	}

	// Load saved theme preference
	const saved = readJsonFile<ThemeConfig>(configPath);
	let activeName = options.initialTheme ?? saved?.activeTheme ?? 'default';

	// Validate active theme exists
	if (!themeMap.has(activeName)) {
		activeName = 'default';
	}

	const getActive = (): ThemeDefinition => {
		return themeMap.get(activeName) ?? DEFAULT_THEME;
	};

	const setActive = (name: string): boolean => {
		if (!themeMap.has(name)) return false;
		activeName = name;
		writeJsonFile(configPath, { activeTheme: name });
		return true;
	};

	const list = (): readonly string[] => {
		return THEMES.map((t) => t.name);
	};

	const get = (name: string): ThemeDefinition | undefined => {
		return themeMap.get(name);
	};

	return Object.freeze({ getActive, setActive, list, get });
}
