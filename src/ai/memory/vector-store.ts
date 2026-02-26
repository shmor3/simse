import { randomUUID } from 'node:crypto';
import {
	createMemoryError,
	createVectorStoreCorruptionError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import {
	checkDuplicate as checkDuplicateImpl,
	findDuplicateGroups,
} from './deduplication.js';
import {
	createMagnitudeCache,
	createMetadataIndex,
	createTopicIndex,
	type TopicIndexOptions,
} from './indexing.js';
import { createInvertedIndex } from './inverted-index.js';
import { createLearningEngine, type LearningEngine } from './learning.js';
import type { RecencyOptions } from './recommendation.js';
import type { StorageBackend } from './storage.js';
import type {
	AdvancedSearchResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateGroup,
	LearningOptions,
	LearningProfile,
	MetadataFilter,
	RecommendationResult,
	RecommendOptions,
	SearchOptions,
	SearchResult,
	TextSearchOptions,
	TextSearchResult,
	TopicInfo,
	VectorEntry,
} from './types.js';
import { computeRecommendations } from './vector-recommend.js';
import {
	advancedVectorSearch,
	filterEntriesByDateRange,
	filterEntriesByMetadata,
	textSearchEntries,
	type VectorSearchConfig,
	vectorSearch,
} from './vector-search.js';
import {
	deserializeFromStorage,
	serializeToStorage,
} from './vector-serialize.js';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface VectorStoreOptions {
	/** Pluggable storage backend. Consumers must provide their own implementation. */
	readonly storage: StorageBackend;

	/**
	 * If `true`, every mutation (add / delete / clear) immediately persists.
	 * If `false`, call `save()` manually or rely on `flushIntervalMs`.
	 * Defaults to `false` for better performance.
	 */
	readonly autoSave?: boolean;

	/**
	 * When > 0, the store will automatically flush dirty state every
	 * `flushIntervalMs` milliseconds. Set to 0 to disable.
	 * Only used when `autoSave` is `false`.
	 * Defaults to `5000` (5 seconds).
	 */
	readonly flushIntervalMs?: number;

	/**
	 * Maximum allowed length for regex search patterns. Patterns exceeding this
	 * limit are rejected to prevent ReDoS. Defaults to `256`.
	 */
	readonly maxRegexPatternLength?: number;

	/**
	 * Options for the internal topic index used by `getTopics()` and
	 * `filterByTopic()`.
	 */
	readonly topicIndex?: TopicIndexOptions;

	/**
	 * Cosine similarity threshold for automatic duplicate detection on `add()`.
	 * Set to 0 to disable (default). Values like `0.95` catch near-duplicates.
	 */
	readonly duplicateThreshold?: number;

	/**
	 * Behavior when a duplicate is detected during `add()`.
	 * - `'skip'` — silently skip the duplicate, return the existing entry's ID.
	 * - `'warn'` — log a warning and add the entry anyway.
	 * - `'error'` — throw a MemoryError.
	 * Defaults to `'warn'`.
	 */
	readonly duplicateBehavior?: 'skip' | 'warn' | 'error';

	/**
	 * Options for recency scoring in `recommend()`.
	 * Controls the exponential decay half-life.
	 */
	readonly recency?: RecencyOptions;

	/**
	 * Options for the adaptive learning engine.
	 * When enabled, the store observes search patterns and adapts
	 * recommendation weights and scoring in real time.
	 */
	readonly learning?: LearningOptions;

	/** Inject a custom logger. */
	readonly logger?: Logger;

	/**
	 * Optional text cache for RAM optimization.
	 * When provided, entry texts are cached in the LRU cache and
	 * populated on add/load for faster search result hydration.
	 */
	readonly textCache?: import('./text-cache.js').TextCache;
}

// ---------------------------------------------------------------------------
// VectorStore interface
// ---------------------------------------------------------------------------

export interface VectorStore {
	readonly load: () => Promise<void>;
	readonly save: () => Promise<void>;
	readonly dispose: () => Promise<void>;
	readonly add: (
		text: string,
		embedding: readonly number[],
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly addBatch: (
		entries: ReadonlyArray<{
			text: string;
			embedding: readonly number[];
			metadata?: Record<string, string>;
		}>,
	) => Promise<string[]>;
	readonly delete: (id: string) => Promise<boolean>;
	readonly deleteBatch: (ids: readonly string[]) => Promise<number>;
	readonly clear: () => Promise<void>;
	readonly search: (
		queryEmbedding: readonly number[],
		maxResults: number,
		threshold: number,
	) => SearchResult[];
	readonly textSearch: (options: TextSearchOptions) => TextSearchResult[];
	readonly filterByMetadata: (
		filters: readonly MetadataFilter[],
	) => VectorEntry[];
	readonly filterByDateRange: (range: DateRange) => VectorEntry[];
	readonly advancedSearch: (options: SearchOptions) => AdvancedSearchResult[];
	readonly getAll: () => VectorEntry[];
	readonly getById: (id: string) => VectorEntry | undefined;
	readonly getTopics: () => TopicInfo[];
	readonly filterByTopic: (topics: readonly string[]) => VectorEntry[];
	readonly findDuplicates: (threshold?: number) => DuplicateGroup[];
	readonly checkDuplicate: (
		embedding: readonly number[],
	) => DuplicateCheckResult;
	readonly recommend: (options?: RecommendOptions) => RecommendationResult[];
	/** The adaptive learning engine instance (if learning is enabled). */
	readonly learningEngine: LearningEngine | undefined;
	/** Snapshot of the current learning profile. */
	readonly learningProfile: LearningProfile | undefined;
	readonly size: number;
	readonly isDirty: boolean;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a vector store backed by a pluggable {@link StorageBackend}.
 *
 * Supports cosine-similarity search, text search, metadata filtering,
 * date-range filtering, duplicate detection, topic indexing, and
 * weighted recommendation scoring with adaptive learning.
 *
 * @param options - Storage backend (required), auto-save, flush interval,
 *   duplicate threshold, recency options, learning config, logger.
 * @returns A frozen {@link VectorStore}. Call `load()` before use, `dispose()` when done.
 * @throws {MemoryError} When accessed before `load()` or with empty text/embedding.
 */
export function createVectorStore(options: VectorStoreOptions): VectorStore {
	let entries: VectorEntry[] = [];
	const logger = (options.logger ?? getDefaultLogger()).child('vector-store');
	const storage: StorageBackend = options.storage;
	const autoSave = options.autoSave ?? false;
	const flushIntervalMs = options.flushIntervalMs ?? 5_000;
	const maxRegexPatternLength = options.maxRegexPatternLength ?? 256;
	const duplicateThreshold = options.duplicateThreshold ?? 0;
	const duplicateBehavior = options.duplicateBehavior ?? 'warn';
	const recencyOpts = options.recency;
	const textCache = options.textCache;
	const learningEnabled = options.learning?.enabled ?? true;
	const learningEngine: LearningEngine | undefined = learningEnabled
		? createLearningEngine(options.learning)
		: undefined;

	// Internal indexes
	const topicIdx = createTopicIndex(options.topicIndex);
	const metadataIdx = createMetadataIndex();
	const magnitudeCache = createMagnitudeCache();
	const invertedIdx = createInvertedIndex();

	// Search config passed to pure search functions
	const searchConfig: VectorSearchConfig = {
		maxRegexPatternLength,
		warn: (msg: string) => logger.warn(msg),
	};

	// Access stats for recommendation engine
	const accessStats = new Map<
		string,
		{ accessCount: number; lastAccessed: number }
	>();

	let dirty = false;
	let initialized = false;

	const ensureLoaded = (): void => {
		if (!initialized) {
			throw createMemoryError(
				'VectorStore has not been loaded. Call load() first.',
				{ code: 'VECTOR_STORE_NOT_LOADED' },
			);
		}
	};
	let flushTimer: ReturnType<typeof setInterval> | null = null;
	let saveChain: Promise<void> = Promise.resolve();
	const dirtyIds = new Set<string>();

	// Serializes concurrent mutation calls (add / addBatch / delete / deleteBatch / clear)
	// to prevent race conditions such as duplicate detection bypass.
	let writeLock: Promise<unknown> = Promise.resolve();

	const startFlushTimer = (): void => {
		if (flushTimer || autoSave || flushIntervalMs <= 0) return;

		flushTimer = setInterval(() => {
			if (dirty && initialized) {
				save().catch((err) => {
					logger.error('Background flush failed', err as Error);
				});
			}
		}, flushIntervalMs);

		if (flushTimer && typeof flushTimer === 'object' && 'unref' in flushTimer) {
			flushTimer.unref();
		}
	};

	// -----------------------------------------------------------------------
	// Index management
	// -----------------------------------------------------------------------

	const indexEntry = (entry: VectorEntry): void => {
		topicIdx.addEntry(entry);
		metadataIdx.addEntry(entry.id, entry.metadata as Record<string, string>);
		magnitudeCache.set(entry.id, entry.embedding);
		invertedIdx.addEntry(entry);
	};

	const deindexEntry = (entry: VectorEntry): void => {
		topicIdx.removeEntry(entry.id);
		metadataIdx.removeEntry(entry.id, entry.metadata as Record<string, string>);
		magnitudeCache.remove(entry.id);
		invertedIdx.removeEntry(entry.id, entry.text);
	};

	const trackAccess = (id: string): void => {
		const existing = accessStats.get(id);
		const now = Date.now();
		if (existing) {
			existing.accessCount += 1;
			existing.lastAccessed = now;
		} else {
			accessStats.set(id, { accessCount: 1, lastAccessed: now });
		}
	};

	const rebuildIndexes = (): void => {
		topicIdx.clear();
		metadataIdx.clear();
		magnitudeCache.clear();
		invertedIdx.clear();
		for (const entry of entries) {
			indexEntry(entry);
		}
	};

	const initEmpty = (reason: string): void => {
		entries = [];
		dirty = false;
		initialized = true;
		dirtyIds.clear();
		startFlushTimer();
		logger.debug(reason);
	};

	// Serialize concurrent load() calls to prevent double-initialization
	let loadPromise: Promise<void> | null = null;

	const load = (): Promise<void> => {
		if (initialized) return Promise.resolve();
		if (loadPromise) return loadPromise;
		loadPromise = doLoad().finally(() => {
			loadPromise = null;
		});
		return loadPromise;
	};

	const doLoad = async (): Promise<void> => {
		if (initialized) return;

		let rawData: Map<string, Buffer>;
		try {
			rawData = await storage.load();
		} catch (error) {
			throw createVectorStoreCorruptionError('storage', {
				cause: error,
			});
		}

		if (rawData.size === 0) {
			initEmpty('No data found — starting with empty store');
			return;
		}

		const deserialized = deserializeFromStorage(rawData, logger);

		if (deserialized.skipped > 0) {
			logger.warn(
				`Loaded ${deserialized.entries.length} entries, skipped ${deserialized.skipped} corrupt entries`,
			);
			dirty = true;
		}

		entries = deserialized.entries;
		accessStats.clear();
		for (const [id, stats] of deserialized.accessStats) {
			accessStats.set(id, stats);
		}
		rebuildIndexes();

		// Populate text cache with loaded entries
		if (textCache) {
			for (const entry of entries) {
				textCache.put(entry.id, entry.text);
			}
		}

		// Restore learning state
		if (learningEngine && deserialized.learningState) {
			try {
				learningEngine.restore(deserialized.learningState);
				const validIds = new Set(deserialized.entries.map((e) => e.id));
				learningEngine.pruneEntries(validIds);
				logger.debug(
					`Restored learning state (${learningEngine.totalQueries} queries recorded)`,
				);
			} catch {
				logger.warn('Failed to restore learning state — starting fresh');
			}
		}

		initialized = true;
		dirtyIds.clear();
		startFlushTimer();
		logger.debug(`Loaded ${entries.length} entries from store`);
	};

	const doSave = async (): Promise<void> => {
		const { data } = serializeToStorage(entries, accessStats, learningEngine);

		await storage.save(data);

		dirtyIds.clear();
		dirty = false;
		logger.debug(`Saved ${entries.length} entries to store`);
	};

	const save = (): Promise<void> => {
		saveChain = saveChain.then(
			() => doSave(),
			(prevError) => {
				logger.warn('Previous save failed, retrying', {
					error: String(prevError),
				});
				return doSave();
			},
		);
		return saveChain;
	};

	const dispose = async (): Promise<void> => {
		if (flushTimer !== null) {
			clearInterval(flushTimer);
			flushTimer = null;
		}

		if (!initialized) return;

		// Drain the write lock so no in-flight mutations are missed, then
		// wait for any in-flight save to finish, then final save if dirty.
		await writeLock.catch(() => {});
		await saveChain;
		if (dirty) {
			await save();
		}
		await storage.close();
	};

	// -----------------------------------------------------------------------
	// CRUD
	// -----------------------------------------------------------------------

	const add = (
		text: string,
		embedding: readonly number[],
		metadata: Record<string, string> = {},
	): Promise<string> => {
		ensureLoaded();
		if (text.length === 0) {
			throw createMemoryError('Cannot add empty text to vector store', {
				code: 'VECTOR_STORE_EMPTY_TEXT',
			});
		}

		if (embedding.length === 0) {
			throw createMemoryError('Cannot add entry with empty embedding vector', {
				code: 'VECTOR_STORE_EMPTY_EMBEDDING',
			});
		}

		const doAdd = async (): Promise<string> => {
			// Duplicate detection
			if (duplicateThreshold > 0) {
				const dupResult = checkDuplicateImpl(
					embedding,
					entries,
					duplicateThreshold,
				);
				if (dupResult.isDuplicate) {
					if (duplicateBehavior === 'skip') {
						logger.debug(
							`Skipping duplicate entry (similarity: ${dupResult.similarity?.toFixed(4)})`,
							{ existingId: dupResult.existingEntry?.id },
						);
						return dupResult.existingEntry?.id ?? '';
					}
					if (duplicateBehavior === 'error') {
						throw createMemoryError(
							`Duplicate entry detected (similarity: ${dupResult.similarity?.toFixed(4)}, existing: ${dupResult.existingEntry?.id})`,
							{ code: 'VECTOR_STORE_DUPLICATE' },
						);
					}
					// 'warn' — log and continue
					logger.warn(
						`Adding near-duplicate entry (similarity: ${dupResult.similarity?.toFixed(4)})`,
						{ existingId: dupResult.existingEntry?.id },
					);
				}
			}

			const id = randomUUID();
			const newEntry: VectorEntry = {
				id,
				text,
				embedding,
				metadata,
				timestamp: Date.now(),
			};
			entries.push(newEntry);
			indexEntry(newEntry);
			textCache?.put(id, text);
			dirty = true;
			dirtyIds.add(id);

			if (autoSave) {
				await save();
			}

			logger.debug(`Added entry "${id}"`, {
				textLength: text.length,
				embeddingDim: embedding.length,
			});

			return id;
		};

		// Serialize via write lock to prevent concurrent duplicate detection bypass
		const result = writeLock.then(doAdd, doAdd);
		writeLock = result.catch(() => {});
		return result;
	};

	const addBatch = (
		batchEntries: ReadonlyArray<{
			text: string;
			embedding: readonly number[];
			metadata?: Record<string, string>;
		}>,
	): Promise<string[]> => {
		ensureLoaded();
		if (batchEntries.length === 0) return Promise.resolve([]);

		// Validate ALL entries before mutating state
		for (const entry of batchEntries) {
			if (entry.text.length === 0) {
				throw createMemoryError(
					'Cannot add empty text to vector store (in batch)',
					{ code: 'VECTOR_STORE_EMPTY_TEXT' },
				);
			}

			if (entry.embedding.length === 0) {
				throw createMemoryError(
					'Cannot add entry with empty embedding vector (in batch)',
					{ code: 'VECTOR_STORE_EMPTY_EMBEDDING' },
				);
			}
		}

		const doAddBatch = async (): Promise<string[]> => {
			const ids: string[] = [];
			const now = Date.now();

			for (const entry of batchEntries) {
				// Duplicate detection (consistent with add())
				if (duplicateThreshold > 0) {
					const dupResult = checkDuplicateImpl(
						entry.embedding,
						entries,
						duplicateThreshold,
					);
					if (dupResult.isDuplicate) {
						if (duplicateBehavior === 'skip') {
							logger.debug(
								`Skipping duplicate entry in batch (similarity: ${dupResult.similarity?.toFixed(4)})`,
								{ existingId: dupResult.existingEntry?.id },
							);
							ids.push(dupResult.existingEntry?.id ?? '');
							continue;
						}
						if (duplicateBehavior === 'error') {
							throw createMemoryError(
								`Duplicate entry detected in batch (similarity: ${dupResult.similarity?.toFixed(4)}, existing: ${dupResult.existingEntry?.id})`,
								{ code: 'VECTOR_STORE_DUPLICATE' },
							);
						}
						// 'warn' — log and continue
						logger.warn(
							`Adding near-duplicate entry in batch (similarity: ${dupResult.similarity?.toFixed(4)})`,
							{ existingId: dupResult.existingEntry?.id },
						);
					}
				}

				const id = randomUUID();
				ids.push(id);
				const newEntry: VectorEntry = {
					id,
					text: entry.text,
					embedding: entry.embedding,
					metadata: entry.metadata ?? {},
					timestamp: now,
				};
				entries.push(newEntry);
				indexEntry(newEntry);
				textCache?.put(id, entry.text);
				dirtyIds.add(id);
			}

			dirty = true;

			if (autoSave) {
				await save();
			}

			logger.debug(`Added batch of ${ids.length} entries`);
			return ids;
		};

		const result = writeLock.then(doAddBatch, doAddBatch);
		writeLock = result.catch(() => {});
		return result;
	};

	const deleteEntry = (id: string): Promise<boolean> => {
		ensureLoaded();

		const doDelete = async (): Promise<boolean> => {
			const idx = entries.findIndex((e) => e.id === id);
			if (idx === -1) {
				logger.debug(`Entry "${id}" not found for deletion`);
				return false;
			}

			const existing = entries[idx];
			entries.splice(idx, 1);
			deindexEntry(existing);
			accessStats.delete(id);
			textCache?.remove(id);
			dirtyIds.delete(id);
			dirty = true;

			if (autoSave) {
				await save();
			}
			logger.debug(`Deleted entry "${id}"`);
			return true;
		};

		const result = writeLock.then(doDelete, doDelete);
		writeLock = result.catch(() => {});
		return result;
	};

	const deleteBatch = (ids: readonly string[]): Promise<number> => {
		ensureLoaded();
		if (ids.length === 0) return Promise.resolve(0);

		const doDeleteBatch = async (): Promise<number> => {
			const idSet = new Set(ids);
			const toRemove = entries.filter((e) => idSet.has(e.id));
			if (toRemove.length === 0) return 0;

			for (const entry of toRemove) {
				deindexEntry(entry);
				accessStats.delete(entry.id);
				textCache?.remove(entry.id);
				dirtyIds.delete(entry.id);
			}
			entries = entries.filter((e) => !idSet.has(e.id));
			dirty = true;

			if (autoSave) {
				await save();
			}
			logger.debug(`Deleted batch of ${toRemove.length} entries`);

			return toRemove.length;
		};

		const result = writeLock.then(doDeleteBatch, doDeleteBatch);
		writeLock = result.catch(() => {});
		return result;
	};

	const clear = (): Promise<void> => {
		ensureLoaded();

		const doClear = async (): Promise<void> => {
			const count = entries.length;
			entries = [];
			topicIdx.clear();
			metadataIdx.clear();
			magnitudeCache.clear();
			invertedIdx.clear();
			accessStats.clear();
			textCache?.clear();
			dirtyIds.clear();
			if (learningEngine) learningEngine.clear();
			dirty = true;

			if (autoSave) {
				await save();
			}

			logger.debug(`Cleared store (removed ${count} entries)`);
		};

		const result = writeLock.then(doClear, doClear);
		writeLock = result.catch(() => {});
		return result;
	};

	// -----------------------------------------------------------------------
	// Search
	// -----------------------------------------------------------------------

	const search = (
		queryEmbedding: readonly number[],
		maxResults: number,
		threshold: number,
	): SearchResult[] => {
		ensureLoaded();
		if (queryEmbedding.length === 0) {
			logger.warn('Search called with empty query embedding');
			return [];
		}

		const results = vectorSearch(
			entries,
			queryEmbedding,
			maxResults,
			threshold,
			magnitudeCache,
		);

		for (const r of results) {
			trackAccess(r.entry.id);
		}

		// Record query for adaptive learning
		if (learningEngine && results.length > 0) {
			learningEngine.recordQuery(
				queryEmbedding,
				results.map((r) => r.entry.id),
			);
			dirty = true;
		}

		return results;
	};

	// -----------------------------------------------------------------------
	// Text Search
	// -----------------------------------------------------------------------

	const textSearch = (searchOptions: TextSearchOptions): TextSearchResult[] => {
		ensureLoaded();
		if (searchOptions.query.length === 0) {
			logger.warn('textSearch called with empty query');
			return [];
		}

		return textSearchEntries(entries, searchOptions, searchConfig, invertedIdx);
	};

	// -----------------------------------------------------------------------
	// Metadata Filtering
	// -----------------------------------------------------------------------

	const filterByMetadata = (
		filters: readonly MetadataFilter[],
	): VectorEntry[] => {
		ensureLoaded();
		return filterEntriesByMetadata(entries, filters, metadataIdx);
	};

	// -----------------------------------------------------------------------
	// Date Range Filtering
	// -----------------------------------------------------------------------

	const filterByDateRange = (range: DateRange): VectorEntry[] => {
		ensureLoaded();
		return filterEntriesByDateRange(entries, range);
	};

	// -----------------------------------------------------------------------
	// Advanced / Combined Search
	// -----------------------------------------------------------------------

	const advancedSearch = (
		searchOptions: SearchOptions,
	): AdvancedSearchResult[] => {
		ensureLoaded();

		const topResults = advancedVectorSearch(
			entries,
			searchOptions,
			searchConfig,
			magnitudeCache,
			metadataIdx,
			invertedIdx,
		);

		for (const r of topResults) {
			trackAccess(r.entry.id);
		}

		// Record query for adaptive learning (only if we have a query embedding)
		const queryEmbedding = searchOptions.queryEmbedding;
		if (
			learningEngine &&
			topResults.length > 0 &&
			queryEmbedding &&
			queryEmbedding.length > 0
		) {
			learningEngine.recordQuery(
				queryEmbedding,
				topResults.map((r) => r.entry.id),
			);
			dirty = true;
		}

		return topResults;
	};

	// -----------------------------------------------------------------------
	// Accessors
	// -----------------------------------------------------------------------

	const getAll = (): VectorEntry[] => {
		ensureLoaded();
		return [...entries];
	};

	const getById = (id: string): VectorEntry | undefined => {
		ensureLoaded();
		const entry = entries.find((e) => e.id === id);
		if (entry) {
			trackAccess(entry.id);
		}
		return entry;
	};

	const getTopics = (): TopicInfo[] => {
		ensureLoaded();
		return [...topicIdx.getAllTopics()];
	};

	const filterByTopic = (topics: readonly string[]): VectorEntry[] => {
		ensureLoaded();
		if (topics.length === 0) return [...entries];

		// Collect all entry IDs that match any of the requested topics
		const matchingIds = new Set<string>();
		for (const topic of topics) {
			for (const id of topicIdx.getEntries(topic)) {
				matchingIds.add(id);
			}
		}

		return entries.filter((e) => matchingIds.has(e.id));
	};

	const findDuplicates = (threshold?: number): DuplicateGroup[] => {
		ensureLoaded();
		const t = threshold ?? duplicateThreshold;
		if (t <= 0) {
			logger.warn(
				'findDuplicates called with no threshold — provide a threshold or set duplicateThreshold in options',
			);
			return [];
		}
		return findDuplicateGroups(entries, t);
	};

	const checkDuplicate = (
		embedding: readonly number[],
	): DuplicateCheckResult => {
		ensureLoaded();
		return checkDuplicateImpl(embedding, entries, duplicateThreshold);
	};

	// -----------------------------------------------------------------------
	// Recommendation
	// -----------------------------------------------------------------------

	const recommend = (
		recommendOptions?: RecommendOptions,
	): RecommendationResult[] => {
		ensureLoaded();

		// Delegate to the pure recommendation function.
		// Do not call trackAccess here to avoid a positive feedback loop
		// where frequently recommended entries inflate their own frequency scores.
		return computeRecommendations(
			entries,
			accessStats,
			recommendOptions ?? {},
			magnitudeCache,
			topicIdx,
			metadataIdx,
			learningEngine,
			recencyOpts,
		);
	};

	// -----------------------------------------------------------------------
	// Return the record
	// -----------------------------------------------------------------------

	return Object.freeze({
		load,
		save,
		dispose,
		add,
		addBatch,
		delete: deleteEntry,
		deleteBatch,
		clear,
		search,
		textSearch,
		filterByMetadata,
		filterByDateRange,
		advancedSearch,
		getAll,
		getById,
		getTopics,
		filterByTopic,
		findDuplicates,
		checkDuplicate,
		recommend,
		get learningEngine() {
			return learningEngine;
		},
		get learningProfile() {
			return learningEngine?.getProfile();
		},
		get size() {
			return entries.length;
		},
		get isDirty() {
			return dirty;
		},
	});
}
