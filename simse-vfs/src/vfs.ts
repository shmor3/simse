// ---------------------------------------------------------------------------
// Virtual Filesystem — createVirtualFS factory
// ---------------------------------------------------------------------------

import { createVFSError } from './errors.js';
import type { Logger } from './logger.js';
import { createNoopLogger } from './logger.js';
import {
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	VFS_ROOT,
	validatePath,
} from './path-utils.js';
import type {
	VFSContentType,
	VFSCopyOptions,
	VFSDeleteOptions,
	VFSDiffHunk,
	VFSDiffLine,
	VFSDiffOptions,
	VFSDiffResult,
	VFSDirEntry,
	VFSHistoryEntry,
	VFSHistoryOptions,
	VFSLimits,
	VFSMkdirOptions,
	VFSNodeType,
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
	readonly onFileWrite?: (event: VFSWriteEvent) => void;
}

// ---------------------------------------------------------------------------
// VirtualFS interface
// ---------------------------------------------------------------------------

export interface VirtualFS {
	readonly readFile: (path: string) => VFSReadResult;
	readonly writeFile: (
		path: string,
		content: string | Uint8Array,
		options?: VFSWriteOptions,
	) => void;
	readonly appendFile: (path: string, content: string) => void;
	readonly deleteFile: (path: string) => boolean;

	readonly mkdir: (path: string, options?: VFSMkdirOptions) => void;
	readonly readdir: (
		path: string,
		options?: VFSReaddirOptions,
	) => readonly VFSDirEntry[];
	readonly rmdir: (path: string, options?: VFSDeleteOptions) => boolean;

	readonly stat: (path: string) => VFSStat;
	readonly exists: (path: string) => boolean;
	readonly rename: (oldPath: string, newPath: string) => void;
	readonly copy: (src: string, dest: string, options?: VFSCopyOptions) => void;

	readonly glob: (pattern: string | readonly string[]) => readonly string[];
	readonly tree: (path?: string) => string;
	readonly du: (path: string) => number;
	readonly search: (
		query: string,
		options?: VFSSearchOptions,
	) => readonly VFSSearchResult[] | number;

	readonly history: (path: string) => readonly VFSHistoryEntry[];
	readonly diff: (
		oldPath: string,
		newPath: string,
		options?: VFSDiffOptions,
	) => VFSDiffResult;
	readonly diffVersions: (
		path: string,
		oldVersion: number,
		newVersion?: number,
		options?: VFSDiffOptions,
	) => VFSDiffResult;
	readonly checkout: (path: string, version: number) => void;

	readonly snapshot: () => VFSSnapshot;
	readonly restore: (snapshot: VFSSnapshot) => void;
	readonly clear: () => void;

	readonly totalSize: number;
	readonly nodeCount: number;
	readonly fileCount: number;
	readonly directoryCount: number;
}

// ---------------------------------------------------------------------------
// Internal node
// ---------------------------------------------------------------------------

interface InternalNode {
	readonly type: VFSNodeType;
	readonly contentType: VFSContentType;
	readonly text: string | undefined;
	readonly data: Uint8Array | undefined;
	readonly size: number;
	readonly createdAt: number;
	readonly modifiedAt: number;
}

// ---------------------------------------------------------------------------
// Glob matching
// ---------------------------------------------------------------------------

const expandBraces = (pattern: string): string[] => {
	const braceStart = pattern.indexOf('{');
	if (braceStart < 0) return [pattern];

	// Find matching closing brace (handle nesting)
	let depth = 0;
	let braceEnd = -1;
	for (let i = braceStart; i < pattern.length; i++) {
		if (pattern[i] === '{') depth++;
		else if (pattern[i] === '}') {
			depth--;
			if (depth === 0) {
				braceEnd = i;
				break;
			}
		}
	}

	if (braceEnd < 0) return [pattern]; // unmatched brace, treat as literal

	const prefix = pattern.slice(0, braceStart);
	const suffix = pattern.slice(braceEnd + 1);
	const alternatives = pattern.slice(braceStart + 1, braceEnd).split(',');

	const results: string[] = [];
	for (const alt of alternatives) {
		// Recursively expand in case of nested braces or suffix braces
		for (const expanded of expandBraces(prefix + alt.trim() + suffix)) {
			results.push(expanded);
		}
	}

	return results;
};

const matchSegment = (segment: string, pattern: string): boolean => {
	let si = 0;
	let pi = 0;
	let starSi = -1;
	let starPi = -1;

	while (si < segment.length) {
		if (pi < pattern.length && pattern[pi] === '?') {
			si++;
			pi++;
		} else if (pi < pattern.length && pattern[pi] === '*') {
			starPi = pi;
			starSi = si;
			pi++;
		} else if (pi < pattern.length && segment[si] === pattern[pi]) {
			si++;
			pi++;
		} else if (starPi >= 0) {
			pi = starPi + 1;
			starSi++;
			si = starSi;
		} else {
			return false;
		}
	}

	while (pi < pattern.length && pattern[pi] === '*') {
		pi++;
	}

	return pi === pattern.length;
};

const matchGlob = (filePath: string, pattern: string): boolean => {
	const expanded = expandBraces(pattern);
	const pathParts = toLocalPath(filePath).split('/').filter(Boolean);

	for (const exp of expanded) {
		const normalizedPattern = normalizePath(exp);
		const patternParts = toLocalPath(normalizedPattern)
			.split('/')
			.filter(Boolean);
		if (matchParts(pathParts, 0, patternParts, 0)) {
			return true;
		}
	}
	return false;
};

