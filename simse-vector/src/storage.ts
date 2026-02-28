// ---------------------------------------------------------------------------
// StorageBackend — pluggable persistence for the vector store
// ---------------------------------------------------------------------------
//
// Defines a minimal interface that any storage backend must implement.
// Consumers provide their own implementation (file-based, SQLite, S3, etc.).
// ---------------------------------------------------------------------------

import type { Buffer } from 'node:buffer';

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface StorageBackend {
	/** Load all key-value pairs from the backend. */
	readonly load: () => Promise<Map<string, Buffer>>;
	/** Persist all key-value pairs to the backend (full snapshot). */
	readonly save: (data: Map<string, Buffer>) => Promise<void>;
	/** Release any resources held by the backend. */
	readonly close: () => Promise<void>;
}
