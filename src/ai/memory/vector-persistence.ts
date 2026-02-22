// ---------------------------------------------------------------------------
// Vector store persistence types and validation
// ---------------------------------------------------------------------------

/**
 * On-disk JSON representation of a vector entry. Embeddings are stored as
 * base64-encoded Float32Array strings. Text content is stored separately
 * as individual compressed .md files.
 */
export interface IndexEntry {
	id: string;
	/** Base64-encoded Float32Array bytes. */
	embedding: string;
	metadata: Record<string, string>;
	timestamp: number;
	/** Number of times this entry has been accessed (recommendation engine). */
	accessCount?: number;
	/** Last access timestamp in epoch milliseconds. */
	lastAccessed?: number;
	/** Auto-extracted or user-assigned topic labels. */
	topics?: string[];
}

/**
 * Root structure of the index.json file.
 */
export interface IndexFile {
	version: 2;
	entries: IndexEntry[];
}

/**
 * Type-guard that validates an unknown value conforms to the `IndexEntry` shape.
 */
export function isValidIndexEntry(value: unknown): value is IndexEntry {
	if (typeof value !== 'object' || value === null) return false;

	const obj = value as Record<string, unknown>;

	if (typeof obj.id !== 'string' || obj.id.length === 0) return false;
	if (typeof obj.embedding !== 'string' || obj.embedding.length === 0)
		return false;
	if (
		typeof obj.metadata !== 'object' ||
		obj.metadata === null ||
		!Object.values(obj.metadata as Record<string, unknown>).every(
			(v) => typeof v === 'string',
		)
	)
		return false;
	if (typeof obj.timestamp !== 'number' || !Number.isFinite(obj.timestamp))
		return false;

	// Optional fields
	if (
		obj.accessCount !== undefined &&
		(typeof obj.accessCount !== 'number' || !Number.isFinite(obj.accessCount))
	)
		return false;
	if (
		obj.lastAccessed !== undefined &&
		(typeof obj.lastAccessed !== 'number' || !Number.isFinite(obj.lastAccessed))
	)
		return false;
	if (
		obj.topics !== undefined &&
		(!Array.isArray(obj.topics) ||
			!obj.topics.every((t) => typeof t === 'string'))
	)
		return false;

	return true;
}

/**
 * Type-guard that validates an unknown value conforms to the `IndexFile` shape.
 */
export function isValidIndexFile(parsed: unknown): parsed is IndexFile {
	if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed))
		return false;

	const obj = parsed as Record<string, unknown>;
	return obj.version === 2 && Array.isArray(obj.entries);
}