const matchParts = (
	pathParts: string[],
	pi: number,
	patternParts: string[],
	gi: number,
): boolean => {
	while (pi < pathParts.length && gi < patternParts.length) {
		if (patternParts[gi] === '**') {
			// ** matches zero or more path segments
			if (gi === patternParts.length - 1) return true;
			for (let skip = pi; skip <= pathParts.length; skip++) {
				if (matchParts(pathParts, skip, patternParts, gi + 1)) {
					return true;
				}
			}
			return false;
		}
		if (!matchSegment(pathParts[pi], patternParts[gi])) {
			return false;
		}
		pi++;
		gi++;
	}

	// Consume trailing ** patterns
	while (gi < patternParts.length && patternParts[gi] === '**') {
		gi++;
	}

	return pi === pathParts.length && gi === patternParts.length;
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createVirtualFS(options: VirtualFSOptions = {}): VirtualFS {
	const logger = (options.logger ?? createNoopLogger()).child('vfs');
	const encoder = new TextEncoder();

	const maxEntriesPerFile = options.history?.maxEntriesPerFile ?? 50;
	const onFileWrite = options.onFileWrite;

	const fileHistory = new Map<string, VFSHistoryEntry[]>();

	const limits: Required<VFSLimits> = {
		maxFileSize: options.limits?.maxFileSize ?? 10_485_760,
		maxTotalSize: options.limits?.maxTotalSize ?? 104_857_600,
		maxPathDepth: options.limits?.maxPathDepth ?? 32,
		maxNameLength: options.limits?.maxNameLength ?? 255,
		maxNodeCount: options.limits?.maxNodeCount ?? 10_000,
		maxPathLength: options.limits?.maxPathLength ?? 1024,
		maxDiffLines: options.limits?.maxDiffLines ?? 10_000,
	};

	const nodes = new Map<string, InternalNode>();
	let currentTotalSize = 0;
	let currentFileCount = 0;
	let currentDirCount = 0;

	const initRoot = (): void => {
		const now = Date.now();
		nodes.set(VFS_ROOT, {
			type: 'directory',
			contentType: 'text',
			text: undefined,
			data: undefined,
			size: 0,
			createdAt: now,
			modifiedAt: now,
		});
		currentDirCount = 1;
	};

	initRoot();

	// -- Helpers ----------------------------------------------------------

	const resolve = (path: string): string => normalizePath(path);

	const assertValidPath = (path: string): string => {
		const normalized = resolve(path);
		const error = validatePath(normalized, limits);
		if (error) {
			throw createVFSError(error, {
				code: 'VFS_INVALID_PATH',
				metadata: { path },
			});
		}
		return normalized;
	};

	const assertNodeExists = (path: string): InternalNode => {
		const node = nodes.get(path);
		if (!node) {
			throw createVFSError(`No such file or directory: ${path}`, {
				code: 'VFS_NOT_FOUND',
				statusCode: 404,
				metadata: { path },
			});
		}
		return node;
	};

	const assertIsFile = (path: string, node: InternalNode): void => {
		if (node.type !== 'file') {
			throw createVFSError(`Not a file: ${path}`, {
				code: 'VFS_NOT_FILE',
				metadata: { path },
			});
		}
	};

	const assertIsDirectory = (path: string, node: InternalNode): void => {
		if (node.type !== 'directory') {
			throw createVFSError(`Not a directory: ${path}`, {
				code: 'VFS_NOT_DIRECTORY',
				metadata: { path },
			});
		}
	};

	const assertNodeLimit = (): void => {
		if (nodes.size >= limits.maxNodeCount) {
			throw createVFSError(
				`Maximum node count exceeded (${limits.maxNodeCount})`,
				{ code: 'VFS_LIMIT_EXCEEDED', metadata: { limit: 'maxNodeCount' } },
			);
		}
	};

	const computeByteSize = (content: string | Uint8Array): number =>
		content instanceof Uint8Array
			? content.byteLength
			: encoder.encode(content).byteLength;

	const assertFileSize = (size: number, path: string): void => {
		if (size > limits.maxFileSize) {
			throw createVFSError(
				`File size ${size} exceeds limit (${limits.maxFileSize}): ${path}`,
				{
					code: 'VFS_LIMIT_EXCEEDED',
					metadata: { limit: 'maxFileSize', path, size },
				},
			);
		}
	};

	const assertTotalSize = (additionalBytes: number): void => {
		if (currentTotalSize + additionalBytes > limits.maxTotalSize) {
			throw createVFSError(
				`Total storage size would exceed limit (${limits.maxTotalSize})`,
				{ code: 'VFS_LIMIT_EXCEEDED', metadata: { limit: 'maxTotalSize' } },
			);
		}
	};

	const ensureParentExists = (path: string): void => {
		const parent = parentPath(path);
		if (parent === undefined) return;
		const parentNode = nodes.get(parent);
		if (!parentNode) {
			throw createVFSError(`Parent directory does not exist: ${parent}`, {
				code: 'VFS_NOT_FOUND',
				statusCode: 404,
				metadata: { path: parent },
			});
		}
		assertIsDirectory(parent, parentNode);
	};

	const createParents = (path: string): void => {
		const ancestors = ancestorPaths(path);
		const ts = Date.now();
		for (const ancestor of ancestors) {
			if (!nodes.has(ancestor)) {
				assertNodeLimit();
				nodes.set(ancestor, {
					type: 'directory',
					contentType: 'text',
					text: undefined,
					data: undefined,
					size: 0,
					createdAt: ts,
					modifiedAt: ts,
				});
				currentDirCount++;
			}
		}
	};

	const getNode = (path: string): InternalNode => {
		const node = nodes.get(path);
		if (!node) {
			throw createVFSError(`Internal error: missing node ${path}`, {
				code: 'VFS_ERROR',
			});
		}
		return node;
	};

	const getDirectChildren = (dirPath: string): string[] => {
		const prefix = dirPath === VFS_ROOT ? VFS_ROOT : `${dirPath}/`;
		const result: string[] = [];
		for (const key of nodes.keys()) {
			if (key === dirPath) continue;
			if (!key.startsWith(prefix)) continue;
			const remainder = key.slice(prefix.length);
			if (!remainder.includes('/')) {
				result.push(key);
			}
		}
		return result;
	};

	const getDescendants = (dirPath: string): string[] => {
		const prefix = dirPath === VFS_ROOT ? VFS_ROOT : `${dirPath}/`;
		const result: string[] = [];
		for (const key of nodes.keys()) {
			if (key === dirPath) continue;
			if (key.startsWith(prefix)) {
				result.push(key);
			}
		}
		return result;
	};

	const updateParentModifiedAt = (path: string, ts: number): void => {
		const parent = parentPath(path);
		if (parent) {
			const parentNode = nodes.get(parent);
			if (parentNode) {
				nodes.set(parent, { ...parentNode, modifiedAt: ts });
			}
		}
	};

	const recordHistory = (path: string, node: InternalNode): void => {
		if (node.type !== 'file') return;

		let entries = fileHistory.get(path);
		if (!entries) {
			entries = [];
			fileHistory.set(path, entries);
		}

		const version = entries.length + 1;
		const entry: VFSHistoryEntry = Object.freeze({
			version,
			contentType: node.contentType,
			text: node.text,
			base64: node.data ? Buffer.from(node.data).toString('base64') : undefined,
			size: node.size,
			timestamp: node.modifiedAt,
		});

		entries.push(entry);

		// Trim to limit
		if (entries.length > maxEntriesPerFile) {
			entries.splice(0, entries.length - maxEntriesPerFile);
		}
	};

	// -- File operations --------------------------------------------------

	const readFile = (path: string): VFSReadResult => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsFile(normalized, node);

		if (node.contentType === 'binary') {
			return Object.freeze({
				contentType: 'binary' as const,
				text: undefined,
				data: new Uint8Array(node.data ?? new Uint8Array(0)),
				size: node.size,
			});
		}

		return Object.freeze({
			contentType: 'text' as const,
			text: node.text ?? '',
			data: undefined,
			size: node.size,
		});
	};

	const writeFile = (
		path: string,
		content: string | Uint8Array,
		writeOptions?: VFSWriteOptions,
	): void => {
		const normalized = assertValidPath(path);
		if (normalized === VFS_ROOT) {
			throw createVFSError('Cannot write to root directory as a file', {
				code: 'VFS_INVALID_OPERATION',
			});
		}

		const contentType: VFSContentType =
			writeOptions?.contentType ??
			(content instanceof Uint8Array ? 'binary' : 'text');

		const newSize = computeByteSize(content);
		assertFileSize(newSize, normalized);

		const existing = nodes.get(normalized);
		if (existing && existing.type === 'directory') {
			throw createVFSError(
				`Cannot overwrite directory with file: ${normalized}`,
				{ code: 'VFS_NOT_FILE', metadata: { path: normalized } },
			);
		}

		const oldSize = existing?.size ?? 0;
		const sizeDelta = newSize - oldSize;
		if (sizeDelta > 0) assertTotalSize(sizeDelta);

		if (writeOptions?.createParents) {
			createParents(normalized);
		} else {
			ensureParentExists(normalized);
		}

		if (!existing) {
			assertNodeLimit();
			currentFileCount++;
		} else if (existing.type === 'file') {
			recordHistory(normalized, existing);
		}

		const ts = Date.now();
		const isText = contentType === 'text';

		nodes.set(normalized, {
			type: 'file',
			contentType,
			text: isText ? (content as string) : undefined,
			data: !isText ? new Uint8Array(content as Uint8Array) : undefined,
			size: newSize,
			createdAt: existing?.createdAt ?? ts,
			modifiedAt: ts,
		});

		currentTotalSize += sizeDelta;
		updateParentModifiedAt(normalized, ts);

		onFileWrite?.({
			path: normalized,
			contentType,
			size: newSize,
			isNew: !existing,
		});

		logger.debug(`Wrote file "${normalized}" (${newSize} bytes)`);
	};

	const appendFile = (path: string, content: string): void => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsFile(normalized, node);

		if (node.contentType !== 'text') {
			throw createVFSError(`Cannot append to binary file: ${normalized}`, {
				code: 'VFS_INVALID_OPERATION',
				metadata: { path: normalized },
			});
		}

		recordHistory(normalized, node);

		const currentText = node.text ?? '';
		const newText = currentText + content;
		const newSize = encoder.encode(newText).byteLength;

		assertFileSize(newSize, normalized);
		const sizeDelta = newSize - node.size;
		if (sizeDelta > 0) assertTotalSize(sizeDelta);

		const ts = Date.now();
		nodes.set(normalized, {
			...node,
			text: newText,
			size: newSize,
			modifiedAt: ts,
		});
		currentTotalSize += sizeDelta;

		onFileWrite?.({
			path: normalized,
			contentType: 'text',
			size: newSize,
			isNew: false,
		});
	};

	const deleteFile = (path: string): boolean => {
		const normalized = assertValidPath(path);
		const node = nodes.get(normalized);
		if (!node) return false;
		assertIsFile(normalized, node);

		currentTotalSize -= node.size;
		currentFileCount--;
		nodes.delete(normalized);
		fileHistory.delete(normalized);
		logger.debug(`Deleted file "${normalized}"`);
		return true;
	};

	// -- Directory operations ---------------------------------------------

	const mkdir = (path: string, mkdirOptions?: VFSMkdirOptions): void => {
		const normalized = assertValidPath(path);
		if (normalized === VFS_ROOT) return;

		const existing = nodes.get(normalized);
		if (existing) {
			if (existing.type === 'directory') return;
			throw createVFSError(
				`Path exists and is not a directory: ${normalized}`,
				{ code: 'VFS_NOT_DIRECTORY', metadata: { path: normalized } },
			);
		}

		if (mkdirOptions?.recursive) {
			createParents(normalized);
		} else {
			ensureParentExists(normalized);
		}

		assertNodeLimit();
		const ts = Date.now();
		nodes.set(normalized, {
			type: 'directory',
			contentType: 'text',
			text: undefined,
			data: undefined,
			size: 0,
			createdAt: ts,
			modifiedAt: ts,
		});
		currentDirCount++;

		logger.debug(`Created directory "${normalized}"`);
	};

	const readdir = (
		path: string,
		readdirOptions?: VFSReaddirOptions,
	): readonly VFSDirEntry[] => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsDirectory(normalized, node);

		if (readdirOptions?.recursive) {
			const descendants = getDescendants(normalized);
			return Object.freeze(
				descendants.map((p) => {
					const n = getNode(p);
					const prefix = normalized === VFS_ROOT ? VFS_ROOT : `${normalized}/`;
					const relativeName = p.slice(prefix.length);
					return Object.freeze({ name: relativeName, type: n.type });
				}),
			);
		}

		const children = getDirectChildren(normalized);
		return Object.freeze(
			children.map((childPath) => {
				const n = getNode(childPath);
				return Object.freeze({ name: baseName(childPath), type: n.type });
			}),
		);
	};

	const rmdir = (path: string, deleteOptions?: VFSDeleteOptions): boolean => {
		const normalized = assertValidPath(path);
		if (normalized === VFS_ROOT) {
			throw createVFSError('Cannot delete root directory', {
				code: 'VFS_INVALID_OPERATION',
			});
		}

		const node = nodes.get(normalized);
		if (!node) return false;
		assertIsDirectory(normalized, node);

		const children = getDirectChildren(normalized);

		if (children.length > 0 && !deleteOptions?.recursive) {
			throw createVFSError(`Directory is not empty: ${normalized}`, {
				code: 'VFS_NOT_EMPTY',
				metadata: { path: normalized, childCount: children.length },
			});
		}

		if (deleteOptions?.recursive) {
			const descendants = getDescendants(normalized);
			for (const desc of descendants) {
				const descNode = nodes.get(desc);
				if (descNode?.type === 'file') {
					currentTotalSize -= descNode.size;
					currentFileCount--;
					fileHistory.delete(desc);
				} else if (descNode?.type === 'directory') {
					currentDirCount--;
				}
				nodes.delete(desc);
			}
		}

		currentDirCount--;
		nodes.delete(normalized);
		logger.debug(`Deleted directory "${normalized}"`);
		return true;
	};

	// -- General operations -----------------------------------------------

	const stat = (path: string): VFSStat => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);

		return Object.freeze({
			path: normalized,
			type: node.type,
			size: node.size,
			createdAt: node.createdAt,
			modifiedAt: node.modifiedAt,
		});
	};

	const exists = (path: string): boolean => {
		const normalized = resolve(path);
		return nodes.has(normalized);
	};

	const rename = (oldPath: string, newPath: string): void => {
		const normalizedOld = assertValidPath(oldPath);
		const normalizedNew = assertValidPath(newPath);

		if (normalizedOld === VFS_ROOT) {
			throw createVFSError('Cannot rename root directory', {
				code: 'VFS_INVALID_OPERATION',
			});
		}

		if (normalizedOld === normalizedNew) return;

		const node = assertNodeExists(normalizedOld);

		if (
			node.type === 'directory' &&
			normalizedNew.startsWith(`${normalizedOld}/`)
		) {
			throw createVFSError(
				`Cannot move directory into its own descendant: ${normalizedOld} -> ${normalizedNew}`,
				{
					code: 'VFS_INVALID_OPERATION',
					metadata: { oldPath: normalizedOld, newPath: normalizedNew },
				},
			);
		}

		if (nodes.has(normalizedNew)) {
			throw createVFSError(`Destination already exists: ${normalizedNew}`, {
				code: 'VFS_ALREADY_EXISTS',
				metadata: { path: normalizedNew },
			});
		}

		ensureParentExists(normalizedNew);

		if (node.type === 'directory') {
			const descendants = getDescendants(normalizedOld);
			for (const desc of descendants) {
				const descNode = getNode(desc);
				const newDescPath = normalizedNew + desc.slice(normalizedOld.length);
				nodes.set(newDescPath, descNode);
				nodes.delete(desc);

				// Transfer history
				const hist = fileHistory.get(desc);
				if (hist) {
					fileHistory.set(newDescPath, hist);
					fileHistory.delete(desc);
				}
			}
		}

		const ts = Date.now();
		nodes.set(normalizedNew, { ...node, modifiedAt: ts });
		nodes.delete(normalizedOld);

		// Transfer history for the node itself
		const hist = fileHistory.get(normalizedOld);
		if (hist) {
			fileHistory.set(normalizedNew, hist);
			fileHistory.delete(normalizedOld);
		}

		logger.debug(`Renamed "${normalizedOld}" -> "${normalizedNew}"`);
	};

	const copy = (
		src: string,
		dest: string,
		copyOptions?: VFSCopyOptions,
	): void => {
		const normalizedSrc = assertValidPath(src);
		const normalizedDest = assertValidPath(dest);

		const srcNode = assertNodeExists(normalizedSrc);

		const destExists = nodes.has(normalizedDest);
		if (destExists && !copyOptions?.overwrite) {
			throw createVFSError(`Destination already exists: ${normalizedDest}`, {
				code: 'VFS_ALREADY_EXISTS',
				metadata: { path: normalizedDest },
			});
		}

		ensureParentExists(normalizedDest);

		if (srcNode.type === 'file') {
			const content =
				srcNode.contentType === 'text'
					? (srcNode.text ?? '')
					: new Uint8Array(srcNode.data ?? new Uint8Array(0));
			writeFile(normalizedDest, content, {
				contentType: srcNode.contentType,
			});
		} else {
			if (!copyOptions?.recursive) {
				throw createVFSError(
					`Cannot copy directory without recursive option: ${normalizedSrc}`,
					{
						code: 'VFS_INVALID_OPERATION',
						metadata: { path: normalizedSrc },
					},
				);
			}

			// Clean dest if overwriting
			if (destExists && copyOptions?.overwrite) {
				const destNode = nodes.get(normalizedDest);
				if (destNode?.type === 'directory') {
					rmdir(normalizedDest, { recursive: true });
				} else if (destNode) {
					deleteFile(normalizedDest);
				}
			}

			mkdir(normalizedDest);
			const descendants = getDescendants(normalizedSrc);
			for (const desc of descendants) {
				const descNode = getNode(desc);
				const newDescPath = normalizedDest + desc.slice(normalizedSrc.length);

				if (descNode.type === 'directory') {
					mkdir(newDescPath);
				} else {
					const content =
						descNode.contentType === 'text'
							? (descNode.text ?? '')
							: new Uint8Array(descNode.data ?? new Uint8Array(0));
					writeFile(newDescPath, content, {
						contentType: descNode.contentType,
					});
				}
			}
		}

		logger.debug(`Copied "${normalizedSrc}" -> "${normalizedDest}"`);
	};

	// -- Query operations -------------------------------------------------

	const globFn = (pattern: string | readonly string[]): readonly string[] => {
		const patterns = typeof pattern === 'string' ? [pattern] : pattern;
		const positivePatterns: string[] = [];
		const negativePatterns: string[] = [];

		for (const p of patterns) {
			if (p.startsWith('!')) {
				negativePatterns.push(p.slice(1));
			} else {
				positivePatterns.push(p);
			}
		}

		// If no positive patterns, match all files (negation-only filters everything)
		const matchAll = positivePatterns.length === 0;

		const results: string[] = [];
		for (const [path, node] of nodes) {
			if (path === VFS_ROOT) continue;
			if (node.type !== 'file') continue;

			// Check positive patterns (must match at least one)
			const included =
				matchAll || positivePatterns.some((p) => matchGlob(path, p));
			if (!included) continue;

			// Check negative patterns (must not match any)
			const excluded = negativePatterns.some((p) => matchGlob(path, p));
			if (excluded) continue;

			results.push(path);
		}
		results.sort();
		return Object.freeze(results);
	};

	const du = (path: string): number => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);

		if (node.type === 'file') return node.size;

		let total = 0;
		const descendants = getDescendants(normalized);
		for (const desc of descendants) {
			const descNode = nodes.get(desc);
			if (descNode?.type === 'file') {
				total += descNode.size;
			}
		}
		return total;
	};

	const tree = (path?: string): string => {
		const normalized = path ? assertValidPath(path) : VFS_ROOT;
		const node = assertNodeExists(normalized);
		assertIsDirectory(normalized, node);

		const lines: string[] = [];
		const rootName = normalized === VFS_ROOT ? VFS_ROOT : baseName(normalized);
		lines.push(rootName);

		const buildTree = (dirPath: string, prefix: string): void => {
			const children = getDirectChildren(dirPath).sort();
			for (let i = 0; i < children.length; i++) {
				const childPath = children[i];
				const childNode = getNode(childPath);
				const isLast = i === children.length - 1;
				const connector = isLast
					? '\u2514\u2500\u2500 '
					: '\u251C\u2500\u2500 ';
				const childPrefix = isLast ? '    ' : '\u2502   ';
				const name = baseName(childPath);

				if (childNode.type === 'directory') {
					lines.push(`${prefix}${connector}${name}/`);
					buildTree(childPath, `${prefix}${childPrefix}`);
				} else {
					lines.push(`${prefix}${connector}${name} (${childNode.size} bytes)`);
				}
			}
		};

		buildTree(normalized, '');
		return lines.join('\n');
	};

	const search = (
		query: string,
		searchOptions?: VFSSearchOptions,
	): readonly VFSSearchResult[] | number => {
		const maxResults = searchOptions?.maxResults ?? 100;
		const globPattern = searchOptions?.glob;
		const mode = searchOptions?.mode ?? 'substring';
		const ctxBefore = searchOptions?.contextBefore ?? 0;
		const ctxAfter = searchOptions?.contextAfter ?? 0;
		const countOnly = searchOptions?.countOnly ?? false;

		const regex = mode === 'regex' ? new RegExp(query, 'g') : null;
		let count = 0;
		const results: VFSSearchResult[] = [];

		for (const [path, node] of nodes) {
			if (!countOnly && results.length >= maxResults) break;
			if (node.type !== 'file' || node.contentType !== 'text') continue;
			if (globPattern && !matchGlob(path, globPattern)) continue;

			const text = node.text ?? '';
			const lines = text.split('\n');
			for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
				if (!countOnly && results.length >= maxResults) break;
				const line = lines[lineIdx];

				let col: number;
				if (regex) {
					regex.lastIndex = 0;
					const m = regex.exec(line);
					if (!m) continue;
					col = m.index;
				} else {
					col = line.indexOf(query);
					if (col < 0) continue;
				}

				if (countOnly) {
					count++;
					continue;
				}

				const beforeLines: string[] = [];
				const afterLines: string[] = [];
				if (ctxBefore > 0) {
					const start = Math.max(0, lineIdx - ctxBefore);
					for (let i = start; i < lineIdx; i++) {
						beforeLines.push(lines[i]);
					}
				}
				if (ctxAfter > 0) {
					const end = Math.min(lines.length - 1, lineIdx + ctxAfter);
					for (let i = lineIdx + 1; i <= end; i++) {
						afterLines.push(lines[i]);
					}
				}

				const result: VFSSearchResult = {
					path,
					line: lineIdx + 1,
					column: col + 1,
					match: line,
					...(beforeLines.length > 0
						? { contextBefore: Object.freeze(beforeLines) }
						: {}),
					...(afterLines.length > 0
						? { contextAfter: Object.freeze(afterLines) }
						: {}),
				};
				results.push(Object.freeze(result));
			}
		}

		if (countOnly) return count;
		return Object.freeze(results);
	};

	// -- History & Diff ---------------------------------------------------

	const getHistory = (path: string): readonly VFSHistoryEntry[] => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsFile(normalized, node);

		const entries = fileHistory.get(normalized);
		if (!entries || entries.length === 0) {
			return Object.freeze([]);
		}

		return Object.freeze([...entries]);
	};

	// Myers diff — computes shortest edit script between two line arrays
	const computeLCS = (
		oldLines: string[],
		newLines: string[],
	): VFSDiffLine[] => {
		const n = oldLines.length;
		const m = newLines.length;
		const max = n + m;

		if (max === 0) return [];

		const totalLines = n + m;
		if (totalLines > limits.maxDiffLines) {
			throw createVFSError(
				`Diff input too large: ${n} + ${m} = ${totalLines} lines exceeds limit (${limits.maxDiffLines})`,
				{
					code: 'VFS_LIMIT_EXCEEDED',
					metadata: { limit: 'maxDiffLines', lines: totalLines },
				},
			);
		}

		// Optimization: handle trivial cases
		if (n === 0) {
			return newLines.map((text, i) =>
				Object.freeze({
					type: 'add' as const,
					text,
					newLine: i + 1,
				}),
			);
		}
		if (m === 0) {
			return oldLines.map((text, i) =>
				Object.freeze({
					type: 'remove' as const,
					text,
					oldLine: i + 1,
				}),
			);
		}

		// Myers algorithm
		const vSize = 2 * max + 1;
		const v = new Int32Array(vSize);
		v.fill(-1);
		const offset = max;
		v[offset + 1] = 0;

		const trace: Int32Array[] = [];

		let found = false;
		for (let d = 0; d <= max && !found; d++) {
			trace.push(new Int32Array(v));
			for (let k = -d; k <= d; k += 2) {
				let x: number;
				if (k === -d || (k !== d && v[offset + k - 1] < v[offset + k + 1])) {
					x = v[offset + k + 1];
				} else {
					x = v[offset + k - 1] + 1;
				}
				let y = x - k;
				while (x < n && y < m && oldLines[x] === newLines[y]) {
					x++;
					y++;
				}
				v[offset + k] = x;
				if (x >= n && y >= m) {
					found = true;
					break;
				}
			}
		}

		// Backtrack to build the edit script
		const result: VFSDiffLine[] = [];
		let x = n;
		let y = m;

		for (let d = trace.length - 1; d >= 0; d--) {
			const vPrev = trace[d];
			const k = x - y;

			let prevK: number;
			if (
				k === -d ||
				(k !== d && vPrev[offset + k - 1] < vPrev[offset + k + 1])
			) {
				prevK = k + 1;
			} else {
				prevK = k - 1;
			}

			const prevX = vPrev[offset + prevK];
			const prevY = prevX - prevK;

			// Diagonal (equal lines)
			while (x > prevX && y > prevY) {
				x--;
				y--;
				result.push(
					Object.freeze({
						type: 'equal' as const,
						text: oldLines[x],
						oldLine: x + 1,
						newLine: y + 1,
					}),
				);
			}

			if (d > 0) {
				if (x === prevX) {
					// Insert
					y--;
					result.push(
						Object.freeze({
							type: 'add' as const,
							text: newLines[y],
							newLine: y + 1,
						}),
					);
				} else {
					// Delete
					x--;
					result.push(
						Object.freeze({
							type: 'remove' as const,
							text: oldLines[x],
							oldLine: x + 1,
						}),
					);
				}
			}
		}

		result.reverse();
		return result;
	};

	const buildHunks = (
		lines: VFSDiffLine[],
		contextLines: number,
	): VFSDiffHunk[] => {
		if (lines.length === 0) return [];

		// Collect change indices
		const changeIndices: number[] = [];
		for (let i = 0; i < lines.length; i++) {
			if (lines[i].type !== 'equal') {
				changeIndices.push(i);
			}
		}

		if (changeIndices.length === 0) return [];

		// Group changes into hunks
		const hunks: VFSDiffHunk[] = [];
		let hunkStart = 0;

		for (let ci = 0; ci < changeIndices.length; ci++) {
			if (ci === 0) {
				hunkStart = changeIndices[ci];
				continue;
			}

			const prev = changeIndices[ci - 1];
			const curr = changeIndices[ci];

			// If gap between changes exceeds 2*context, start a new hunk
			if (curr - prev > contextLines * 2 + 1) {
				const hunkEnd = prev;
				hunks.push(buildSingleHunk(lines, hunkStart, hunkEnd, contextLines));
				hunkStart = curr;
			}
		}

		// Final hunk
		hunks.push(
			buildSingleHunk(
				lines,
				hunkStart,
				changeIndices[changeIndices.length - 1],
				contextLines,
			),
		);

		return hunks;
	};

	const buildSingleHunk = (
		lines: VFSDiffLine[],
		firstChange: number,
		lastChange: number,
		contextLines: number,
	): VFSDiffHunk => {
		const start = Math.max(0, firstChange - contextLines);
		const end = Math.min(lines.length - 1, lastChange + contextLines);

		const hunkLines: VFSDiffLine[] = [];
		let oldStart = 0;
		let newStart = 0;
		let oldCount = 0;
		let newCount = 0;
		let foundFirst = false;

		for (let i = start; i <= end; i++) {
			const line = lines[i];
			hunkLines.push(line);

			if (!foundFirst) {
				oldStart = line.oldLine ?? (line.type === 'add' ? 0 : 0);
				newStart = line.newLine ?? (line.type === 'remove' ? 0 : 0);
				if (line.type === 'equal') {
					oldStart = line.oldLine ?? 1;
					newStart = line.newLine ?? 1;
				} else if (line.type === 'remove') {
					oldStart = line.oldLine ?? 1;
					newStart = oldStart;
				} else {
					newStart = line.newLine ?? 1;
					oldStart = newStart;
				}
				foundFirst = true;
			}

			if (line.type === 'equal') {
				oldCount++;
				newCount++;
			} else if (line.type === 'remove') {
				oldCount++;
			} else {
				newCount++;
			}
		}

		return Object.freeze({
			oldStart,
			oldCount,
			newStart,
			newCount,
			lines: Object.freeze(hunkLines),
		});
	};

	const getTextLines = (node: InternalNode): string[] => {
		if (node.contentType !== 'text') {
			return ['[binary content]'];
		}
		return (node.text ?? '').split('\n');
	};

	const getHistoryEntryLines = (entry: VFSHistoryEntry): string[] => {
		if (entry.contentType !== 'text') {
			return ['[binary content]'];
		}
		return (entry.text ?? '').split('\n');
	};

	const diffFiles = (
		oldPath: string,
		newPath: string,
		diffOptions?: VFSDiffOptions,
	): VFSDiffResult => {
		const normalizedOld = assertValidPath(oldPath);
		const normalizedNew = assertValidPath(newPath);
		const oldNode = assertNodeExists(normalizedOld);
		const newNode = assertNodeExists(normalizedNew);
		assertIsFile(normalizedOld, oldNode);
		assertIsFile(normalizedNew, newNode);

		const contextLines = diffOptions?.context ?? 3;
		const oldLines = getTextLines(oldNode);
		const newLines = getTextLines(newNode);

		const allLines = computeLCS(oldLines, newLines);
		const hunks = buildHunks(allLines, contextLines);

		let additions = 0;
		let deletions = 0;
		for (const line of allLines) {
			if (line.type === 'add') additions++;
			if (line.type === 'remove') deletions++;
		}

		return Object.freeze({
			oldPath: normalizedOld,
			newPath: normalizedNew,
			hunks: Object.freeze(hunks),
			additions,
			deletions,
		});
	};

	const diffVersions = (
		path: string,
		oldVersion: number,
		newVersion?: number,
		diffOptions?: VFSDiffOptions,
	): VFSDiffResult => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsFile(normalized, node);

		const entries = fileHistory.get(normalized);
		const contextLines = diffOptions?.context ?? 3;

		const getVersionLines = (version: number): string[] => {
			// Check if version is current
			const histLen = entries?.length ?? 0;
			const currentVersion = histLen + 1;

			if (version === currentVersion) {
				return getTextLines(node);
			}

			if (!entries || version < 1 || version > histLen) {
				throw createVFSError(
					`Version ${version} not found for ${normalized} (available: 1-${currentVersion})`,
					{
						code: 'VFS_NOT_FOUND',
						metadata: { path: normalized, version },
					},
				);
			}

			return getHistoryEntryLines(entries[version - 1]);
		};

		const actualNewVersion = newVersion ?? (entries?.length ?? 0) + 1;

		const oldLines = getVersionLines(oldVersion);
		const newLines = getVersionLines(actualNewVersion);

		const allLines = computeLCS(oldLines, newLines);
		const hunks = buildHunks(allLines, contextLines);

		let additions = 0;
		let deletions = 0;
		for (const line of allLines) {
			if (line.type === 'add') additions++;
			if (line.type === 'remove') deletions++;
		}

		return Object.freeze({
			oldPath: `${normalized}@v${oldVersion}`,
			newPath: `${normalized}@v${actualNewVersion}`,
			hunks: Object.freeze(hunks),
			additions,
			deletions,
		});
	};

	const checkout = (path: string, version: number): void => {
		const normalized = assertValidPath(path);
		const node = assertNodeExists(normalized);
		assertIsFile(normalized, node);

		const entries = fileHistory.get(normalized);
		const currentVersion = (entries?.length ?? 0) + 1;

		if (version === currentVersion) return; // Already at this version

		if (!entries || version < 1 || version > entries.length) {
			throw createVFSError(
				`Version ${version} not found for ${normalized} (available: 1-${currentVersion})`,
				{
					code: 'VFS_NOT_FOUND',
					metadata: { path: normalized, version },
				},
			);
		}

		const entry = entries[version - 1];

		// Record current state as history before reverting
		recordHistory(normalized, node);

		const ts = Date.now();
		if (entry.contentType === 'binary' && entry.base64) {
			const binary = new Uint8Array(Buffer.from(entry.base64, 'base64'));
			const sizeDelta = binary.byteLength - node.size;
			nodes.set(normalized, {
				type: 'file',
				contentType: 'binary',
				text: undefined,
				data: binary,
				size: binary.byteLength,
				createdAt: node.createdAt,
				modifiedAt: ts,
			});
			currentTotalSize += sizeDelta;
		} else {
			const text = entry.text ?? '';
			const newSize = encoder.encode(text).byteLength;
			const sizeDelta = newSize - node.size;
			nodes.set(normalized, {
				type: 'file',
				contentType: 'text',
				text,
				data: undefined,
				size: newSize,
				createdAt: node.createdAt,
				modifiedAt: ts,
			});
			currentTotalSize += sizeDelta;
		}

		logger.debug(`Checked out "${normalized}" at version ${version}`);
	};

	// -- Introspection ----------------------------------------------------

	const snapshot = (): VFSSnapshot => {
		const files: VFSSnapshot['files'][number][] = [];
		const directories: VFSSnapshot['directories'][number][] = [];

		for (const [path, node] of nodes) {
			if (path === VFS_ROOT && node.type === 'directory') continue;
			if (node.type === 'file') {
				files.push(
					Object.freeze({
						path,
						contentType: node.contentType,
						text: node.text,
						base64: node.data
							? Buffer.from(node.data).toString('base64')
							: undefined,
						createdAt: node.createdAt,
						modifiedAt: node.modifiedAt,
					}),
				);
			} else {
				directories.push(
					Object.freeze({
						path,
						createdAt: node.createdAt,
						modifiedAt: node.modifiedAt,
					}),
				);
			}
		}

		return Object.freeze({
			files: Object.freeze(files),
			directories: Object.freeze(directories),
		});
	};

	const restore = (snap: VFSSnapshot): void => {
		// Validate all entries against limits before committing
		let totalSize = 0;
		let totalNodes = 1; // root

		const sortedDirs = [...snap.directories].sort(
			(a, b) => pathDepth(a.path) - pathDepth(b.path),
		);

		for (const dir of sortedDirs) {
			const normalized = normalizePath(dir.path);
			const pathError = validatePath(normalized, limits);
			if (pathError) {
				throw createVFSError(`Snapshot restore failed: ${pathError}`, {
					code: 'VFS_INVALID_PATH',
					metadata: { path: dir.path },
				});
			}
			totalNodes++;
		}

		for (const file of snap.files) {
			const normalized = normalizePath(file.path);
			const pathError = validatePath(normalized, limits);
			if (pathError) {
				throw createVFSError(`Snapshot restore failed: ${pathError}`, {
					code: 'VFS_INVALID_PATH',
					metadata: { path: file.path },
				});
			}

			let fileSize: number;
			if (file.contentType === 'binary' && file.base64) {
				fileSize = Math.floor(file.base64.length * 0.75);
			} else {
				fileSize = encoder.encode(file.text ?? '').byteLength;
			}

			if (fileSize > limits.maxFileSize) {
				throw createVFSError(
					`Snapshot restore failed: file size ${fileSize} exceeds limit (${limits.maxFileSize}): ${file.path}`,
					{
						code: 'VFS_LIMIT_EXCEEDED',
						metadata: { limit: 'maxFileSize', path: file.path },
					},
				);
			}
			totalSize += fileSize;
			totalNodes++;
		}

		// Validate parent directories exist for every file
		const dirPaths = new Set(sortedDirs.map((d) => normalizePath(d.path)));
		for (const file of snap.files) {
			const normalized = normalizePath(file.path);
			const ancestors = ancestorPaths(normalized);
			for (const ancestor of ancestors) {
				if (ancestor === VFS_ROOT) continue;
				if (!dirPaths.has(ancestor)) {
					throw createVFSError(
						`Snapshot restore failed: missing parent directory "${ancestor}" for file "${file.path}"`,
						{
							code: 'VFS_INVALID_OPERATION',
							metadata: { path: file.path, missingDir: ancestor },
						},
					);
				}
			}
		}

		if (totalNodes > limits.maxNodeCount) {
			throw createVFSError(
				`Snapshot restore failed: node count ${totalNodes} exceeds limit (${limits.maxNodeCount})`,
				{ code: 'VFS_LIMIT_EXCEEDED', metadata: { limit: 'maxNodeCount' } },
			);
		}

		if (totalSize > limits.maxTotalSize) {
			throw createVFSError(
				`Snapshot restore failed: total size ${totalSize} exceeds limit (${limits.maxTotalSize})`,
				{ code: 'VFS_LIMIT_EXCEEDED', metadata: { limit: 'maxTotalSize' } },
			);
		}

		// Validation passed — commit
		doClear();

		for (const dir of sortedDirs) {
			const normalized = normalizePath(dir.path);
			nodes.set(normalized, {
				type: 'directory',
				contentType: 'text',
				text: undefined,
				data: undefined,
				size: 0,
				createdAt: dir.createdAt,
				modifiedAt: dir.modifiedAt,
			});
			currentDirCount++;
		}

		for (const file of snap.files) {
			const normalized = normalizePath(file.path);
			if (file.contentType === 'binary' && file.base64) {
				const binary = new Uint8Array(Buffer.from(file.base64, 'base64'));
				nodes.set(normalized, {
					type: 'file',
					contentType: 'binary',
					text: undefined,
					data: binary,
					size: binary.byteLength,
					createdAt: file.createdAt,
					modifiedAt: file.modifiedAt,
				});
				currentTotalSize += binary.byteLength;
			} else {
				const text = file.text ?? '';
				const size = encoder.encode(text).byteLength;
				nodes.set(normalized, {
					type: 'file',
					contentType: 'text',
					text,
					data: undefined,
					size,
					createdAt: file.createdAt,
					modifiedAt: file.modifiedAt,
				});
				currentTotalSize += size;
			}
			currentFileCount++;
		}

		logger.debug(
			`Restored VFS snapshot (${snap.files.length} files, ${snap.directories.length} directories)`,
		);
	};

	const doClear = (): void => {
		nodes.clear();
		fileHistory.clear();
		currentTotalSize = 0;
		currentFileCount = 0;
		currentDirCount = 0;
		initRoot();
		logger.debug('Cleared VFS');
	};

	// -- Return frozen interface ------------------------------------------

	return Object.freeze({
		readFile,
		writeFile,
		appendFile,
		deleteFile,
		mkdir,
		readdir,
		rmdir,
		stat,
		exists,
		rename,
		copy,
		glob: globFn,
		tree,
		du,
		search,
		history: getHistory,
		diff: diffFiles,
		diffVersions,
		checkout,
		snapshot,
		restore,
		clear: doClear,
		get totalSize() {
			return currentTotalSize;
		},
		get nodeCount() {
			return nodes.size;
		},
		get fileCount() {
			return currentFileCount;
		},
		get directoryCount() {
			return currentDirCount;
		},
	});
}
