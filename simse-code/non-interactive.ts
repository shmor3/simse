/**
 * SimSE Code — Non-Interactive Mode
 *
 * Supports -p <prompt> for single-shot generation without REPL.
 * Outputs text or JSON and exits.
 * No external deps.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface NonInteractiveArgs {
	readonly prompt: string;
	readonly format: 'text' | 'json';
	readonly serverName?: string;
	readonly agentId?: string;
}

export interface NonInteractiveResult {
	readonly output: string;
	readonly model: string;
	readonly durationMs: number;
	readonly exitCode: number;
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

/**
 * Parse non-interactive arguments from process.argv.
 * Returns undefined if no -p flag is found (i.e. interactive mode).
 */
export function parseNonInteractiveArgs(
	argv: readonly string[],
): NonInteractiveArgs | undefined {
	const args = argv.slice(2);
	let prompt: string | undefined;
	let format: 'text' | 'json' = 'text';
	let serverName: string | undefined;
	let agentId: string | undefined;

	for (let i = 0; i < args.length; i++) {
		const arg = args[i];
		const next = args[i + 1];

		if ((arg === '-p' || arg === '--prompt') && next) {
			prompt = args[++i];
		} else if (arg === '--format' && next) {
			const fmt = args[++i];
			if (fmt === 'json' || fmt === 'text') format = fmt;
		} else if (arg === '--server' && next) {
			serverName = args[++i];
		} else if (arg === '--agent' && next) {
			agentId = args[++i];
		}
	}

	if (!prompt) return undefined;

	return Object.freeze({ prompt, format, serverName, agentId });
}

/**
 * Format the result for output based on the requested format.
 */
export function formatNonInteractiveResult(
	result: NonInteractiveResult,
	format: 'text' | 'json',
): string {
	if (format === 'json') {
		return JSON.stringify(
			{
				output: result.output,
				model: result.model,
				durationMs: result.durationMs,
			},
			null,
			'\t',
		);
	}
	return result.output;
}

/**
 * Check if the current invocation is non-interactive.
 */
export function isNonInteractive(argv: readonly string[]): boolean {
	return argv.includes('-p') || argv.includes('--prompt');
}
