// ---------------------------------------------------------------------------
// Command Execution Interface — pluggable backend for VFS command passthrough
// ---------------------------------------------------------------------------

import type { VirtualFS } from './vfs.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ExecResult {
	readonly stdout: string;
	readonly stderr: string;
	readonly exitCode: number;
	readonly filesChanged: readonly string[];
}

export interface ExecOptions {
	readonly cwd?: string;
	readonly env?: Readonly<Record<string, string>>;
	readonly timeout?: number;
	readonly stdin?: string;
}

export interface ExecBackend {
	readonly run: (
		command: string,
		args: readonly string[],
		vfs: VirtualFS,
		options?: ExecOptions,
	) => Promise<ExecResult>;
	readonly dispose: () => Promise<void>;
}

// ---------------------------------------------------------------------------
// VFSExecutor — wraps VFS + backend
// ---------------------------------------------------------------------------

export interface VFSExecutor {
	readonly run: (
		command: string,
		args: readonly string[],
		options?: ExecOptions,
	) => Promise<ExecResult>;
	readonly dispose: () => Promise<void>;
}

export function createVFSExecutor(
	vfs: VirtualFS,
	backend: ExecBackend,
): VFSExecutor {
	return Object.freeze({
		run(
			command: string,
			args: readonly string[],
			options?: ExecOptions,
		): Promise<ExecResult> {
			return backend.run(command, args, vfs, options);
		},
		dispose(): Promise<void> {
			return backend.dispose();
		},
	});
}
