/**
 * SimSE Code — Terminal UI Primitives
 *
 * Colors, spinner, markdown renderer, and formatters.
 * Matches Claude Code's visual language exactly.
 * No external deps — raw ANSI escape codes only.
 */

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

export interface TermColors {
	readonly bold: (s: string) => string;
	readonly dim: (s: string) => string;
	readonly italic: (s: string) => string;
	readonly underline: (s: string) => string;
	readonly red: (s: string) => string;
	readonly green: (s: string) => string;
	readonly yellow: (s: string) => string;
	readonly blue: (s: string) => string;
	readonly magenta: (s: string) => string;
	readonly cyan: (s: string) => string;
	readonly gray: (s: string) => string;
	readonly white: (s: string) => string;
	readonly enabled: boolean;
}

export interface TermColorsOptions {
	readonly enabled?: boolean;
}

function ansi(code: number): (s: string) => string {
	const open = `\x1b[${code}m`;
	const close = '\x1b[0m';
	return (s: string) => `${open}${s}${close}`;
}

export function createColors(options?: TermColorsOptions): TermColors {
	const enabled =
		options?.enabled ??
		(process.stdout.isTTY === true && !process.env.NO_COLOR);

	if (!enabled) {
		const identity = (s: string): string => s;
		return Object.freeze({
			bold: identity,
			dim: identity,
			italic: identity,
			underline: identity,
			red: identity,
			green: identity,
			yellow: identity,
			blue: identity,
			magenta: identity,
			cyan: identity,
			gray: identity,
			white: identity,
			enabled: false,
		});
	}

	return Object.freeze({
		bold: ansi(1),
		dim: ansi(2),
		italic: ansi(3),
		underline: ansi(4),
		red: ansi(31),
		green: ansi(32),
		yellow: ansi(33),
		blue: ansi(34),
		magenta: ansi(35),
		cyan: ansi(36),
		gray: ansi(90),
		white: ansi(37),
		enabled: true,
	});
}

// ---------------------------------------------------------------------------
// Mascot — pixel-art character rendered with Unicode block elements
// ---------------------------------------------------------------------------

// 3-line mascot — stylized brain/circuit icon in teal (38 = deep cyan in 256-color)
const MASCOT_LINES = ['╭◉─◉╮', '│▓▓▓│', '╰─┬─╯'];

function ansi256Fg(code: number): (s: string) => string {
	return (s: string) => `\x1b[38;5;${code}m${s}\x1b[0m`;
}

const mascotColor = ansi256Fg(38);

// ---------------------------------------------------------------------------
// Spinner — Claude Code style (cycling star characters)
// ---------------------------------------------------------------------------

export interface Spinner {
	readonly start: (message?: string) => void;
	readonly update: (message: string) => void;
	readonly succeed: (message: string) => void;
	readonly fail: (message: string) => void;
	readonly stop: () => void;
}

export interface SpinnerOptions {
	readonly colors?: TermColors;
	readonly stream?: NodeJS.WriteStream;
}

// Claude Code spinner frames: cycling unicode star glyphs
const SPINNER_FRAMES = ['·', '✢', '✳', '∗', '✻', '✽'];
const ASCII_FRAMES = ['-', '\\', '|', '/'];

// Braille frames for terminal tab title animation
const BRAILLE_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

export function createSpinner(options?: SpinnerOptions): Spinner {
	const colors = options?.colors ?? createColors();
	const stream = options?.stream ?? process.stderr;
	const isTTY = stream.isTTY === true;
	const frames = isTTY ? SPINNER_FRAMES : ASCII_FRAMES;

	let timer: ReturnType<typeof setInterval> | undefined;
	let frameIdx = 0;
	let currentMessage = '';
	let brailleIdx = 0;

	const clearLine = (): void => {
		if (isTTY) stream.write('\x1b[2K\r');
	};

	const setTabTitle = (title: string): void => {
		if (isTTY) stream.write(`\x1b]0;${title}\x07`);
	};

	const render = (): void => {
		const frame = colors.yellow(frames[frameIdx % frames.length]);
		clearLine();
		stream.write(`  ${frame} ${colors.dim(currentMessage)}`);
		frameIdx++;
		const braille = BRAILLE_FRAMES[brailleIdx % BRAILLE_FRAMES.length];
		setTabTitle(`${braille} simse`);
		brailleIdx++;
	};

	const start = (message?: string): void => {
		stop();
		currentMessage = message ?? '';
		frameIdx = 0;
		brailleIdx = 0;

		if (!isTTY) {
			stream.write(`  ${message}\n`);
			return;
		}

		stream.write('\x1b[?25l'); // hide cursor
		render();
		timer = setInterval(render, 120);
	};

	const update = (message: string): void => {
		currentMessage = message;
		if (!isTTY) {
			stream.write(`  ${message}\n`);
		}
	};

	const stop = (): void => {
		if (timer) {
			clearInterval(timer);
			timer = undefined;
		}
		if (isTTY) {
			clearLine();
			stream.write('\x1b[?25h'); // restore cursor
			setTabTitle('simse');
		}
	};

	const succeed = (message: string): void => {
		stop();
		stream.write(`  ${colors.green('●')} ${message}\n`);
	};

	const fail = (message: string): void => {
		stop();
		stream.write(`  ${colors.red('●')} ${message}\n`);
	};

	return Object.freeze({ start, update, succeed, fail, stop });
}

