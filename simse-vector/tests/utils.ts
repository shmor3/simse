// ---------------------------------------------------------------------------
// Shared mock factories for simse-vector tests
// ---------------------------------------------------------------------------

import type { Logger } from '../src/logger.js';
import { createNoopLogger } from '../src/logger.js';
import type { StorageBackend } from '../src/storage.js';

// ---------------------------------------------------------------------------
// In-memory StorageBackend
// ---------------------------------------------------------------------------

export function createMemoryStorage(
	sharedData?: Map<string, Buffer>,
): StorageBackend {
	const data: Map<string, Buffer> = sharedData ?? new Map();

	return Object.freeze({
		load: async () => new Map(data),
		save: async (newData: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of newData) {
				data.set(k, v);
			}
		},
		close: async () => {},
	});
}

// ---------------------------------------------------------------------------
// Silent Logger
// ---------------------------------------------------------------------------

export function createSilentLogger(): Logger {
	return createNoopLogger();
}
