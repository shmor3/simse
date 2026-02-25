// ---------------------------------------------------------------------------
// Vector store persistence types and validation
// ---------------------------------------------------------------------------

/**
 * On-disk JSON representation of a vector entry. Embeddings are stored as
 * base64-encoded Float32Array strings. Text content is stored separately
 * as individual compressed .md files.
 */
export interface IndexEntry {
	readonly id: string;
	/** Base64-encoded Float32Array bytes. */
	readonly embedding: string;
	readonly metadata: Readonly<Record<string, string>>;
	readonly timestamp: number;
	/** Number of times this entry has been accessed (recommendation engine). */
	readonly accessCount?: number;
	/** Last access timestamp in epoch milliseconds. */
	readonly lastAccessed?: number;
	/** Auto-extracted or user-assigned topic labels. */
	readonly topics?: readonly string[];
}

/**
 * Root structure of the index.json file.
 */
export interface IndexFile {
	readonly version: 2;
	readonly entries: readonly IndexEntry[];
}

// ---------------------------------------------------------------------------
// Learning state persistence
// ---------------------------------------------------------------------------

/** Per-entry feedback record stored on disk. */
export interface FeedbackEntry {
	readonly id: string;
	readonly queryCount: number;
	readonly totalRetrievals: number;
	readonly lastQueryTimestamp: number;
}

/** Serialised query record for the learning state file. */
export interface SerializedQueryRecord {
	/** Base64-encoded Float32Array embedding. */
	readonly embedding: string;
	readonly timestamp: number;
	readonly resultCount: number;
}

/** Per-entry explicit feedback record stored on disk. */
export interface ExplicitFeedbackEntry {
	readonly entryId: string;
	readonly positiveCount: number;
	readonly negativeCount: number;
}

/** Per-topic learning profile stored on disk. */
export interface TopicProfileEntry {
	readonly topic: string;
	readonly weights: {
		readonly vector: number;
		readonly recency: number;
		readonly frequency: number;
	};
	/** Base64-encoded Float32Array interest embedding for this topic. */
	readonly interestEmbedding?: string;
	readonly queryCount: number;
}

/** Root structure of the learning.json file. */
export interface LearningState {
	readonly version: 1;
	readonly feedback: readonly FeedbackEntry[];
	readonly queryHistory: readonly SerializedQueryRecord[];
	readonly adaptedWeights: {
		readonly vector: number;
		readonly recency: number;
		readonly frequency: number;
	};
	readonly interestEmbedding: string | undefined;
	readonly totalQueries: number;
	readonly lastUpdated: number;
	readonly explicitFeedback?: ReadonlyArray<ExplicitFeedbackEntry>;
	readonly topicProfiles?: ReadonlyArray<TopicProfileEntry>;
}

/**
 * Type-guard that validates an unknown value conforms to the `LearningState` shape.
 */
export function isValidLearningState(value: unknown): value is LearningState {
	if (typeof value !== 'object' || value === null || Array.isArray(value))
		return false;

	const obj = value as Record<string, unknown>;
	if (obj.version !== 1) return false;
	if (!Array.isArray(obj.feedback)) return false;
	if (!Array.isArray(obj.queryHistory)) return false;
	if (typeof obj.adaptedWeights !== 'object' || obj.adaptedWeights === null)
		return false;
	if (typeof obj.totalQueries !== 'number') return false;
	if (typeof obj.lastUpdated !== 'number') return false;

	// Optional explicitFeedback array
	if (
		obj.explicitFeedback !== undefined &&
		!Array.isArray(obj.explicitFeedback)
	)
		return false;

	// Optional topicProfiles array
	if (obj.topicProfiles !== undefined && !Array.isArray(obj.topicProfiles))
		return false;

	return true;
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