// ---------------------------------------------------------------------------
// Thinking Spinner — rotating verbs, Claude Code style
// Format: ✢ Grooving...
// ---------------------------------------------------------------------------

const THINKING_VERBS = [
	'Thinking',
	'Analyzing',
	'Considering',
	'Processing',
	'Reasoning',
	'Pondering',
	'Evaluating',
	'Computing',
	'Exploring',
	'Discovering',
	'Connecting',
	'Crafting',
	'Reflecting',
	'Deducing',
	'Composing',
	'Cogitating',
	'Noodling',
	'Vibing',
	'Finagling',
	'Cerebrating',
	'Musing',
	'Ruminating',
	'Brainstorming',
	'Mulling',
	'Simmering',
	'Brewing',
	'Percolating',
	'Crystallizing',
	'Transmuting',
	'Coalescing',
	'Reticulating',
	'Conjuring',
	'Concocting',
	'Hatching',
	'Incubating',
	'Germinating',
	'Manifesting',
	'Actualizing',
	'Channeling',
	'Orchestrating',
	'Architecting',
	'Sculpting',
	'Weaving',
	'Distilling',
	'Calibrating',
	'Harmonizing',
	'Spelunking',
	'Wrangling',
	'Tinkering',
	'Grokking',
	'Unfurling',
	'Ideating',
	'Hustling',
	'Scheming',
	'Frolicking',
	'Moseying',
	'Meandering',
	'Puttering',
	'Shimmying',
	'Jiving',
	'Grooving',
	'Baking',
	'Cooking',
	'Stewing',
	'Marinating',
	'Steeping',
	'Fermenting',
	'Blossoming',
	'Flourishing',
	'Sussing',
	'Puzzling',
	'Smooshing',
	'Wibbling',
	'Wizarding',
	'Clauding',
	'Honking',
	'Booping',
	'Herding',
	'Shucking',
	'Spinning',
	'Whirring',
	'Working',
	'Creating',
	'Generating',
	'Determining',
	'Forging',
	'Forming',
	'Imagining',
	'Inferring',
	'Enchanting',
	'Divining',
] as const;

export interface ThinkingSpinnerOptions {
	readonly colors?: TermColors;
	readonly stream?: NodeJS.WriteStream;
	readonly verbs?: readonly string[];
	readonly verbIntervalMs?: number;
}

export interface ThinkingSpinner extends Spinner {
	/** Update the token count displayed in the spinner suffix. */
	readonly setTokens: (tokens: number) => void;
	/** Update the thinking/processing state label. */
	readonly setState: (state: string) => void;
}

