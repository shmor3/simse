// ---------------------------------------------------------------------------
// Indexing — TopicIndex, MetadataIndex, MagnitudeCache
// ---------------------------------------------------------------------------
//
// In-memory indexes to accelerate vector store lookups.
// All factories return frozen interfaces backed by plain Maps/Sets.
// No external dependencies.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface TopicIndexOptions {
	/** Maximum number of auto-extracted topics per entry. Defaults to `5`. */
	readonly maxTopicsPerEntry?: number;
	/** Additional stop words to ignore during topic extraction. */
	readonly extraStopWords?: readonly string[];
}

// ---------------------------------------------------------------------------
// Topic Index
// ---------------------------------------------------------------------------

export interface TopicIndex {
	/** Get all entry IDs associated with a given topic. */
	readonly getEntries: (topic: string) => ReadonlySet<string>;
	/** List all known topics. */
	readonly getAllTopics: () => readonly string[];
	/** Get topics for a specific entry. */
	readonly getTopics: (id: string) => readonly string[];
	/** Add an entry to the index, extracting topics from text and metadata. */
	readonly addEntry: (
		id: string,
		text: string,
		metadata: Record<string, string>,
	) => void;
	/** Remove an entry from the index. */
	readonly removeEntry: (id: string) => void;
	/** Remove all entries from the index. */
	readonly clear: () => void;
	/** Number of distinct topics tracked. */
	readonly topicCount: number;
}

// Default stop words for topic extraction
const DEFAULT_STOP_WORDS = new Set([
	'a',
	'an',
	'and',
	'are',
	'as',
	'at',
	'be',
	'but',
	'by',
	'do',
	'for',
	'from',
	'had',
	'has',
	'have',
	'he',
	'her',
	'his',
	'how',
	'i',
	'if',
	'in',
	'into',
	'is',
	'it',
	'its',
	'my',
	'no',
	'not',
	'of',
	'on',
	'or',
	'our',
	'she',
	'so',
	'that',
	'the',
	'their',
	'them',
	'then',
	'there',
	'these',
	'they',
	'this',
	'to',
	'was',
	'we',
	'were',
	'what',
	'when',
	'which',
	'who',
	'will',
	'with',
	'you',
	'your',
]);

const NON_ALPHANUMERIC_RE = /[^\p{L}\p{N}\s]/gu;

function extractTopics(
	text: string,
	metadata: Record<string, string>,
	stopWords: ReadonlySet<string>,
	maxTopics: number,
): string[] {
	// If metadata.topic is explicitly set, use it
	if (metadata.topic) {
		return metadata.topic
			.split(',')
			.map((t) => t.trim().toLowerCase())
			.filter((t) => t.length > 0);
	}

	// Otherwise auto-extract from text
	const words = text
		.toLowerCase()
		.replace(NON_ALPHANUMERIC_RE, '')
		.split(/\s+/)
		.filter((w) => w.length > 2 && !stopWords.has(w));

	// Count frequencies
	const freq = new Map<string, number>();
	for (const word of words) {
		freq.set(word, (freq.get(word) ?? 0) + 1);
	}

	// Return top N by frequency
	return [...freq.entries()]
		.sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
		.slice(0, maxTopics)
		.map(([word]) => word);
}

export function createTopicIndex(options?: TopicIndexOptions): TopicIndex {
	const maxTopics = options?.maxTopicsPerEntry ?? 5;
	const stopWords = new Set(DEFAULT_STOP_WORDS);
	if (options?.extraStopWords) {
		for (const w of options.extraStopWords) {
			stopWords.add(w.toLowerCase());
		}
	}

	// topic → Set<entryId>
	const topicToEntries = new Map<string, Set<string>>();
	// entryId → topics[]
	const entryToTopics = new Map<string, string[]>();

	const addEntry = (
		id: string,
		text: string,
		metadata: Record<string, string>,
	): void => {
		// Remove existing mapping if re-indexing
		removeEntry(id);

		const topics = extractTopics(text, metadata, stopWords, maxTopics);
		entryToTopics.set(id, topics);

		for (const topic of topics) {
			let set = topicToEntries.get(topic);
			if (!set) {
				set = new Set();
				topicToEntries.set(topic, set);
			}
			set.add(id);
		}
	};

	const removeEntry = (id: string): void => {
		const topics = entryToTopics.get(id);
		if (!topics) return;

		for (const topic of topics) {
			const set = topicToEntries.get(topic);
			if (set) {
				set.delete(id);
				if (set.size === 0) {
					topicToEntries.delete(topic);
				}
			}
		}
		entryToTopics.delete(id);
	};

	const clear = (): void => {
		topicToEntries.clear();
		entryToTopics.clear();
	};

	const emptySet: ReadonlySet<string> = new Set();

	return Object.freeze({
		getEntries: (topic: string) =>
			topicToEntries.get(topic.toLowerCase()) ?? emptySet,
		getAllTopics: () => [...topicToEntries.keys()],
		getTopics: (id: string) => entryToTopics.get(id) ?? [],
		addEntry,
		removeEntry,
		clear,
		get topicCount() {
			return topicToEntries.size;
		},
	});
}

