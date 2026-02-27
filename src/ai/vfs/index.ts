// ---------------------------------------------------------------------------
// VFS module — barrel re-export
// ---------------------------------------------------------------------------

export {
	VFS_ROOT,
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	validatePath,
	validateSegment,
} from './path-utils.js';
export type {
	VFSCommitOperation,
	VFSCommitOptions,
	VFSCommitResult,
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
	VFSLoadOptions,
	VFSMkdirOptions,
	VFSNodeType,
	VFSReaddirOptions,
	VFSReadResult,
	VFSSearchOptions,
	VFSSearchResult,
	VFSSnapshot,
	VFSSnapshotDirectory,
	VFSSnapshotFile,
	VFSStat,
	VFSWriteEvent,
	VFSWriteOptions,
} from './types.js';
export type {
	VFSValidationIssue,
	VFSValidationResult,
	VFSValidator,
} from './validators.js';
export {
	createDefaultValidators,
	createEmptyFileValidator,
	createJSONSyntaxValidator,
	createMissingTrailingNewlineValidator,
	createMixedIndentationValidator,
	createMixedLineEndingsValidator,
	createTrailingWhitespaceValidator,
	validateSnapshot,
} from './validators.js';
export type { VirtualFS, VirtualFSOptions } from './vfs.js';
export { createVirtualFS } from './vfs.js';
export type { VFSDisk, VFSDiskOptions } from './vfs-disk.js';
export { createVFSDisk } from './vfs-disk.js';