export function createThinkingSpinner(
	options?: ThinkingSpinnerOptions,
): ThinkingSpinner {
	const colors = options?.colors ?? createColors();
	const stream = options?.stream ?? process.stderr;
	const isTTY = stream.isTTY === true;
	const verbs = options?.verbs ?? THINKING_VERBS;
	const verbInterval = options?.verbIntervalMs ?? 3000;
	const frames = isTTY ? SPINNER_FRAMES : ASCII_FRAMES;

	let timer: ReturnType<typeof setInterval> | undefined;
	let verbTimer: ReturnType<typeof setInterval> | undefined;
	let frameIdx = 0;
	let brailleIdx = 0;
	let verbIdx = 0;
	let currentVerb = '';
	let startedAt = 0;
	let tokenCount = 0;
	let stateLabel = 'thinking';

	const formatSuffix = (): string => {
		const parts: string[] = [];
		if (startedAt > 0) {
			const elapsed = Date.now() - startedAt;
			parts.push(formatDuration(elapsed));
		}
		if (tokenCount > 0) {
			const formatted =
				tokenCount >= 1000
					? `${(tokenCount / 1000).toFixed(1)}k`
					: String(tokenCount);
			parts.push(`↓ ${formatted} tokens`);
		}
		if (stateLabel) {
			parts.push(stateLabel);
		}
		return parts.length > 0 ? ` ${colors.dim(`(${parts.join(' · ')})`)}` : '';
	};

	const clearLine = (): void => {
		if (isTTY) stream.write('\x1b[2K\r');
	};

	const setTabTitle = (title: string): void => {
		if (isTTY) stream.write(`\x1b]0;${title}\x07`);
	};

	const render = (): void => {
		const frame = colors.yellow(frames[frameIdx % frames.length]);
		clearLine();
		const suffix = formatSuffix();
		stream.write(`  ${frame} ${colors.dim(`${currentVerb}...`)}${suffix}`);
		frameIdx++;
		const braille = BRAILLE_FRAMES[brailleIdx % BRAILLE_FRAMES.length];
		setTabTitle(`${braille} simse`);
		brailleIdx++;
	};

	const clearTimers = (): void => {
		if (timer) {
			clearInterval(timer);
			timer = undefined;
		}
		if (verbTimer) {
			clearInterval(verbTimer);
			verbTimer = undefined;
		}
	};

	const start = (message?: string): void => {
		clearTimers();
		startedAt = Date.now();
		frameIdx = 0;
		brailleIdx = 0;

		if (message) {
			currentVerb = message.replace(/\.{3}$/, '');
			if (!isTTY) {
				stream.write(`  ${message}\n`);
				return;
			}
			stream.write('\x1b[?25l');
			render();
			timer = setInterval(render, 120);
			return;
		}

		verbIdx = Math.floor(Math.random() * verbs.length);
		currentVerb = verbs[verbIdx];

		if (!isTTY) {
			stream.write(`  ${currentVerb}...\n`);
			return;
		}

		stream.write('\x1b[?25l');
		render();
		timer = setInterval(render, 120);

		if (verbs.length > 1) {
			verbTimer = setInterval(() => {
				verbIdx = (verbIdx + 1) % verbs.length;
				currentVerb = verbs[verbIdx];
			}, verbInterval);
		}
	};

	const update = (message: string): void => {
		currentVerb = message.replace(/\.{3}$/, '');
		if (!isTTY) {
			stream.write(`  ${message}\n`);
		}
	};

	const stop = (): void => {
		clearTimers();
		if (isTTY) {
			clearLine();
			stream.write('\x1b[?25h');
			setTabTitle('simse');
		}
		startedAt = 0;
		tokenCount = 0;
		stateLabel = 'thinking';
	};

	const succeed = (message: string): void => {
		stop();
		stream.write(`  ${colors.green('●')} ${message}\n`);
	};

	const fail = (message: string): void => {
		stop();
		stream.write(`  ${colors.red('●')} ${message}\n`);
	};

	const setTokens = (tokens: number): void => {
		tokenCount = tokens;
	};

	const setState = (state: string): void => {
		stateLabel = state;
	};

	return Object.freeze({
		start,
		update,
		succeed,
		fail,
		stop,
		setTokens,
		setState,
	});
}

// ---------------------------------------------------------------------------
// Markdown Renderer (for /chain output — streaming uses raw text)
// ---------------------------------------------------------------------------

export interface MarkdownRenderer {
	readonly render: (text: string) => string;
}

