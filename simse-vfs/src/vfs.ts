// ---------------------------------------------------------------------------
// Virtual Filesystem — async client wrapper over Rust subprocess
// ---------------------------------------------------------------------------

import { createVFSClient, type VFSClientEvent } from './client.js';
import type { Logger } from './logger.js';
import type {
	VFSCallbacks,
	VFSCopyOptions,
	VFSDeleteOptions,
	VFSDiffOptions,
	VFSDiffResult,
	VFSDirEntry,
	VFSHistoryEntry,
	VFSHistoryOptions,
	VFSLimits,
	VFSMkdirOptions,
	VFSOp,
	VFSReaddirOptions,
	VFSReadResult,
	VFSSearchOptions,
	VFSSearchResult,
	VFSSnapshot,
	VFSStat,
	VFSWriteEvent,
	VFSWriteOptions,
} from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface VirtualFSOptions {
	readonly limits?: VFSLimits;
	readonly history?: VFSHistoryOptions;
	readonly logger?: Logger;
	readonly enginePath?: string;
	readonly onFileWrite?: (event: VFSWriteEvent) => void;
	readonly callbacks?: VFSCallbacks;
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

export interface VFSMetrics {
	readonly totalSize: number;
	readonly nodeCount: number;
	readonly fileCount: number;
	readonly directoryCount: number;
}

// ---------------------------------------------------------------------------
// VirtualFS interface (all methods async)
// ---------------------------------------------------------------------------

export interface VirtualFS {
	readonly readFile: (path: string) => Promise<VFSReadResult>;
	readonly writeFile: (
		path: string,
		content: string | Uint8Array,
		options?: VFSWriteOptions,
	) => Promise<void>;
	readonly appendFile: (path: string, content: string) => Promise<void>;
	readonly deleteFile: (path: string) => Promise<boolean>;

	readonly mkdir: (path: string, options?: VFSMkdirOptions) => Promise<void>;
	readonly readdir: (
		path: string,
		options?: VFSReaddirOptions,
	) => Promise<readonly VFSDirEntry[]>;
	readonly rmdir: (
		path: string,
		options?: VFSDeleteOptions,
	) => Promise<boolean>;

	readonly stat: (path: string) => Promise<VFSStat>;
	readonly exists: (path: string) => Promise<boolean>;
	readonly rename: (oldPath: string, newPath: string) => Promise<void>;
	readonly copy: (
		src: string,
		dest: string,
		options?: VFSCopyOptions,
	) => Promise<void>;

	readonly glob: (
		pattern: string | readonly string[],
	) => Promise<readonly string[]>;
	readonly tree: (path?: string) => Promise<string>;
	readonly du: (path: string) => Promise<number>;
	readonly search: (
		query: string,
		options?: VFSSearchOptions,
	) => Promise<readonly VFSSearchResult[] | number>;

	readonly history: (
		path: string,
	) => Promise<readonly VFSHistoryEntry[]>;
	readonly diff: (
		oldPath: string,
		newPath: string,
		options?: VFSDiffOptions,
	) => Promise<VFSDiffResult>;
	readonly diffVersions: (
		path: string,
		oldVersion: number,
		newVersion?: number,
		options?: VFSDiffOptions,
	) => Promise<VFSDiffResult>;
	readonly checkout: (path: string, version: number) => Promise<void>;

	readonly snapshot: () => Promise<VFSSnapshot>;
	readonly restore: (snapshot: VFSSnapshot) => Promise<void>;
	readonly clear: () => Promise<void>;
	readonly transaction: (ops: readonly VFSOp[]) => Promise<void>;

	readonly metrics: () => Promise<VFSMetrics>;
	readonly dispose: () => Promise<void>;
}

// ---------------------------------------------------------------------------
// Binary encoding helpers
// ---------------------------------------------------------------------------

const encodeBase64 = (bytes: Uint8Array): string =>
	globalThis.Buffer.from(bytes).toString('base64');

const decodeBase64 = (b64: string): Uint8Array =>
	new Uint8Array(globalThis.Buffer.from(b64, 'base64'));

// ---------------------------------------------------------------------------
// Transaction op serialization (encode Uint8Array content to base64)
// ---------------------------------------------------------------------------

interface SerializedOp {
	readonly type: string;
	readonly path?: string;
	readonly content?: string;
	readonly contentType?: string;
	readonly oldPath?: string;
	readonly newPath?: string;
	readonly src?: string;
	readonly dest?: string;
}

const serializeOps = (ops: readonly VFSOp[]): readonly SerializedOp[] =>
	ops.map((op) => {
		if (op.type === 'writeFile') {
			if (op.content instanceof Uint8Array) {
				return {
					type: op.type,
					path: op.path,
					content: encodeBase64(op.content),
					contentType: 'binary',
				};
			}
			return {
				type: op.type,
				path: op.path,
				content: op.content,
				contentType: 'text',
			};
		}
		return op as SerializedOp;
	});

// ---------------------------------------------------------------------------
// Event routing
// ---------------------------------------------------------------------------

const routeEvent = (
	event: VFSClientEvent,
	callbacks: VFSCallbacks | undefined,
	onFileWrite: ((event: VFSWriteEvent) => void) | undefined,
): void => {
	if (!callbacks && !onFileWrite) return;

	switch (event.type) {
		case 'write':
			callbacks?.onWrite?.(event.path, event.size ?? 0);
			onFileWrite?.({
				path: event.path,
				contentType: 'text',
				size: event.size ?? 0,
				isNew: false,
			});
			break;
		case 'delete':
			callbacks?.onDelete?.(event.path);
			break;
		case 'rename':
			if (event.oldPath && event.newPath) {
				callbacks?.onRename?.(event.oldPath, event.newPath);
			}
			break;
		case 'mkdir':
			callbacks?.onMkdir?.(event.path);
			break;
	}
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export async function createVirtualFS(
	options: VirtualFSOptions = {},
): Promise<VirtualFS> {
	const enginePath = options.enginePath ?? 'simse-vfs-engine';

	const client = await createVFSClient({
		enginePath,
		limits: options.limits,
		history: options.history,
		logger: options.logger,
		onEvent: (event) =>
			routeEvent(event, options.callbacks, options.onFileWrite),
	});

	return Object.freeze({
		// ----- File operations ------------------------------------------------

		readFile: async (path: string): Promise<VFSReadResult> => {
			const result = await client.request<{
				contentType: string;
				text?: string;
				data?: string;
				size: number;
			}>('vfs/readFile', { path });

			if (result.contentType === 'binary' && result.data) {
				return {
					contentType: 'binary',
					text: undefined,
					data: decodeBase64(result.data),
					size: result.size,
				};
			}
			return {
				contentType: 'text',
				text: result.text ?? '',
				data: undefined,
				size: result.size,
			};
		},

		writeFile: async (
			path: string,
			content: string | Uint8Array,
			opts?: VFSWriteOptions,
		): Promise<void> => {
			const params: Record<string, unknown> = { path };
			if (content instanceof Uint8Array) {
				params.content = encodeBase64(content);
				params.contentType = opts?.contentType ?? 'binary';
			} else {
				params.content = content;
				params.contentType = opts?.contentType ?? 'text';
			}
			if (opts?.createParents !== undefined) {
				params.createParents = opts.createParents;
			}
			await client.request('vfs/writeFile', params);
		},

		appendFile: async (path: string, content: string): Promise<void> => {
			await client.request('vfs/appendFile', { path, content });
		},

		deleteFile: async (path: string): Promise<boolean> => {
			const result = await client.request<{ deleted: boolean }>(
				'vfs/deleteFile',
				{ path },
			);
			return result.deleted;
		},

		// ----- Directory operations -------------------------------------------

		mkdir: async (
			path: string,
			opts?: VFSMkdirOptions,
		): Promise<void> => {
			const params: Record<string, unknown> = { path };
			if (opts?.recursive !== undefined) {
				params.recursive = opts.recursive;
			}
			await client.request('vfs/mkdir', params);
		},

		readdir: async (
			path: string,
			opts?: VFSReaddirOptions,
		): Promise<readonly VFSDirEntry[]> => {
			const result = await client.request<{
				entries: readonly VFSDirEntry[];
			}>('vfs/readdir', {
				path,
				recursive: opts?.recursive,
			});
			return result.entries;
		},

		rmdir: async (
			path: string,
			opts?: VFSDeleteOptions,
		): Promise<boolean> => {
			const result = await client.request<{ deleted: boolean }>(
				'vfs/rmdir',
				{
					path,
					recursive: opts?.recursive,
				},
			);
			return result.deleted;
		},

		// ----- Stat / exists / rename / copy ----------------------------------

		stat: async (path: string): Promise<VFSStat> => {
			return client.request<VFSStat>('vfs/stat', { path });
		},

		exists: async (path: string): Promise<boolean> => {
			const result = await client.request<{ exists: boolean }>(
				'vfs/exists',
				{ path },
			);
			return result.exists;
		},

		rename: async (oldPath: string, newPath: string): Promise<void> => {
			await client.request('vfs/rename', { oldPath, newPath });
		},

		copy: async (
			src: string,
			dest: string,
			opts?: VFSCopyOptions,
		): Promise<void> => {
			const params: Record<string, unknown> = { src, dest };
			if (opts?.overwrite !== undefined) params.overwrite = opts.overwrite;
			if (opts?.recursive !== undefined) params.recursive = opts.recursive;
			await client.request('vfs/copy', params);
		},

		// ----- Glob / tree / du / search --------------------------------------

		glob: async (
			pattern: string | readonly string[],
		): Promise<readonly string[]> => {
			const result = await client.request<{
				matches: readonly string[];
			}>('vfs/glob', { pattern });
			return result.matches;
		},

		tree: async (path?: string): Promise<string> => {
			const result = await client.request<{ tree: string }>('vfs/tree', {
				path,
			});
			return result.tree;
		},

		du: async (path: string): Promise<number> => {
			const result = await client.request<{ size: number }>('vfs/du', {
				path,
			});
			return result.size;
		},

		search: async (
			query: string,
			opts?: VFSSearchOptions,
		): Promise<readonly VFSSearchResult[] | number> => {
			const params: Record<string, unknown> = { query };
			if (opts?.glob !== undefined) params.glob = opts.glob;
			if (opts?.maxResults !== undefined)
				params.maxResults = opts.maxResults;
			if (opts?.mode !== undefined) params.mode = opts.mode;
			if (opts?.contextBefore !== undefined)
				params.contextBefore = opts.contextBefore;
			if (opts?.contextAfter !== undefined)
				params.contextAfter = opts.contextAfter;
			if (opts?.countOnly !== undefined) params.countOnly = opts.countOnly;

			if (opts?.countOnly) {
				const result = await client.request<{ count: number }>(
					'vfs/search',
					params,
				);
				return result.count;
			}

			const result = await client.request<{
				results: readonly VFSSearchResult[];
			}>('vfs/search', params);
			return result.results;
		},

		// ----- History / diff / checkout --------------------------------------

		history: async (
			path: string,
		): Promise<readonly VFSHistoryEntry[]> => {
			const result = await client.request<{
				entries: readonly VFSHistoryEntry[];
			}>('vfs/history', { path });
			return result.entries;
		},

		diff: async (
			oldPath: string,
			newPath: string,
			opts?: VFSDiffOptions,
		): Promise<VFSDiffResult> => {
			const params: Record<string, unknown> = { oldPath, newPath };
			if (opts?.context !== undefined) params.context = opts.context;
			return client.request<VFSDiffResult>('vfs/diff', params);
		},

		diffVersions: async (
			path: string,
			oldVersion: number,
			newVersion?: number,
			opts?: VFSDiffOptions,
		): Promise<VFSDiffResult> => {
			const params: Record<string, unknown> = { path, oldVersion };
			if (newVersion !== undefined) params.newVersion = newVersion;
			if (opts?.context !== undefined) params.context = opts.context;
			return client.request<VFSDiffResult>('vfs/diffVersions', params);
		},

		checkout: async (path: string, version: number): Promise<void> => {
			await client.request('vfs/checkout', { path, version });
		},

		// ----- Snapshot / restore / clear / transaction -----------------------

		snapshot: async (): Promise<VFSSnapshot> => {
			return client.request<VFSSnapshot>('vfs/snapshot', {});
		},

		restore: async (snapshot: VFSSnapshot): Promise<void> => {
			await client.request('vfs/restore', { snapshot });
		},

		clear: async (): Promise<void> => {
			await client.request('vfs/clear', {});
		},

		transaction: async (ops: readonly VFSOp[]): Promise<void> => {
			await client.request('vfs/transaction', {
				ops: serializeOps(ops),
			});
		},

		// ----- Metrics --------------------------------------------------------

		metrics: async (): Promise<VFSMetrics> => {
			return client.request<VFSMetrics>('vfs/metrics', {});
		},

		// ----- Lifecycle ------------------------------------------------------

		dispose: (): Promise<void> => {
			return client.dispose();
		},
	});
}
