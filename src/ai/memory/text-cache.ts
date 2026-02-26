// ---------------------------------------------------------------------------
// LRU Text Cache
// ---------------------------------------------------------------------------
//
// Keeps frequently accessed entry texts in memory to avoid repeated disk
// reads during search result hydration. Evicts by entry count and total
// byte budget (whichever limit is hit first).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface TextCacheOptions {
	/** Maximum number of entries in the cache. Defaults to `500`. */
	readonly maxEntries?: number;
	/** Maximum total bytes of cached text. Defaults to `5_242_880` (5 MB). */
	readonly maxBytes?: number;
}

// ---------------------------------------------------------------------------
// TextCache interface
// ---------------------------------------------------------------------------

export interface TextCache {
	/** Get a cached text by entry ID. Returns `undefined` on miss. */
	readonly get: (id: string) => string | undefined;
	/** Put a text into the cache, promoting it to most-recently-used. */
	readonly put: (id: string, text: string) => void;
	/** Remove a specific entry from the cache. */
	readonly remove: (id: string) => boolean;
	/** Clear all cached entries. */
	readonly clear: () => void;
	/** Number of entries currently in the cache. */
	readonly size: number;
	/** Total bytes currently used. */
	readonly bytes: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createTextCache(options?: TextCacheOptions): TextCache {
	const maxEntries = options?.maxEntries ?? 500;
	const maxBytes = options?.maxBytes ?? 5_242_880;

	// Map preserves insertion order — we use it as an LRU list.
	// The most recently accessed entry is moved to the end.
	const cache = new Map<string, string>();
	let totalBytes = 0;

	function byteLength(text: string): number {
		// Fast approximation: JS strings are UTF-16, but for budget
		// purposes we count UTF-8 bytes (matches disk size better).
		let bytes = 0;
		for (let i = 0; i < text.length; i++) {
			const code = text.charCodeAt(i);
			if (code < 0x80) bytes += 1;
			else if (code < 0x800) bytes += 2;
			else bytes += 3;
		}
		return bytes;
	}

	function evict(): void {
		while (cache.size > maxEntries || totalBytes > maxBytes) {
			const first = cache.keys().next();
			if (first.done) break;
			const key = first.value;
			const text = cache.get(key)!;
			totalBytes -= byteLength(text);
			cache.delete(key);
		}
	}

	const textCache: TextCache = {
		get(id: string): string | undefined {
			const text = cache.get(id);
			if (text === undefined) return undefined;
			// Promote to most-recently-used (move to end)
			cache.delete(id);
			cache.set(id, text);
			return text;
		},

		put(id: string, text: string): void {
			// Remove old entry if exists (will be re-added at end)
			const existing = cache.get(id);
			if (existing !== undefined) {
				totalBytes -= byteLength(existing);
				cache.delete(id);
			}
			const bytes = byteLength(text);
			totalBytes += bytes;
			cache.set(id, text);
			evict();
		},

		remove(id: string): boolean {
			const text = cache.get(id);
			if (text === undefined) return false;
			totalBytes -= byteLength(text);
			cache.delete(id);
			return true;
		},

		clear(): void {
			cache.clear();
			totalBytes = 0;
		},

		get size() {
			return cache.size;
		},

		get bytes() {
			return totalBytes;
		},
	};

	return Object.freeze(textCache);
}