export function createMarkdownRenderer(colors: TermColors): MarkdownRenderer {
	const render = (text: string): string => {
		const lines = text.split('\n');
		const output: string[] = [];
		let inCodeBlock = false;
		let codeLang = '';

		for (const line of lines) {
			if (line.trimStart().startsWith('```')) {
				if (!inCodeBlock) {
					inCodeBlock = true;
					codeLang = line.trimStart().slice(3).trim();
					if (codeLang) {
						output.push(`  ${colors.dim(codeLang)}`);
					}
					continue;
				}
				inCodeBlock = false;
				codeLang = '';
				continue;
			}

			if (inCodeBlock) {
				output.push(`  ${colors.dim('│')} ${line}`);
				continue;
			}

			if (/^-{3,}$/.test(line.trim()) || /^\*{3,}$/.test(line.trim())) {
				output.push(colors.dim('─'.repeat(40)));
				continue;
			}

			const headerMatch = line.match(/^(#{1,3})\s+(.+)/);
			if (headerMatch) {
				const level = headerMatch[1].length;
				const text = headerMatch[2];
				if (level === 1) {
					output.push(colors.bold(colors.cyan(text)));
				} else if (level === 2) {
					output.push(colors.bold(text));
				} else {
					output.push(colors.underline(text));
				}
				continue;
			}

			if (line.startsWith('> ')) {
				output.push(`  ${colors.dim('│')} ${colors.dim(line.slice(2))}`);
				continue;
			}

			const listMatch = line.match(/^(\s*)[-*]\s+(.+)/);
			if (listMatch) {
				const indent = listMatch[1];
				const content = formatInline(listMatch[2], colors);
				output.push(`${indent}  - ${content}`);
				continue;
			}

			const orderedMatch = line.match(/^(\s*)\d+\.\s+(.+)/);
			if (orderedMatch) {
				const indent = orderedMatch[1];
				const content = formatInline(orderedMatch[2], colors);
				output.push(`${indent}  ${content}`);
				continue;
			}

			output.push(formatInline(line, colors));
		}

		return output.join('\n');
	};

	return Object.freeze({ render });
}

function formatInline(line: string, colors: TermColors): string {
	let result = line;
	result = result.replace(/\*\*(.+?)\*\*/g, (_, t) => colors.bold(t));
	result = result.replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, (_, t) =>
		colors.italic(t),
	);
	result = result.replace(/`([^`]+)`/g, (_, t) => colors.cyan(t));
	return result;
}

// ---------------------------------------------------------------------------
// Formatters
// ---------------------------------------------------------------------------

export interface BannerOptions {
	readonly version: string;
	readonly dataDir: string;
	readonly workDir: string;
	readonly model?: string;
	readonly toolCount?: number;
	readonly noteCount?: number;
}

/**
 * Render the banner with mascot:
 *
 *   ╭◉─◉╮    simse-code v1.0.0
 *   │▓▓▓│    llama3 · ollama
 *   ╰─┬─╯    D:\GitHub\project
 */
export function renderBanner(
	options: BannerOptions,
	colors: TermColors,
): string {
	const textLines: string[] = [];
	textLines.push(
		`${colors.bold('simse-code')} ${colors.dim(`v${options.version}`)}`,
	);
	if (options.model) {
		textLines.push(options.model);
	}
	textLines.push(colors.dim(options.workDir));

	// Pad mascot + text side by side
	const mascotWidth = 10; // visual width of widest mascot line + padding
	const output: string[] = [];
	const maxLines = Math.max(MASCOT_LINES.length, textLines.length);

	for (let i = 0; i < maxLines; i++) {
		const mascotPart = MASCOT_LINES[i] ?? '';
		const textPart = textLines[i] ?? '';
		const coloredMascot = colors.enabled ? mascotColor(mascotPart) : mascotPart;
		// Pad mascot to fixed width (account for visible chars only)
		const padding = ' '.repeat(Math.max(0, mascotWidth - mascotPart.length));
		output.push(`  ${coloredMascot}${padding}${textPart}`);
	}

	return output.join('\n');
}

export function renderServiceStatus(
	name: string,
	status: 'ok' | 'warn' | 'fail',
	detail: string,
	colors: TermColors,
): string {
	const icon =
		status === 'ok'
			? colors.green('●')
			: status === 'warn'
				? colors.yellow('●')
				: colors.red('●');
	const label = colors.bold(name.padEnd(10));
	return `  ${icon} ${label} ${detail}`;
}

export interface CommandInfo {
	readonly name: string;
	readonly aliases?: readonly string[];
	readonly usage: string;
	readonly description: string;
	readonly category: string;
}

export function renderHelp(
	commands: readonly CommandInfo[],
	categoryLabels: Readonly<Record<string, string>>,
	colors: TermColors,
): string {
	const lines: string[] = [];
	const maxUsage = Math.max(...commands.map((c) => c.usage.length));

	let lastCategory: string | undefined;
	for (const cmd of commands) {
		if (cmd.category !== lastCategory) {
			if (lastCategory) lines.push('');
			lines.push(
				colors.bold(
					colors.cyan(`${categoryLabels[cmd.category] ?? cmd.category}:`),
				),
			);
			lastCategory = cmd.category;
		}
		const aliasStr =
			cmd.aliases && cmd.aliases.length > 0
				? colors.gray(` (${cmd.aliases.join(', ')})`)
				: '';
		lines.push(
			`  ${colors.white(cmd.usage.padEnd(maxUsage + 2))} ${colors.dim(cmd.description)}${aliasStr}`,
		);
	}

	return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Conversation Formatters — Claude Code style
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tool display names — Claude Code style function-call labels
// ---------------------------------------------------------------------------

const TOOL_DISPLAY_NAMES: Readonly<Record<string, string>> = {
	bash: 'Bash',
	shell: 'Bash',
	exec: 'Bash',
	execute: 'Bash',
	run_command: 'Bash',
	vfs_write: 'Write',
	vfs_read: 'Read',
	vfs_delete: 'Delete',
	vfs_rename: 'Rename',
	vfs_list: 'List',
	vfs_stat: 'Stat',
	vfs_search: 'Search',
	vfs_diff: 'Diff',
	vfs_mkdir: 'Mkdir',
	file_write: 'Write',
	file_read: 'Read',
	file_edit: 'Update',
	file_create: 'Write',
	glob: 'Search',
	grep: 'Search',
	memory_search: 'Search',
	memory_add: 'Add',
	memory_list: 'List',
	memory_delete: 'Delete',
	task_list: 'TaskList',
	task_create: 'TaskCreate',
	task_update: 'TaskUpdate',
	task_get: 'TaskGet',
	task_delete: 'TaskDelete',
};

function getToolDisplayName(name: string): string {
	return (
		TOOL_DISPLAY_NAMES[name] ?? name.charAt(0).toUpperCase() + name.slice(1)
	);
}

// ---------------------------------------------------------------------------
// Tool summary verbs — for completed tool result lines
// ---------------------------------------------------------------------------

const TOOL_SUMMARY_VERBS: Readonly<Record<string, string>> = {
	vfs_write: 'Wrote',
	vfs_read: 'Read',
	vfs_delete: 'Deleted',
	file_write: 'Wrote',
	file_read: 'Read',
	file_edit: 'Updated',
	file_create: 'Created',
	glob: 'Found',
	grep: 'Searched',
	memory_search: 'Found',
	memory_add: 'Saved',
};

// ---------------------------------------------------------------------------
// Agent kind display names — Claude Code style
// ---------------------------------------------------------------------------

const KIND_DISPLAY_NAMES: Readonly<Record<string, string>> = {
	read: 'Read',
	edit: 'Update',
	delete: 'Delete',
	move: 'Move',
	search: 'Search',
	execute: 'Bash',
	think: 'Think',
	fetch: 'Fetch',
	other: 'Tool',
};

/**
 * Extract the most useful display argument from a tool call's JSON args.
 * Returns a short string like a file path, command, or query.
 */
function extractPrimaryArg(_name: string, argsStr: string): string {
	try {
		const parsed = JSON.parse(argsStr) as Record<string, unknown>;
		// Try common arg keys in priority order
		for (const key of [
			'path',
			'file_path',
			'filePath',
			'filename',
			'command',
			'query',
			'pattern',
			'name',
			'url',
		]) {
			const val = parsed[key];
			if (typeof val === 'string' && val.length > 0) {
				return val.length > 80 ? `${val.slice(0, 77)}...` : val;
			}
		}
		// Fall back to first string value
		for (const val of Object.values(parsed)) {
			if (typeof val === 'string' && val.length > 0) {
				return val.length > 60 ? `${val.slice(0, 57)}...` : val;
			}
		}
	} catch {
		// Not valid JSON — return trimmed raw string
		if (argsStr && argsStr !== '{}') {
			return argsStr.length > 60 ? `${argsStr.slice(0, 57)}...` : argsStr;
		}
	}
	return '';
}

// ---------------------------------------------------------------------------
// Rich tool call renderers — Claude Code style
// ---------------------------------------------------------------------------

export interface ToolCallCompletedOptions {
	readonly durationMs?: number;
	readonly summary?: string;
	readonly verbose?: boolean;
}

/**
 * Render an active (in-progress) tool call — Claude Code style:
 *   ● Read(src/lib.ts)
 */
export function renderToolCallActive(
	name: string,
	argsStr: string,
	colors: TermColors,
): string {
	const displayName = getToolDisplayName(name);
	const arg = extractPrimaryArg(name, argsStr);
	const display = arg ? `${displayName}(${arg})` : displayName;
	return `  ${colors.magenta('●')} ${colors.bold(display)}`;
}

/**
 * Render a completed tool call — Claude Code style:
 *   ● Read(src/lib.ts)
 *   ⎿ Read 150 lines (42ms)
 */
export function renderToolCallCompleted(
	name: string,
	argsStr: string,
	colors: TermColors,
	options?: ToolCallCompletedOptions,
): string {
	const displayName = getToolDisplayName(name);
	const arg = extractPrimaryArg(name, argsStr);
	const display = arg ? `${displayName}(${arg})` : displayName;

	const parts: string[] = [];
	if (options?.summary) parts.push(options.summary);
	if (options?.durationMs !== undefined) {
		parts.push(formatDuration(options.durationMs));
	}
	const suffix =
		parts.length > 0 ? ` ${colors.dim(`(${parts.join(', ')})`)}` : '';

	return `  ${colors.magenta('●')} ${colors.bold(display)}${suffix}`;
}

/**
 * Render a failed tool call — Claude Code style:
 *   ● Read(src/lib.ts)
 *   ⎿ Error: file not found
 */
export function renderToolCallFailed(
	name: string,
	argsStr: string,
	error: string,
	colors: TermColors,
): string {
	const displayName = getToolDisplayName(name);
	const arg = extractPrimaryArg(name, argsStr);
	const display = arg ? `${displayName}(${arg})` : displayName;
	const errMsg = error.length > 120 ? `${error.slice(0, 117)}...` : error;
	return `  ${colors.red('●')} ${colors.bold(display)}\n    ${colors.dim('⎿')} ${colors.red(errMsg)}`;
}

/**
 * Render a collapsed tool result — Claude Code style:
 *   ⎿ Read 150 lines (ctrl+o to expand)
 */
export function renderToolResultCollapsed(
	name: string,
	output: string,
	isError: boolean,
	colors: TermColors,
): string {
	if (isError) {
		const firstLine = output.split('\n')[0] ?? output;
		const errMsg =
			firstLine.length > 120 ? `${firstLine.slice(0, 117)}...` : firstLine;
		return `    ${colors.dim('⎿')} ${colors.red(errMsg)}`;
	}
	const lineCount = output.split('\n').length;
	const summaryVerb = TOOL_SUMMARY_VERBS[name] ?? '';
	const summary = summaryVerb
		? `${summaryVerb} ${lineCount} line${lineCount !== 1 ? 's' : ''}`
		: `${lineCount} line${lineCount !== 1 ? 's' : ''}`;
	return `    ${colors.dim('⎿')} ${summary} ${colors.dim('(ctrl+o to expand)')}`;
}

/**
 * Render an active agent tool call — Claude Code style:
 *   ● Read(file.ts)
 */
export function renderAgentToolCallActive(
	title: string,
	kind: string,
	colors: TermColors,
	modelBadge?: string,
): string {
	const displayName = KIND_DISPLAY_NAMES[kind] ?? 'Tool';
	const display = title ? `${displayName}(${title})` : displayName;
	const badge = modelBadge ? ` ${colors.dim(modelBadge)}` : '';
	return `  ${colors.magenta('●')} ${colors.bold(display)}${badge}`;
}

/**
 * Render a completed agent tool call — Claude Code style:
 *   ● Read(file.ts)
 *   ⎿ Done (45 tool uses · 106.6k tokens · 3m 37s)
 */
export function renderAgentToolCallCompleted(
	title: string,
	kind: string,
	colors: TermColors,
	options?: ToolCallCompletedOptions,
): string {
	const displayName = KIND_DISPLAY_NAMES[kind] ?? 'Tool';
	const display = title ? `${displayName}(${title})` : displayName;

	const parts: string[] = [];
	if (options?.summary) parts.push(options.summary);
	if (options?.durationMs !== undefined) {
		parts.push(formatDuration(options.durationMs));
	}
	const suffix =
		parts.length > 0 ? ` ${colors.dim(`(${parts.join(' · ')})`)}` : '';

	return `  ${colors.magenta('●')} ${colors.bold(display)}${suffix}`;
}

/**
 * Render a failed agent tool call — Claude Code style:
 *   ● Tool(operation)
 *   ⎿ Error: error msg
 */
export function renderAgentToolCallFailed(
	kind: string,
	error: string,
	colors: TermColors,
): string {
	const displayName = KIND_DISPLAY_NAMES[kind] ?? 'Tool';
	const errMsg = error.length > 120 ? `${error.slice(0, 117)}...` : error;
	return `  ${colors.red('●')} ${colors.bold(displayName)}\n    ${colors.dim('⎿')} ${colors.red(errMsg)}`;
}

/**
 * Render a sub-agent result summary — Claude Code style:
 *   ⎿ Done (45 tool uses · 106.6k tokens · 3m 37s)
 *   (ctrl+o to expand)
 */
export function renderSubagentResult(
	colors: TermColors,
	options?: {
		readonly toolUses?: number;
		readonly tokens?: number;
		readonly durationMs?: number;
	},
): string {
	const parts: string[] = [];
	if (options?.toolUses !== undefined) {
		parts.push(
			`${options.toolUses} tool use${options.toolUses !== 1 ? 's' : ''}`,
		);
	}
	if (options?.tokens !== undefined) {
		const formatted =
			options.tokens >= 1000
				? `${(options.tokens / 1000).toFixed(1)}k`
				: String(options.tokens);
		parts.push(`${formatted} tokens`);
	}
	if (options?.durationMs !== undefined) {
		parts.push(formatDuration(options.durationMs));
	}
	const suffix = parts.length > 0 ? ` (${parts.join(' · ')})` : '';
	const lines: string[] = [];
	lines.push(`    ${colors.dim('⎿')} Done${suffix}`);
	lines.push(`    ${colors.dim('(ctrl+o to expand)')}`);
	return lines.join('\n');
}

export interface RichToolCallOptions {
	readonly verbose?: boolean;
	readonly durationMs?: number;
}

/** Render a tool call — Claude Code style: ● ToolName(primary_arg) */
export function renderToolCall(
	name: string,
	argsStr: string,
	colors: TermColors,
	options?: RichToolCallOptions,
): string {
	const displayName = getToolDisplayName(name);
	const verbose = options?.verbose ?? false;

	let argDisplay = '';
	if (verbose) {
		// Verbose mode: show all args
		try {
			const parsed = JSON.parse(argsStr) as Record<string, unknown>;
			const parts = Object.entries(parsed).map(([k, v]) => {
				const val =
					typeof v === 'string'
						? `"${v.length > 80 ? `${v.slice(0, 77)}...` : v}"`
						: String(v);
				return `${k}: ${val}`;
			});
			if (parts.length > 0) argDisplay = parts.join(', ');
		} catch {
			if (argsStr && argsStr !== '{}') argDisplay = argsStr;
		}
	} else {
		argDisplay = extractPrimaryArg(name, argsStr);
	}

	const display = argDisplay ? `${displayName}(${argDisplay})` : displayName;

	let suffix = '';
	if (options?.durationMs !== undefined) {
		suffix = ` ${colors.dim(`(${formatDuration(options.durationMs)})`)}`;
	}

	return `  ${colors.magenta('●')} ${colors.bold(display)}${suffix}`;
}

export interface RichToolResultOptions {
	readonly verbose?: boolean;
}

/** Render a tool result: ⎿ result text */
export function renderToolResult(
	output: string,
	isError: boolean,
	colors: TermColors,
	options?: RichToolResultOptions,
): string {
	const verbose = options?.verbose ?? false;
	const lines = output.split('\n');
	const maxLines = verbose ? 50 : 8;
	const maxLineLen = verbose ? 500 : 200;
	const displayLines = lines.slice(0, maxLines);
	const remaining = lines.length - maxLines;

	const colorFn = isError ? colors.red : (s: string) => s;

	const firstLine = displayLines[0] ?? '';
	const truncFirst =
		firstLine.length > maxLineLen
			? `${firstLine.slice(0, maxLineLen - 3)}...`
			: firstLine;
	const result: string[] = [`    ${colors.dim('⎿')} ${colorFn(truncFirst)}`];

	for (let i = 1; i < displayLines.length; i++) {
		const line = displayLines[i];
		const truncLine =
			line.length > maxLineLen ? `${line.slice(0, maxLineLen - 3)}...` : line;
		result.push(`      ${colorFn(truncLine)}`);
	}

	if (remaining > 0) {
		result.push(`      ${colors.dim(`... (+${remaining} more lines)`)}`);
	}

	return result.join('\n');
}

/** Render a sub-detail line: ⎿ text */
export function renderDetailLine(text: string, colors: TermColors): string {
	return `    ${colors.dim('⎿')} ${colors.dim(text)}`;
}

/** Render an error message. */
export function renderError(message: string, colors: TermColors): string {
	return `  ${colors.red('●')} ${message}`;
}

/** Render an AI assistant response with markdown formatting (for /chain). */
export function renderAssistantMessage(
	text: string,
	md: MarkdownRenderer,
	_colors: TermColors,
): string {
	const rendered = md.render(text);
	const lines = rendered.split('\n');
	return lines.map((line) => `  ${line}`).join('\n');
}

/** Render a skill loading indicator. */
export function renderSkillLoading(name: string, colors: TermColors): string {
	return `  ${colors.magenta('●')} ${colors.dim('Loading skill:')} ${colors.bold(name)}`;
}

// ---------------------------------------------------------------------------
// Thinking Display — dim italic text, suppressed in normal mode
// ---------------------------------------------------------------------------

/** Render thinking text (visible only in verbose mode). */
export function renderThinking(text: string, colors: TermColors): string {
	const lines = text.split('\n');
	return lines.map((line) => `  ${colors.dim(colors.italic(line))}`).join('\n');
}

// ---------------------------------------------------------------------------
// Context Visualization — 40x2 grid showing context usage
// ---------------------------------------------------------------------------

/**
 * Render a 40x2 colored grid showing context window usage.
 * Each cell = ~1.25% of context. Green = used, dim = free.
 */
export function renderContextGrid(
	usedChars: number,
	maxChars: number,
	colors: TermColors,
): string {
	const totalCells = 80; // 40 columns x 2 rows
	const usedPercent = Math.min(1, usedChars / maxChars);
	const usedCells = Math.round(usedPercent * totalCells);

	const lines: string[] = [];
	lines.push(
		`  ${colors.bold('Context usage:')} ${colors.cyan(`${Math.round(usedPercent * 100)}%`)} (${formatBytes(usedChars)} / ${formatBytes(maxChars)})`,
	);
	lines.push('');

	for (let row = 0; row < 2; row++) {
		let line = '  ';
		for (let col = 0; col < 40; col++) {
			const idx = row * 40 + col;
			if (idx < usedCells) {
				// Color based on fill level
				if (usedPercent > 0.9) {
					line += colors.red('█');
				} else if (usedPercent > 0.7) {
					line += colors.yellow('█');
				} else {
					line += colors.green('█');
				}
			} else {
				line += colors.dim('░');
			}
		}
		lines.push(line);
	}

	return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Permission Dialog — richer than simple allow/deny
// ---------------------------------------------------------------------------

export interface PermissionDialogOptions {
	readonly description: string;
	readonly toolName: string;
	readonly args?: string;
	readonly showDiff?: boolean;
}

/** Render a permission request dialog header. */
export function renderPermissionDialog(
	options: PermissionDialogOptions,
	colors: TermColors,
): string {
	const lines: string[] = [];
	lines.push(
		`\n  ${colors.yellow('⚠')} ${colors.bold('Permission requested:')}`,
	);
	lines.push(`    ${options.description}`);
	if (options.args) {
		const truncArgs =
			options.args.length > 200
				? `${options.args.slice(0, 197)}...`
				: options.args;
		lines.push(`    ${colors.dim(truncArgs)}`);
	}
	lines.push('');

	const choices = options.showDiff
		? '[y]es / [n]o / [d]iff / [e]dit'
		: '[y]es / [n]o';
	lines.push(`  ${colors.dim(choices)}`);

	return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Status Indicators — compact inline widgets
// ---------------------------------------------------------------------------

/** Render a mode indicator badge: [MODE] */
export function renderModeBadge(mode: string, colors: TermColors): string {
	switch (mode) {
		case 'plan':
			return colors.yellow(`[PLAN]`);
		case 'acceptEdits':
			return colors.green(`[AUTO-EDIT]`);
		case 'dontAsk':
			return colors.red(`[YOLO]`);
		case 'verbose':
			return colors.cyan(`[VERBOSE]`);
		default:
			return '';
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
	const mins = Math.floor(ms / 60_000);
	const secs = Math.round((ms % 60_000) / 1000);
	return `${mins}m${secs}s`;
}

function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes}B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}