// ---------------------------------------------------------------------------
// Metadata Index
// ---------------------------------------------------------------------------

export interface MetadataIndex {
	/** Get entry IDs matching an exact key-value pair (O(1)). */
	readonly getEntries: (key: string, value: string) => ReadonlySet<string>;
	/** Get entry IDs that have a specific metadata key. */
	readonly getEntriesWithKey: (key: string) => ReadonlySet<string>;
	/** Add an entry's metadata to the index. */
	readonly addEntry: (id: string, metadata: Record<string, string>) => void;
	/** Remove an entry from the index. */
	readonly removeEntry: (id: string, metadata: Record<string, string>) => void;
	/** Remove all entries from the index. */
	readonly clear: () => void;
}

export function createMetadataIndex(): MetadataIndex {
	// "key\0value" → Set<entryId>
	const kvIndex = new Map<string, Set<string>>();
	// "key" → Set<entryId>
	const keyIndex = new Map<string, Set<string>>();

	const kvKey = (key: string, value: string): string => `${key}\0${value}`;

	const addEntry = (id: string, metadata: Record<string, string>): void => {
		for (const [key, value] of Object.entries(metadata)) {
			// Key-value index
			const composite = kvKey(key, value);
			let kvSet = kvIndex.get(composite);
			if (!kvSet) {
				kvSet = new Set();
				kvIndex.set(composite, kvSet);
			}
			kvSet.add(id);

			// Key-only index
			let keySet = keyIndex.get(key);
			if (!keySet) {
				keySet = new Set();
				keyIndex.set(key, keySet);
			}
			keySet.add(id);
		}
	};

	const removeEntry = (id: string, metadata: Record<string, string>): void => {
		for (const [key, value] of Object.entries(metadata)) {
			const composite = kvKey(key, value);
			const kvSet = kvIndex.get(composite);
			if (kvSet) {
				kvSet.delete(id);
				if (kvSet.size === 0) kvIndex.delete(composite);
			}

			const keySet = keyIndex.get(key);
			if (keySet) {
				keySet.delete(id);
				if (keySet.size === 0) keyIndex.delete(key);
			}
		}
	};

	const clear = (): void => {
		kvIndex.clear();
		keyIndex.clear();
	};

	const emptySet: ReadonlySet<string> = new Set();

	return Object.freeze({
		getEntries: (key: string, value: string) =>
			kvIndex.get(kvKey(key, value)) ?? emptySet,
		getEntriesWithKey: (key: string) => keyIndex.get(key) ?? emptySet,
		addEntry,
		removeEntry,
		clear,
	});
}

// ---------------------------------------------------------------------------
// Magnitude Cache
// ---------------------------------------------------------------------------

export interface MagnitudeCache {
	/** Get the cached magnitude for an entry. */
	readonly get: (id: string) => number | undefined;
	/** Compute and cache the magnitude for an entry's embedding. */
	readonly set: (id: string, embedding: readonly number[]) => void;
	/** Remove a cached magnitude. */
	readonly remove: (id: string) => void;
	/** Clear all cached magnitudes. */
	readonly clear: () => void;
}

/** Compute the L2 (Euclidean) magnitude of a vector. */
export function computeMagnitude(embedding: readonly number[]): number {
	let sum = 0;
	for (let i = 0; i < embedding.length; i++) {
		sum += embedding[i] * embedding[i];
	}
	return Math.sqrt(sum);
}

export function createMagnitudeCache(): MagnitudeCache {
	const cache = new Map<string, number>();

	return Object.freeze({
		get: (id: string) => cache.get(id),
		set: (id: string, embedding: readonly number[]) => {
			cache.set(id, computeMagnitude(embedding));
		},
		remove: (id: string) => {
			cache.delete(id);
		},
		clear: () => {
			cache.clear();
		},
	});
}
