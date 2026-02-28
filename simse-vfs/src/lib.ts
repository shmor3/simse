// ---------------------------------------------------------------------------
// simse-vfs — public API surface
// ---------------------------------------------------------------------------

export type { VFSError, VFSErrorOptions } from './errors.js';
// ---- Errors ----------------------------------------------------------------
export { createVFSError, isVFSError, toError } from './errors.js';
// ---- Exec (command passthrough) --------------------------------------------
export type {
	ExecBackend,
	ExecOptions,
	ExecResult,
	VFSExecutor,
} from './exec.js';
export { createVFSExecutor } from './exec.js';
// ---- Logger / EventBus ----------------------------------------------------
export type { EventBus, Logger } from './logger.js';
export { createNoopLogger } from './logger.js';
// ---- Path utilities --------------------------------------------------------
export {
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	VFS_ROOT,
	VFS_SCHEME,
	validatePath,
	validateSegment,
} from './path-utils.js';
// ---- Types -----------------------------------------------------------------
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
// ---- Validators ------------------------------------------------------------
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
// ---- VFS (in-memory) -------------------------------------------------------
export type { VirtualFS, VirtualFSOptions } from './vfs.js';
export { createVirtualFS } from './vfs.js';
// ---- VFS Disk (commit / load) ----------------------------------------------
export type { VFSDisk, VFSDiskOptions } from './vfs-disk.js';
export { createVFSDisk } from './vfs-disk.js';
