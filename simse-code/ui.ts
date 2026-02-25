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

export function createThinkingSpinner(
	options?: ThinkingSpinnerOptions,
): Spinner {
	const verbs = options?.verbs ?? THINKING_VERBS;
	const verbInterval = options?.verbIntervalMs ?? 3000;
	const inner = createSpinner({
		colors: options?.colors,
		stream: options?.stream,
	});

	let verbTimer: ReturnType<typeof setInterval> | undefined;
	let verbIdx = 0;

	const clearTimers = (): void => {
		if (verbTimer) {
			clearInterval(verbTimer);
			verbTimer = undefined;
		}
	};

	const start = (message?: string): void => {
		clearTimers();

		if (message) {
			inner.start(message);
			return;
		}

		verbIdx = Math.floor(Math.random() * verbs.length);
		inner.start(`${verbs[verbIdx]}...`);

		if (verbs.length > 1) {
			verbTimer = setInterval(() => {
				verbIdx = (verbIdx + 1) % verbs.length;
				inner.update(`${verbs[verbIdx]}...`);
			}, verbInterval);
		}
	};

	const update = (message: string): void => {
		clearTimers();
		inner.update(message);
	};

	const stop = (): void => {
		clearTimers();
		inner.stop();
	};

	const succeed = (message: string): void => {
		clearTimers();
		inner.succeed(message);
	};

	const fail = (message: string): void => {
		clearTimers();
		inner.fail(message);
	};

	return Object.freeze({ start, update, succeed, fail, stop });
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

/** Render a tool call: ● ToolName(args) */
export function renderToolCall(
	name: string,
	argsStr: string,
	colors: TermColors,
): string {
	let formattedArgs = '';
	try {
		const parsed = JSON.parse(argsStr) as Record<string, unknown>;
		const parts = Object.entries(parsed).map(([k, v]) => {
			const val = typeof v === 'string' ? `"${v}"` : String(v);
			return `${k}: ${val}`;
		});
		if (parts.length > 0) {
			formattedArgs = `(${parts.join(', ')})`;
		}
	} catch {
		if (argsStr && argsStr !== '{}') {
			formattedArgs = `(${argsStr})`;
		}
	}

	const full = `${name}${formattedArgs}`;
	const display = full.length > 120 ? `${full.slice(0, 117)}...` : full;
	return `  ${colors.magenta('●')} ${colors.bold(display)}`;
}

/** Render a tool result: ⎿ result text */
export function renderToolResult(
	output: string,
	isError: boolean,
	colors: TermColors,
): string {
	const lines = output.split('\n');
	const maxLines = 8;
	const displayLines = lines.slice(0, maxLines);
	const remaining = lines.length - maxLines;

	const colorFn = isError ? colors.red : (s: string) => s;

	const firstLine = displayLines[0] ?? '';
	const truncFirst =
		firstLine.length > 200 ? `${firstLine.slice(0, 197)}...` : firstLine;
	const result: string[] = [`    ${colors.dim('⎿')} ${colorFn(truncFirst)}`];

	for (let i = 1; i < displayLines.length; i++) {
		const line = displayLines[i];
		const truncLine = line.length > 200 ? `${line.slice(0, 197)}...` : line;
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
