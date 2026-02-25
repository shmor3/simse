import { Buffer } from 'node:buffer';
import { randomUUID } from 'node:crypto';
import {
	createMemoryError,
	createVectorStoreCorruptionError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import { decodeEmbedding, encodeEmbedding } from './compression.js';
import {
	checkDuplicate as checkDuplicateImpl,
	findDuplicateGroups,
} from './deduplication.js';
import {
	computeMagnitude,
	createMagnitudeCache,
	createMetadataIndex,
	createTopicIndex,
	type TopicIndexOptions,
} from './indexing.js';
import { createLearningEngine, type LearningEngine } from './learning.js';
import {
	computeRecommendationScore,
	frequencyScore,
	normalizeWeights,
	type RecencyOptions,
	recencyScore,
} from './recommendation.js';
import type { StorageBackend } from './storage.js';
import {
	fuzzyScore,
	matchesAllMetadataFilters,
	tokenOverlapScore,
} from './text-search.js';
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
import { isValidLearningState } from './vector-persistence.js';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface VectorStoreOptions {
	/** Pluggable storage backend. Consumers must provide their own implementation. */
	storage: StorageBackend;

	/**
	 * If `true`, every mutation (add / delete / clear) immediately persists.
	 * If `false`, call `save()` manually or rely on `flushIntervalMs`.
	 * Defaults to `false` for better performance.
	 */
	autoSave?: boolean;

	/**
	 * When > 0, the store will automatically flush dirty state every
	 * `flushIntervalMs` milliseconds. Set to 0 to disable.
	 * Only used when `autoSave` is `false`.
	 * Defaults to `5000` (5 seconds).
	 */
	flushIntervalMs?: number;

	/**
	 * Maximum allowed length for regex search patterns. Patterns exceeding this
	 * limit are rejected to prevent ReDoS. Defaults to `256`.
	 */
	maxRegexPatternLength?: number;

	/**
	 * Options for the internal topic index used by `getTopics()` and
	 * `filterByTopic()`.
	 */
	topicIndex?: TopicIndexOptions;

	/**
	 * Cosine similarity threshold for automatic duplicate detection on `add()`.
	 * Set to 0 to disable (default). Values like `0.95` catch near-duplicates.
	 */
	duplicateThreshold?: number;

	/**
	 * Behavior when a duplicate is detected during `add()`.
	 * - `'skip'` — silently skip the duplicate, return the existing entry's ID.
	 * - `'warn'` — log a warning and add the entry anyway.
	 * - `'error'` — throw a MemoryError.
	 * Defaults to `'warn'`.
	 */
	duplicateBehavior?: 'skip' | 'warn' | 'error';

	/**
	 * Options for recency scoring in `recommend()`.
	 * Controls the exponential decay half-life.
	 */
	recency?: RecencyOptions;

	/**
	 * Options for the adaptive learning engine.
	 * When enabled, the store observes search patterns and adapts
	 * recommendation weights and scoring in real time.
	 */
	learning?: LearningOptions;

	/** Inject a custom logger. */
	logger?: Logger;
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
		embedding: number[],
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly addBatch: (
		entries: Array<{
			text: string;
			embedding: number[];
			metadata?: Record<string, string>;
		}>,
	) => Promise<string[]>;
	readonly delete: (id: string) => Promise<boolean>;
	readonly deleteBatch: (ids: string[]) => Promise<number>;
	readonly clear: () => Promise<void>;
	readonly search: (
		queryEmbedding: number[],
		maxResults: number,
		threshold: number,
	) => SearchResult[];
	readonly textSearch: (options: TextSearchOptions) => TextSearchResult[];
	readonly filterByMetadata: (filters: MetadataFilter[]) => VectorEntry[];
	readonly filterByDateRange: (range: DateRange) => VectorEntry[];
	readonly advancedSearch: (options: SearchOptions) => AdvancedSearchResult[];
	readonly getAll: () => VectorEntry[];
	readonly getById: (id: string) => VectorEntry | undefined;
	readonly getTopics: () => TopicInfo[];
	readonly filterByTopic: (topics: string[]) => VectorEntry[];
	readonly findDuplicates: (threshold?: number) => DuplicateGroup[];
	readonly checkDuplicate: (embedding: number[]) => DuplicateCheckResult;
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
	const learningEnabled = options.learning?.enabled ?? true;
	const learningEngine: LearningEngine | undefined = learningEnabled
		? createLearningEngine(options.learning)
		: undefined;

	// Internal indexes
	const topicIdx = createTopicIndex(options.topicIndex);
	const metadataIdx = createMetadataIndex();
	const magnitudeCache = createMagnitudeCache();

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
	// Internal search helpers
	// -----------------------------------------------------------------------

	/**
	 * Fast cosine similarity using pre-computed magnitudes.
	 * Returns undefined if vectors are incompatible or zero-magnitude.
	 */
	const fastCosine = (
		queryEmbedding: readonly number[],
		queryMag: number,
		entry: VectorEntry,
	): number | undefined => {
		if (entry.embedding.length !== queryEmbedding.length) return undefined;
		const entryMag =
			magnitudeCache.get(entry.id) ?? computeMagnitude(entry.embedding);
		if (entryMag === 0) return undefined;
		let dot = 0;
		for (let i = 0; i < queryEmbedding.length; i++) {
			dot += queryEmbedding[i] * entry.embedding[i];
		}
		const raw = dot / (queryMag * entryMag);
		// Clamp to [-1, 1] to guard against floating-point rounding
		return Number.isFinite(raw) ? Math.min(1, Math.max(-1, raw)) : undefined;
	};

	const scoreText = (
		candidate: string,
		query: string,
		mode: string,
		compiledRegex?: RegExp,
	): number => {
		switch (mode) {
			case 'fuzzy':
				return fuzzyScore(query, candidate);

			case 'substring':
				return candidate.toLowerCase().includes(query.toLowerCase()) ? 1 : 0;

			case 'exact':
				return candidate === query ? 1 : 0;

			case 'regex': {
				if (compiledRegex) {
					return compiledRegex.test(candidate) ? 1 : 0;
				}
				// Fallback: compile once (should not normally reach here)
				if (query.length > maxRegexPatternLength) {
					logger.warn(
						`Regex pattern exceeds ${maxRegexPatternLength} chars, skipping`,
					);
					return 0;
				}
				try {
					return new RegExp(query).test(candidate) ? 1 : 0;
				} catch {
					logger.warn(`Invalid regex pattern: ${query}`);
					return 0;
				}
			}

			case 'token':
				return tokenOverlapScore(query, candidate);

			default:
				return 0;
		}
	};

	const combineScores = (
		vectorScore: number | undefined,
		textScore: number | undefined,
		rankBy: string,
	): number => {
		const v = vectorScore ?? 0;
		const t = textScore ?? 0;
		const hasVector = vectorScore !== undefined;
		const hasText = textScore !== undefined;

		if (!hasVector && !hasText) return 0;
		if (!hasVector) return t;
		if (!hasText) return v;

		switch (rankBy) {
			case 'vector':
				return v;
			case 'text':
				return t;
			case 'multiply':
				return v * t;
			default:
				return (v + t) / 2;
		}
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	// -----------------------------------------------------------------------
	// Index management
	// -----------------------------------------------------------------------

	const indexEntry = (entry: VectorEntry): void => {
		topicIdx.addEntry(
			entry.id,
			entry.text,
			entry.metadata as Record<string, string>,
		);
		metadataIdx.addEntry(entry.id, entry.metadata as Record<string, string>);
		magnitudeCache.set(entry.id, entry.embedding);
	};

	const deindexEntry = (entry: VectorEntry): void => {
		topicIdx.removeEntry(entry.id);
		metadataIdx.removeEntry(entry.id, entry.metadata as Record<string, string>);
		magnitudeCache.remove(entry.id);
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
		for (const entry of entries) {
			indexEntry(entry);
		}
	};

	// -----------------------------------------------------------------------
	// Entry serialization — binary format per entry value in the KV store
	// [4b text-len][text][4b emb-b64-len][emb-b64][4b meta-json-len][meta-json]
	// [8b timestamp][4b accessCount][8b lastAccessed]
	// -----------------------------------------------------------------------

	const LEARNING_KEY = '__learning';

	const serializeEntry = (
		entry: VectorEntry,
		stats?: { accessCount: number; lastAccessed: number },
	): Buffer => {
		const textBuf = Buffer.from(entry.text, 'utf-8');
		const embBuf = Buffer.from(encodeEmbedding(entry.embedding), 'utf-8');
		const metaBuf = Buffer.from(JSON.stringify(entry.metadata), 'utf-8');

		const totalSize =
			4 + textBuf.length + 4 + embBuf.length + 4 + metaBuf.length + 8 + 4 + 8;

		const buf = Buffer.alloc(totalSize);
		let offset = 0;

		buf.writeUInt32BE(textBuf.length, offset);
		offset += 4;
		textBuf.copy(buf, offset);
		offset += textBuf.length;

		buf.writeUInt32BE(embBuf.length, offset);
		offset += 4;
		embBuf.copy(buf, offset);
		offset += embBuf.length;

		buf.writeUInt32BE(metaBuf.length, offset);
		offset += 4;
		metaBuf.copy(buf, offset);
		offset += metaBuf.length;

		// Timestamp as two 32-bit halves (JS numbers are 64-bit floats)
		const ts = entry.timestamp;
		buf.writeUInt32BE(Math.floor(ts / 0x100000000), offset);
		offset += 4;
		buf.writeUInt32BE(ts >>> 0, offset);
		offset += 4;

		buf.writeUInt32BE(stats?.accessCount ?? 0, offset);
		offset += 4;

		const la = stats?.lastAccessed ?? 0;
		buf.writeUInt32BE(Math.floor(la / 0x100000000), offset);
		offset += 4;
		buf.writeUInt32BE(la >>> 0, offset);

		return buf;
	};

	const deserializeEntry = (
		id: string,
		buf: Buffer,
	): {
		entry: VectorEntry;
		accessCount: number;
		lastAccessed: number;
	} | null => {
		try {
			let offset = 0;

			const textLen = buf.readUInt32BE(offset);
			offset += 4;
			const text = buf.toString('utf-8', offset, offset + textLen);
			offset += textLen;

			const embLen = buf.readUInt32BE(offset);
			offset += 4;
			const embB64 = buf.toString('utf-8', offset, offset + embLen);
			offset += embLen;
			const embedding = decodeEmbedding(embB64);

			const metaLen = buf.readUInt32BE(offset);
			offset += 4;
			const metaJson = buf.toString('utf-8', offset, offset + metaLen);
			offset += metaLen;
			const metadata: Record<string, string> = JSON.parse(metaJson);

			const tsHigh = buf.readUInt32BE(offset);
			offset += 4;
			const tsLow = buf.readUInt32BE(offset);
			offset += 4;
			const timestamp = tsHigh * 0x100000000 + tsLow;

			const accessCount = buf.readUInt32BE(offset);
			offset += 4;

			const laHigh = buf.readUInt32BE(offset);
			offset += 4;
			const laLow = buf.readUInt32BE(offset);
			const lastAccessed = laHigh * 0x100000000 + laLow;

			return {
				entry: { id, text, embedding, metadata, timestamp },
				accessCount,
				lastAccessed,
			};
		} catch {
			return null;
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

		let data: Map<string, Buffer>;
		try {
			data = await storage.load();
		} catch (error) {
			throw createVectorStoreCorruptionError('storage', {
				cause: error,
			});
		}

		if (data.size === 0) {
			initEmpty('No data found — starting with empty store');
			return;
		}

		const valid: VectorEntry[] = [];
		let skipped = 0;
		accessStats.clear();

		for (const [key, value] of data) {
			if (key === LEARNING_KEY) continue; // handled below

			const result = deserializeEntry(key, value);
			if (result === null) {
				skipped++;
				logger.warn(`Skipping corrupt entry: ${key}`);
				continue;
			}

			valid.push(result.entry);

			if (result.accessCount > 0 || result.lastAccessed > 0) {
				accessStats.set(result.entry.id, {
					accessCount: result.accessCount,
					lastAccessed: result.lastAccessed,
				});
			}
		}

		if (skipped > 0) {
			logger.warn(
				`Loaded ${valid.length} entries, skipped ${skipped} corrupt entries`,
			);
			dirty = true;
		}

		entries = valid;
		rebuildIndexes();

		// Restore learning state
		if (learningEngine && data.has(LEARNING_KEY)) {
			try {
				const learningBuf = data.get(LEARNING_KEY);
				if (!learningBuf) throw new Error('Missing learning key');
				const learningJson = learningBuf.toString('utf-8');
				const learningParsed: unknown = JSON.parse(learningJson);
				if (isValidLearningState(learningParsed)) {
					learningEngine.restore(learningParsed);
					const validIds = new Set(valid.map((e) => e.id));
					learningEngine.pruneEntries(validIds);
					logger.debug(
						`Restored learning state (${learningEngine.totalQueries} queries recorded)`,
					);
				} else {
					logger.warn('Invalid learning state — starting fresh');
				}
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
		const data = new Map<string, Buffer>();

		for (const entry of entries) {
			const stats = accessStats.get(entry.id);
			data.set(entry.id, serializeEntry(entry, stats));
		}

		// Persist learning state alongside entries
		if (learningEngine?.hasData) {
			const learningState = learningEngine.serialize();
			const learningJson = JSON.stringify(learningState);
			data.set(LEARNING_KEY, Buffer.from(learningJson, 'utf-8'));
		}

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
		embedding: number[],
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
		batchEntries: Array<{
			text: string;
			embedding: number[];
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

	const deleteBatch = (ids: string[]): Promise<number> => {
		ensureLoaded();
		if (ids.length === 0) return Promise.resolve(0);

		const doDeleteBatch = async (): Promise<number> => {
			const idSet = new Set(ids);
			const toRemove = entries.filter((e) => idSet.has(e.id));
			if (toRemove.length === 0) return 0;

			for (const entry of toRemove) {
				deindexEntry(entry);
				accessStats.delete(entry.id);
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
			accessStats.clear();
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
		queryEmbedding: number[],
		maxResults: number,
		threshold: number,
	): SearchResult[] => {
		ensureLoaded();
		if (queryEmbedding.length === 0) {
			logger.warn('Search called with empty query embedding');
			return [];
		}

		// Pre-compute query magnitude once
		const queryMag = computeMagnitude(queryEmbedding);
		if (queryMag === 0) return [];

		const scored: SearchResult[] = [];

		for (const entry of entries) {
			const score = fastCosine(queryEmbedding, queryMag, entry);
			if (score === undefined) continue;
			if (score >= threshold) {
				scored.push({ entry, score });
			}
		}

		scored.sort((a, b) => b.score - a.score);
		const results = scored.slice(0, maxResults);
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
		const { query, mode = 'fuzzy', threshold = 0.3 } = searchOptions;

		if (query.length === 0) {
			logger.warn('textSearch called with empty query');
			return [];
		}

		// Compile regex once before the loop
		let compiledRegex: RegExp | undefined;
		if (mode === 'regex') {
			if (query.length > maxRegexPatternLength) {
				logger.warn(
					`Regex pattern exceeds ${maxRegexPatternLength} chars, skipping`,
				);
				return [];
			}
			try {
				compiledRegex = new RegExp(query);
			} catch {
				logger.warn(`Invalid regex pattern: ${query}`);
				return [];
			}
		}

		const results: TextSearchResult[] = [];

		for (const entry of entries) {
			const score = scoreText(entry.text, query, mode, compiledRegex);
			if (score >= threshold) {
				results.push({ entry, score });
			}
		}

		results.sort((a, b) => b.score - a.score);
		return results;
	};

	// -----------------------------------------------------------------------
	// Metadata Filtering
	// -----------------------------------------------------------------------

	const filterByMetadata = (filters: MetadataFilter[]): VectorEntry[] => {
		ensureLoaded();
		if (filters.length === 0) return [...entries];

		// Optimization: if all filters are simple "eq" mode, use the metadata index
		const allEq = filters.every(
			(f) => (f.mode ?? 'eq') === 'eq' && f.value !== undefined,
		);
		if (allEq) {
			// Intersect sets from the metadata index
			let candidateIds: Set<string> | undefined;
			for (const f of filters) {
				const ids = metadataIdx.getEntries(f.key, f.value as string);
				if (candidateIds === undefined) {
					candidateIds = new Set(ids);
				} else {
					for (const id of candidateIds) {
						if (!ids.has(id)) candidateIds.delete(id);
					}
				}
				if (candidateIds.size === 0) return [];
			}
			if (!candidateIds) return [];
			return entries.filter((e) => candidateIds.has(e.id));
		}

		// Fallback: linear scan for complex filter modes
		return entries.filter((e) =>
			matchesAllMetadataFilters(e.metadata, filters),
		);
	};

	// -----------------------------------------------------------------------
	// Date Range Filtering
	// -----------------------------------------------------------------------

	const filterByDateRange = (range: DateRange): VectorEntry[] => {
		ensureLoaded();
		return entries.filter((e) => {
			if (range.after !== undefined && e.timestamp < range.after) return false;
			if (range.before !== undefined && e.timestamp > range.before)
				return false;
			return true;
		});
	};

	// -----------------------------------------------------------------------
	// Advanced / Combined Search
	// -----------------------------------------------------------------------

	const advancedSearch = (
		searchOptions: SearchOptions,
	): AdvancedSearchResult[] => {
		ensureLoaded();
		const {
			queryEmbedding,
			similarityThreshold = 0,
			text,
			metadata,
			dateRange,
			maxResults = 10,
			rankBy = 'average',
		} = searchOptions;

		const results: AdvancedSearchResult[] = [];

		// Pre-compute query magnitude for fast cosine
		const queryMag =
			queryEmbedding && queryEmbedding.length > 0
				? computeMagnitude(queryEmbedding)
				: 0;

		// Pre-compile regex for text search if needed
		let compiledRegex: RegExp | undefined;
		if (text && (text.mode ?? 'fuzzy') === 'regex') {
			if (text.query.length > maxRegexPatternLength) {
				logger.warn(
					`Regex pattern exceeds ${maxRegexPatternLength} chars, skipping text filter`,
				);
			} else {
				try {
					compiledRegex = new RegExp(text.query);
				} catch {
					logger.warn(`Invalid regex pattern: ${text.query}`);
				}
			}
		}

		for (const entry of entries) {
			if (dateRange) {
				if (dateRange.after !== undefined && entry.timestamp < dateRange.after)
					continue;
				if (
					dateRange.before !== undefined &&
					entry.timestamp > dateRange.before
				)
					continue;
			}

			if (metadata && metadata.length > 0) {
				if (
					!matchesAllMetadataFilters(
						entry.metadata,
						metadata as MetadataFilter[],
					)
				)
					continue;
			}

			let vectorScore: number | undefined;
			if (queryEmbedding && queryEmbedding.length > 0 && queryMag > 0) {
				vectorScore = fastCosine(queryEmbedding, queryMag, entry);
				if (vectorScore === undefined) continue;
				if (vectorScore < similarityThreshold) continue;
			}

			let textScoreVal: number | undefined;
			if (text) {
				const mode = text.mode ?? 'fuzzy';
				const textThreshold = text.threshold ?? 0.3;
				textScoreVal = scoreText(entry.text, text.query, mode, compiledRegex);
				if (textScoreVal < textThreshold) continue;
			}

			const finalScore = combineScores(vectorScore, textScoreVal, rankBy);

			results.push({
				entry,
				score: finalScore,
				scores: {
					vector: vectorScore,
					text: textScoreVal,
				},
			});
		}

		results.sort((a, b) => b.score - a.score);
		const topResults = results.slice(0, maxResults);
		for (const r of topResults) {
			trackAccess(r.entry.id);
		}

		// Record query for adaptive learning (only if we have a query embedding)
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
		return topicIdx.getAllTopics().map((topic) => {
			const entryIds = [...topicIdx.getEntries(topic)];
			return { topic, entryCount: entryIds.length, entryIds };
		});
	};

	const filterByTopic = (topics: string[]): VectorEntry[] => {
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

	const checkDuplicate = (embedding: number[]): DuplicateCheckResult => {
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
		const opts = recommendOptions ?? {};
		// Use adapted weights from learning engine if available, falling back to user-provided or defaults
		const baseWeights =
			learningEngine && !opts.weights
				? learningEngine.getAdaptedWeights()
				: normalizeWeights(opts.weights);
		const weights = baseWeights;
		const maxResults = opts.maxResults ?? 10;
		const minScore = opts.minScore ?? 0;

		// Pre-filter candidates
		let candidates = entries;

		// Topic filter
		if (opts.topics && opts.topics.length > 0) {
			const matchingIds = new Set<string>();
			for (const topic of opts.topics) {
				for (const id of topicIdx.getEntries(topic)) {
					matchingIds.add(id);
				}
			}
			candidates = candidates.filter((e) => matchingIds.has(e.id));
		}

		// Metadata filter
		if (opts.metadata && opts.metadata.length > 0) {
			candidates = candidates.filter((e) =>
				matchesAllMetadataFilters(
					e.metadata,
					opts.metadata as MetadataFilter[],
				),
			);
		}

		// Date range filter
		if (opts.dateRange) {
			const { after, before } = opts.dateRange;
			candidates = candidates.filter((e) => {
				if (after !== undefined && e.timestamp < after) return false;
				if (before !== undefined && e.timestamp > before) return false;
				return true;
			});
		}

		if (candidates.length === 0) return [];

		// Find max access count for frequency normalization
		let maxAccessCount = 0;
		for (const entry of candidates) {
			const stats = accessStats.get(entry.id);
			if (stats && stats.accessCount > maxAccessCount) {
				maxAccessCount = stats.accessCount;
			}
		}

		// Pre-compute query magnitude for vector scoring
		const queryEmbedding = opts.queryEmbedding;
		const queryMag =
			queryEmbedding && queryEmbedding.length > 0
				? computeMagnitude(queryEmbedding)
				: 0;

		const results: RecommendationResult[] = [];

		for (const entry of candidates) {
			// Vector similarity score
			let vectorScoreVal: number | undefined;
			if (queryEmbedding && queryEmbedding.length > 0 && queryMag > 0) {
				vectorScoreVal = fastCosine(queryEmbedding, queryMag, entry);
			}

			// Recency score
			const recencyVal = recencyScore(entry.timestamp, recencyOpts);

			// Frequency score
			const stats = accessStats.get(entry.id);
			const freqVal = frequencyScore(stats?.accessCount ?? 0, maxAccessCount);

			const recommendation = computeRecommendationScore(
				{
					vectorScore: vectorScoreVal,
					recencyScore: recencyVal,
					frequencyScore: freqVal,
				},
				weights,
			);

			// Apply learning boost if available
			const boost = learningEngine
				? learningEngine.computeBoost(entry.id, entry.embedding)
				: 1.0;
			const boostedScore = recommendation.score * boost;

			if (boostedScore >= minScore) {
				results.push({
					entry,
					score: boostedScore,
					scores: recommendation.scores,
				});
			}
		}

		results.sort((a, b) => b.score - a.score);

		// Do not call trackAccess here to avoid a positive feedback loop
		// where frequently recommended entries inflate their own frequency scores.
		return results.slice(0, maxResults);
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
