// ---------------------------------------------------------------------------
// Indexing — TopicIndex, MetadataIndex, MagnitudeCache
// ---------------------------------------------------------------------------
//
// In-memory indexes to accelerate vector store lookups.
// All factories return frozen interfaces backed by plain Maps/Sets.
// No external dependencies.
// ---------------------------------------------------------------------------

import type { RelatedTopic, TopicInfo, VectorEntry } from './types.js';

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
	/** Get all entry IDs associated with a topic and its descendants. */
	readonly getEntries: (topic: string) => readonly string[];
	/** List all known topics with hierarchy info. */
	readonly getAllTopics: () => readonly TopicInfo[];
	/** Get topics for a specific entry. */
	readonly getTopics: (id: string) => readonly string[];
	/** Add an entry to the index, extracting topics from text and metadata. */
	readonly addEntry: (entry: VectorEntry) => void;
	/** Remove an entry from the index. */
	readonly removeEntry: (id: string) => void;
	/** Remove all entries from the index. */
	readonly clear: () => void;
	/** Number of distinct topics tracked. */
	readonly topicCount: number;
	/** Get topics that co-occur with the given topic, sorted by count desc. */
	readonly getRelatedTopics: (topic: string) => readonly RelatedTopic[];
	/** Move all entries from one topic to another. */
	readonly mergeTopic: (from: string, to: string) => void;
	/** Get direct child topic paths (not grandchildren). */
	readonly getChildren: (topic: string) => readonly string[];
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

