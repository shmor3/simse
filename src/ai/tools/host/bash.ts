// ---------------------------------------------------------------------------
// Bash Tool
//
// Registers a shell-execution tool on a ToolRegistry. Uses Bun.spawn for
// subprocess management with timeout and output truncation support.
// ---------------------------------------------------------------------------

import { toError } from '../../../errors/base.js';
import type { ToolRegistry } from '../types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface BashToolOptions {
	readonly workingDirectory: string;
	readonly defaultTimeoutMs?: number;
	readonly maxOutputBytes?: number;
	readonly env?: Readonly<Record<string, string>>;
	readonly shell?: string;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_MAX_OUTPUT_BYTES = 50_000;

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerBashTool(
	registry: ToolRegistry,
	options: BashToolOptions,
): void {
	const {
		workingDirectory,
		defaultTimeoutMs = DEFAULT_TIMEOUT_MS,
		maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES,
		env,
		shell = process.platform === 'win32' ? 'bash' : '/bin/sh',
	} = options;

	registry.register(
		{
			name: 'bash',
			description:
				'Execute a shell command. Returns stdout/stderr combined output with the exit code.',
			parameters: {
				command: {
					type: 'string',
					description: 'The shell command to execute',
					required: true,
				},
				timeout: {
					type: 'number',
					description: `Timeout in milliseconds (default: ${defaultTimeoutMs})`,
				},
				cwd: {
					type: 'string',
					description: `Working directory (default: ${workingDirectory})`,
				},
			},
			category: 'execute',
			annotations: { destructive: true },
		},
		async (args) => {
			const command = String(args.command ?? '');
			const timeout =
				typeof args.timeout === 'number' ? args.timeout : defaultTimeoutMs;
			const cwd = typeof args.cwd === 'string' ? args.cwd : workingDirectory;

			try {
				const proc = Bun.spawn([shell, '-c', command], {
					cwd,
					env: env ? { ...process.env, ...env } : undefined,
					stdout: 'pipe',
					stderr: 'pipe',
				});

				// Race process completion against timeout
				const timeoutSentinel = Symbol('timeout');
				const timeoutPromise = new Promise<typeof timeoutSentinel>(
					(resolve) => {
						setTimeout(() => resolve(timeoutSentinel), timeout);
					},
				);

				const processPromise = (async () => {
					const [stdout, stderr] = await Promise.all([
						new Response(proc.stdout).text(),
						new Response(proc.stderr).text(),
					]);
					const exitCode = await proc.exited;
					return { stdout, stderr, exitCode };
				})();

				const winner = await Promise.race([processPromise, timeoutPromise]);

				const timedOut = winner === timeoutSentinel;
				if (timedOut) {
					proc.kill();
				}

				// Collect whatever output is available
				const { stdout, stderr, exitCode } = timedOut
					? {
							stdout: '',
							stderr: '',
							exitCode: -1,
						}
					: (winner as Awaited<typeof processPromise>);

				let output = stdout + stderr;

				// Truncate if output exceeds limit
				const byteLength = Buffer.byteLength(output, 'utf-8');
				if (byteLength > maxOutputBytes) {
					const buf = Buffer.from(output, 'utf-8');
					output =
						buf.subarray(0, maxOutputBytes).toString('utf-8') +
						`\n[truncated: ${byteLength} bytes total, showing first ${maxOutputBytes}]`;
				}

				if (timedOut) {
					return `[timeout after ${timeout}ms]\n${output}`;
				}

				if (exitCode !== 0) {
					return `[exit code ${exitCode}]\n${output}`;
				}

				return output;
			} catch (err) {
				throw toError(err);
			}
		},
	);
}
