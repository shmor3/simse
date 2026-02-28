// ---------------------------------------------------------------------------
// Virtual Filesystem Types
// ---------------------------------------------------------------------------

export type VFSNodeType = 'file' | 'directory';

export type VFSContentType = 'text' | 'binary';

export interface VFSStat {
	readonly path: string;
	readonly type: VFSNodeType;
	readonly size: number;
	readonly createdAt: number;
	readonly modifiedAt: number;
}

export interface VFSDirEntry {
	readonly name: string;
	readonly type: VFSNodeType;
}

export type VFSReadResult =
	| {
			readonly contentType: 'text';
			readonly text: string;
			readonly data: undefined;
			readonly size: number;
	  }
	| {
			readonly contentType: 'binary';
			readonly text: undefined;
			readonly data: Uint8Array;
			readonly size: number;
	  };

// ---------------------------------------------------------------------------
// Operation Options
// ---------------------------------------------------------------------------

export interface VFSWriteOptions {
	readonly createParents?: boolean;
	readonly contentType?: VFSContentType;
}

export interface VFSMkdirOptions {
	readonly recursive?: boolean;
}

export interface VFSDeleteOptions {
	readonly recursive?: boolean;
}

export interface VFSReaddirOptions {
	readonly recursive?: boolean;
}

export interface VFSCopyOptions {
	readonly overwrite?: boolean;
	readonly recursive?: boolean;
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

export interface VFSSearchOptions {
	readonly glob?: string;
	readonly maxResults?: number;
	readonly mode?: 'substring' | 'regex';
	readonly contextBefore?: number;
	readonly contextAfter?: number;
	readonly countOnly?: boolean;
}

export interface VFSSearchResult {
	readonly path: string;
	readonly line: number;
	readonly column: number;
	readonly match: string;
	readonly contextBefore?: readonly string[];
	readonly contextAfter?: readonly string[];
}

// ---------------------------------------------------------------------------
// History & Diff
// ---------------------------------------------------------------------------

export interface VFSHistoryOptions {
	readonly maxEntriesPerFile?: number;
}

export interface VFSHistoryEntry {
	readonly version: number;
	readonly contentType: VFSContentType;
	readonly text?: string;
	readonly base64?: string;
	readonly size: number;
	readonly timestamp: number;
}

export interface VFSDiffLine {
	readonly type: 'add' | 'remove' | 'equal';
	readonly text: string;
	readonly oldLine?: number;
	readonly newLine?: number;
}

export interface VFSDiffResult {
	readonly oldPath: string;
	readonly newPath: string;
	readonly hunks: readonly VFSDiffHunk[];
	readonly additions: number;
	readonly deletions: number;
}

export interface VFSDiffHunk {
	readonly oldStart: number;
	readonly oldCount: number;
	readonly newStart: number;
	readonly newCount: number;
	readonly lines: readonly VFSDiffLine[];
}

export interface VFSDiffOptions {
	readonly context?: number;
}

// ---------------------------------------------------------------------------
// Disk Commit / Load
// ---------------------------------------------------------------------------

export interface VFSCommitOptions {
	readonly overwrite?: boolean;
	readonly dryRun?: boolean;
	readonly filter?: (path: string) => boolean;
	readonly validate?:
		| boolean
		| readonly import('./validators.js').VFSValidator[];
}

export interface VFSLoadOptions {
	readonly overwrite?: boolean;
	readonly filter?: (path: string) => boolean;
	readonly maxFileSize?: number;
}

export interface VFSCommitResult {
	readonly filesWritten: number;
	readonly directoriesCreated: number;
	readonly bytesWritten: number;
	readonly operations: readonly VFSCommitOperation[];
	readonly validation?: import('./validators.js').VFSValidationResult;
}

export interface VFSCommitOperation {
	readonly type: 'write' | 'mkdir' | 'skip';
	readonly path: string;
	readonly diskPath: string;
	readonly size?: number;
	readonly reason?: string;
}

// ---------------------------------------------------------------------------
// Write Event (notification callback)
// ---------------------------------------------------------------------------

export interface VFSWriteEvent {
	readonly path: string;
	readonly contentType: VFSContentType;
	readonly size: number;
	readonly isNew: boolean;
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

export interface VFSLimits {
	readonly maxFileSize?: number;
	readonly maxTotalSize?: number;
	readonly maxPathDepth?: number;
	readonly maxNameLength?: number;
	readonly maxNodeCount?: number;
	readonly maxPathLength?: number;
	readonly maxDiffLines?: number;
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

export interface VFSSnapshotFile {
	readonly path: string;
	readonly contentType: VFSContentType;
	readonly text?: string;
	readonly base64?: string;
	readonly createdAt: number;
	readonly modifiedAt: number;
}

export interface VFSSnapshotDirectory {
	readonly path: string;
	readonly createdAt: number;
	readonly modifiedAt: number;
}

export interface VFSSnapshot {
	readonly files: readonly VFSSnapshotFile[];
	readonly directories: readonly VFSSnapshotDirectory[];
}