function extractTopicsFromText(
	text: string,
	stopWords: ReadonlySet<string>,
	maxTopics: number,
): string[] {
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

/**
 * Resolve topics from entry metadata + text fallback.
 *
 * Priority:
 * 1. `metadata.topics` — JSON-stringified string array (multi-topic)
 * 2. `metadata.topic` — single string (comma-separated supported)
 * 3. Auto-extract from text via word frequency
 */
function resolveTopics(
	text: string,
	metadata: Record<string, string>,
	stopWords: ReadonlySet<string>,
	maxTopics: number,
): string[] {
	// 1. metadata.topics (JSON array)
	if (metadata.topics) {
		try {
			const parsed: unknown = JSON.parse(metadata.topics);
			if (Array.isArray(parsed)) {
				return parsed
					.filter((t): t is string => typeof t === 'string')
					.map((t) => t.trim().toLowerCase())
					.filter((t) => t.length > 0);
			}
		} catch {
			// Fall through to next strategy
		}
	}

	// 2. metadata.topic (single string, comma-separated)
	if (metadata.topic) {
		return metadata.topic
			.split(',')
			.map((t) => t.trim().toLowerCase())
			.filter((t) => t.length > 0);
	}

	// 3. Auto-extract from text
	return extractTopicsFromText(text, stopWords, maxTopics);
}

/**
 * Get the direct parent of a topic path, or undefined for root topics.
 */
function getParentPath(topic: string): string | undefined {
	const idx = topic.lastIndexOf('/');
	return idx === -1 ? undefined : topic.slice(0, idx);
}

/**
 * Create a composite key for a pair of topics (order-independent).
 */
function coOccurrenceKey(a: string, b: string): string {
	return a < b ? `${a}\0${b}` : `${b}\0${a}`;
}

export function createTopicIndex(options?: TopicIndexOptions): TopicIndex {
	const maxTopics = options?.maxTopicsPerEntry ?? 5;
	const stopWords = new Set(DEFAULT_STOP_WORDS);
	if (options?.extraStopWords) {
		for (const w of options.extraStopWords) {
			stopWords.add(w.toLowerCase());
		}
	}

	// topic → Set<entryId> (direct entries only, not descendants)
	const topicToEntries = new Map<string, Set<string>>();
	// entryId → topic paths (leaf topics the entry was directly added to)
	const entryToTopics = new Map<string, string[]>();
	// topic → Set<child topic> (direct children only)
	const topicToChildren = new Map<string, Set<string>>();
	// co-occurrence pair key → count
	const coOccurrence = new Map<string, number>();

	/**
	 * Ensure a topic and all its ancestors exist in the index structures.
	 */
	const ensureTopicExists = (topic: string): void => {
		if (!topicToEntries.has(topic)) {
			topicToEntries.set(topic, new Set());
		}
		const parent = getParentPath(topic);
		if (parent !== undefined) {
			ensureTopicExists(parent);
			let children = topicToChildren.get(parent);
			if (!children) {
				children = new Set();
				topicToChildren.set(parent, children);
			}
			children.add(topic);
		}
	};

	/**
	 * Clean up a topic node if it has no direct entries and no children.
	 */
	const cleanupTopic = (topic: string): void => {
		const entries = topicToEntries.get(topic);
		const children = topicToChildren.get(topic);
		if (entries && entries.size === 0 && (!children || children.size === 0)) {
			topicToEntries.delete(topic);
			topicToChildren.delete(topic);
			const parent = getParentPath(topic);
			if (parent !== undefined) {
				const parentChildren = topicToChildren.get(parent);
				if (parentChildren) {
					parentChildren.delete(topic);
				}
				cleanupTopic(parent);
			}
		}
	};

	/**
	 * Increment pairwise co-occurrence counters for a set of topics.
	 */
	const incrementCoOccurrence = (topics: readonly string[]): void => {
		for (let i = 0; i < topics.length; i++) {
			for (let j = i + 1; j < topics.length; j++) {
				const key = coOccurrenceKey(topics[i], topics[j]);
				coOccurrence.set(key, (coOccurrence.get(key) ?? 0) + 1);
			}
		}
	};

	/**
	 * Decrement pairwise co-occurrence counters for a set of topics.
	 */
	const decrementCoOccurrence = (topics: readonly string[]): void => {
		for (let i = 0; i < topics.length; i++) {
			for (let j = i + 1; j < topics.length; j++) {
				const key = coOccurrenceKey(topics[i], topics[j]);
				const current = coOccurrence.get(key) ?? 0;
				if (current <= 1) {
					coOccurrence.delete(key);
				} else {
					coOccurrence.set(key, current - 1);
				}
			}
		}
	};

	/**
	 * Collect all entry IDs for a topic and all its descendants.
	 */
	const collectDescendantEntries = (topic: string): string[] => {
		const result = new Set<string>();
		const directEntries = topicToEntries.get(topic);
		if (directEntries) {
			for (const id of directEntries) {
				result.add(id);
			}
		}
		const children = topicToChildren.get(topic);
		if (children) {
			for (const child of children) {
				for (const id of collectDescendantEntries(child)) {
					result.add(id);
				}
			}
		}
		return [...result];
	};

	const addEntry = (entry: VectorEntry): void => {
		const { id, text, metadata } = entry;
		// Remove existing mapping if re-indexing
		removeEntry(id);

		const topics = resolveTopics(
			text,
			metadata as Record<string, string>,
			stopWords,
			maxTopics,
		);
		entryToTopics.set(id, topics);

		for (const topic of topics) {
			ensureTopicExists(topic);
			topicToEntries.get(topic)?.add(id);
		}

		// Track co-occurrence between all topics on this entry
		if (topics.length > 1) {
			incrementCoOccurrence(topics);
		}
	};

	const removeEntry = (id: string): void => {
		const topics = entryToTopics.get(id);
		if (!topics) return;

		// Decrement co-occurrence before removing
		if (topics.length > 1) {
			decrementCoOccurrence(topics);
		}

		for (const topic of topics) {
			const set = topicToEntries.get(topic);
			if (set) {
				set.delete(id);
				cleanupTopic(topic);
			}
		}
		entryToTopics.delete(id);
	};

	const clear = (): void => {
		topicToEntries.clear();
		entryToTopics.clear();
		topicToChildren.clear();
		coOccurrence.clear();
	};

	const getEntries = (topic: string): readonly string[] => {
		const normalized = topic.toLowerCase();
		return collectDescendantEntries(normalized);
	};

	const getAllTopics = (): readonly TopicInfo[] => {
		const result: TopicInfo[] = [];
		for (const [topic, entries] of topicToEntries) {
			const children = topicToChildren.get(topic);
			result.push({
				topic,
				entryCount: entries.size,
				entryIds: [...entries],
				parent: getParentPath(topic),
				children: children ? [...children] : [],
			});
		}
		return result;
	};

	const getTopics = (id: string): readonly string[] =>
		entryToTopics.get(id) ?? [];

	const getRelatedTopics = (topic: string): readonly RelatedTopic[] => {
		const normalized = topic.toLowerCase();
		const related = new Map<string, number>();
		for (const [key, count] of coOccurrence) {
			const [a, b] = key.split('\0');
			if (a === normalized) {
				related.set(b, (related.get(b) ?? 0) + count);
			} else if (b === normalized) {
				related.set(a, (related.get(a) ?? 0) + count);
			}
		}
		return [...related.entries()]
			.map(([t, c]) => ({ topic: t, coOccurrenceCount: c }))
			.sort((a, b) => b.coOccurrenceCount - a.coOccurrenceCount);
	};

	const mergeTopic = (from: string, to: string): void => {
		const fromNorm = from.toLowerCase();
		const toNorm = to.toLowerCase();

		const fromEntries = topicToEntries.get(fromNorm);
		if (!fromEntries || fromEntries.size === 0) return;

		// Ensure the target topic exists
		ensureTopicExists(toNorm);

		const toEntries = topicToEntries.get(toNorm);
		if (!toEntries) return;

		// Move each entry from `from` to `to`
		for (const id of fromEntries) {
			toEntries.add(id);

			// Update the entry-to-topics mapping
			const topics = entryToTopics.get(id);
			if (topics) {
				// Decrement old co-occurrence for this entry's topic set
				if (topics.length > 1) {
					decrementCoOccurrence(topics);
				}

				// Replace `from` with `to` in the topic list
				const idx = topics.indexOf(fromNorm);
				if (idx !== -1) {
					// Avoid duplicates if entry already has `to`
					if (topics.includes(toNorm)) {
						topics.splice(idx, 1);
					} else {
						topics[idx] = toNorm;
					}
				}

				// Increment new co-occurrence for updated topic set
				if (topics.length > 1) {
					incrementCoOccurrence(topics);
				}
			}
		}

		// Clear the `from` topic entries and clean up
		fromEntries.clear();
		cleanupTopic(fromNorm);

		// Also move co-occurrence counters that reference `from` to `to`
		// (this handles co-occurrences with third-party topics)
		const keysToRemove: string[] = [];
		const updates = new Map<string, number>();
		for (const [key, count] of coOccurrence) {
			const [a, b] = key.split('\0');
			if (a === fromNorm || b === fromNorm) {
				keysToRemove.push(key);
				const other = a === fromNorm ? b : a;
				if (other !== toNorm) {
					const newKey = coOccurrenceKey(toNorm, other);
					updates.set(
						newKey,
						(updates.get(newKey) ?? coOccurrence.get(newKey) ?? 0) + count,
					);
				}
			}
		}
		for (const key of keysToRemove) {
			coOccurrence.delete(key);
		}
		for (const [key, count] of updates) {
			coOccurrence.set(key, count);
		}
	};

	const getChildren = (topic: string): readonly string[] => {
		const normalized = topic.toLowerCase();
		const children = topicToChildren.get(normalized);
		return children ? [...children] : [];
	};

	return Object.freeze({
		getEntries,
		getAllTopics,
		getTopics,
		addEntry,
		removeEntry,
		clear,
		get topicCount() {
			return topicToEntries.size;
		},
		getRelatedTopics,
		mergeTopic,
		getChildren,
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
