import {
	createEmbeddingError,
	createMemoryError,
	isEmbeddingError,
	toError,
} from '../../errors/index.js';
import type { EventBus } from '../../events/types.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import { parseQuery } from './query-dsl.js';
import type { StorageBackend } from './storage.js';
import type {
	AdvancedSearchResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateGroup,
	EmbeddingProvider,
	LearningProfile,
	MemoryConfig,
	MetadataFilter,
	RecommendationResult,
	RecommendOptions,
	SearchOptions,
	SearchResult,
	SummarizeOptions,
	SummarizeResult,
	TextGenerationProvider,
	TextSearchOptions,
	TextSearchResult,
	TopicInfo,
	VectorEntry,
} from './types.js';
import { createVectorStore, type VectorStoreOptions } from './vector-store.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface MemoryManagerOptions {
	/** Pluggable storage backend. Consumers must provide their own implementation. */
	storage: StorageBackend;
	/** Inject a custom logger. */
	logger?: Logger;
	/** Override vector store options (except storage and logger, which are set at this level). */
	vectorStoreOptions?: Omit<VectorStoreOptions, 'logger' | 'storage'>;
	/**
	 * Optional text generation provider used for summarization.
	 * Can also be set later via `setTextGenerator()`.
	 */
	textGenerator?: TextGenerationProvider;
	/** Optional event bus for publishing memory lifecycle events. */
	eventBus?: EventBus;
}

// ---------------------------------------------------------------------------
// MemoryManager interface
// ---------------------------------------------------------------------------

export interface MemoryManager {
	readonly initialize: () => Promise<void>;
	readonly dispose: () => Promise<void>;
	readonly add: (
		text: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly addBatch: (
		entries: Array<{ text: string; metadata?: Record<string, string> }>,
	) => Promise<string[]>;
	readonly search: (
		query: string,
		maxResults?: number,
		threshold?: number,
	) => Promise<SearchResult[]>;
	readonly textSearch: (options: TextSearchOptions) => TextSearchResult[];
	readonly filterByMetadata: (filters: MetadataFilter[]) => VectorEntry[];
	readonly filterByDateRange: (range: DateRange) => VectorEntry[];
	readonly advancedSearch: (
		options: SearchOptions,
	) => Promise<AdvancedSearchResult[]>;
	readonly query: (dsl: string) => Promise<AdvancedSearchResult[]>;
	readonly getById: (id: string) => VectorEntry | undefined;
	readonly getAll: () => VectorEntry[];
	readonly getTopics: () => TopicInfo[];
	readonly filterByTopic: (topics: string[]) => VectorEntry[];
	readonly recommend: (
		query: string,
		options?: Omit<RecommendOptions, 'queryEmbedding'>,
	) => Promise<RecommendationResult[]>;
	readonly findDuplicates: (threshold?: number) => DuplicateGroup[];
	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
	readonly summarize: (options: SummarizeOptions) => Promise<SummarizeResult>;
	readonly setTextGenerator: (provider: TextGenerationProvider) => void;
	/** Record explicit user feedback on whether an entry was relevant. */
	readonly recordFeedback: (entryId: string, relevant: boolean) => void;
	readonly delete: (id: string) => Promise<boolean>;
	readonly deleteBatch: (ids: string[]) => Promise<number>;
	readonly clear: () => Promise<void>;
	/** Snapshot of the adaptive learning profile, or undefined if learning is disabled. */
	readonly learningProfile: LearningProfile | undefined;
	readonly size: number;
	readonly isInitialized: boolean;
	readonly isDirty: boolean;
	readonly embeddingAgent: string | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a memory manager that wraps a vector store with automatic
 * embedding, search, deduplication, recommendation, and summarization.
 *
 * @param embedder - Provider that converts text into embedding vectors.
 * @param config - Memory settings (similarity threshold, max results, embedding agent).
 * @param options - Storage backend, logger, vector store options, optional text generator.
 * @returns A frozen {@link MemoryManager}. Call `initialize()` before use.
 * @throws {EmbeddingError} When the embedding provider fails during add/search.
 * @throws {MemoryError} When the store is not initialized or text is empty.
 */
export function createMemoryManager(
	embedder: EmbeddingProvider,
	config: MemoryConfig,
	options: MemoryManagerOptions,
): MemoryManager {
	const logger = (options.logger ?? getDefaultLogger()).child('memory');
	const eventBus = options.eventBus;
	const store = createVectorStore({
		storage: options.storage,
		logger,
		...(options.vectorStoreOptions ?? {}),
	});

	let initialized = false;
	let textGenerator: TextGenerationProvider | undefined =
		options?.textGenerator;

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	const ensureInitialized = (): void => {
		if (!initialized) {
			throw createMemoryError(
				'MemoryManager has not been initialized. Call initialize() first.',
				{ code: 'MEMORY_NOT_INITIALIZED' },
			);
		}
	};

	const getEmbedding = async (text: string): Promise<number[]> => {
		const result = await safeEmbed(text);
		const embedding = result.embeddings[0];

		if (!embedding || embedding.length === 0) {
			throw createEmbeddingError(
				'Embedding agent returned an empty embedding vector',
				{ model: config.embeddingAgent },
			);
		}

		return embedding;
	};

	const safeEmbed = async (
		input: string | string[],
	): Promise<{ embeddings: number[][] }> => {
		try {
			const result = await embedder.embed(input, config.embeddingAgent);
			return {
				embeddings: result.embeddings.map((e) => [...e]),
			};
		} catch (error) {
			if (isEmbeddingError(error)) throw error;

			throw createEmbeddingError(
				`Embedding request failed: ${toError(error).message}`,
				{
					cause: error,
					model: config.embeddingAgent,
				},
			);
		}
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	let initPromise: Promise<void> | null = null;

	const initialize = (): Promise<void> => {
		if (initialized) return Promise.resolve();
		if (initPromise) return initPromise;

		initPromise = (async () => {
			logger.debug('Initializing memory manager', {
				embeddingAgent: config.embeddingAgent,
			});

			await store.load();
			initialized = true;

			logger.info(`Memory manager initialized (${store.size} entries loaded)`);
		})().finally(() => {
			initPromise = null;
		});

		return initPromise;
	};

	const dispose = async (): Promise<void> => {
		logger.debug('Disposing memory manager');
		await store.dispose();
		initialized = false;
		logger.debug('Memory manager disposed');
	};

	// -----------------------------------------------------------------------
	// Write operations
	// -----------------------------------------------------------------------

	const add = async (
		text: string,
		metadata: Record<string, string> = {},
	): Promise<string> => {
		ensureInitialized();

		if (text.trim().length === 0) {
			throw createMemoryError(
				'Cannot add empty or whitespace-only text to memory',
				{
					code: 'MEMORY_EMPTY_TEXT',
				},
			);
		}

		logger.debug('Embedding and storing text', {
			textLength: text.length,
			embeddingAgent: config.embeddingAgent,
		});

		const embedding = await getEmbedding(text);
		const id = await store.add(text, embedding, metadata);

		logger.debug(`Stored memory entry "${id}"`, {
			embeddingDim: embedding.length,
			metadataKeys: Object.keys(metadata),
		});

		eventBus?.publish('memory.add', { id, contentLength: text.length });

		return id;
	};

	const addBatch = async (
		batchEntries: Array<{
			text: string;
			metadata?: Record<string, string>;
		}>,
	): Promise<string[]> => {
		ensureInitialized();

		if (batchEntries.length === 0) return [];

		for (let i = 0; i < batchEntries.length; i++) {
			if (batchEntries[i].text.trim().length === 0) {
				throw createMemoryError(
					`Cannot add empty or whitespace-only text to memory (batch index ${i})`,
					{ code: 'MEMORY_EMPTY_TEXT', metadata: { batchIndex: i } },
				);
			}
		}

		logger.debug(`Embedding batch of ${batchEntries.length} texts`, {
			embeddingAgent: config.embeddingAgent,
		});

		const texts = batchEntries.map((e) => e.text);
		const result = await safeEmbed(texts);

		if (result.embeddings.length < batchEntries.length) {
			throw createEmbeddingError(
				`Embedding agent returned ${result.embeddings.length} embeddings ` +
					`for ${batchEntries.length} inputs`,
				{ model: config.embeddingAgent },
			);
		}

		// Validate each individual embedding is non-empty
		for (let i = 0; i < batchEntries.length; i++) {
			if (!result.embeddings[i] || result.embeddings[i].length === 0) {
				throw createEmbeddingError(
					`Embedding agent returned an empty embedding vector at index ${i}`,
					{ model: config.embeddingAgent },
				);
			}
		}

		const storeBatch = batchEntries.map((entry, i) => ({
			text: entry.text,
			embedding: result.embeddings[i],
			metadata: entry.metadata ?? {},
		}));

		const ids = await store.addBatch(storeBatch);

		logger.debug(`Stored batch of ${ids.length} memory entries`);
		return ids;
	};

	// -----------------------------------------------------------------------
	// Vector Search (embedding-based)
	// -----------------------------------------------------------------------

	const searchFn = async (
		query: string,
		maxResults?: number,
		threshold?: number,
	): Promise<SearchResult[]> => {
		ensureInitialized();

		if (query.trim().length === 0) {
			logger.warn('Search called with empty query — returning no results');
			return [];
		}

		logger.debug('Searching memory', {
			queryLength: query.length,
			maxResults: maxResults ?? config.maxResults,
			threshold: threshold ?? config.similarityThreshold,
		});

		const start = Date.now();
		const queryEmbedding = await getEmbedding(query);

		const results = store.search(
			queryEmbedding,
			maxResults ?? config.maxResults,
			threshold ?? config.similarityThreshold,
		);

		const durationMs = Date.now() - start;
		logger.debug(`Found ${results.length} matching memories`);
		eventBus?.publish('memory.search', {
			query,
			resultCount: results.length,
			durationMs,
		});
		return results;
	};

	// -----------------------------------------------------------------------
	// Text Search (content-based, no embeddings)
	// -----------------------------------------------------------------------

	const textSearch = (searchOptions: TextSearchOptions): TextSearchResult[] => {
		ensureInitialized();

		if (searchOptions.query.trim().length === 0) {
			logger.warn('textSearch called with empty query — returning no results');
			return [];
		}

		logger.debug('Text searching memory', {
			query: searchOptions.query,
			mode: searchOptions.mode ?? 'fuzzy',
			threshold: searchOptions.threshold ?? 0.3,
		});

		const results = store.textSearch(searchOptions);

		logger.debug(`Text search found ${results.length} matching memories`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Metadata Filtering
	// -----------------------------------------------------------------------

	const filterByMetadata = (filters: MetadataFilter[]): VectorEntry[] => {
		ensureInitialized();

		logger.debug('Filtering memory by metadata', {
			filterCount: filters.length,
		});

		const results = store.filterByMetadata(filters);

		logger.debug(
			`Metadata filter returned ${results.length} matching memories`,
		);
		return results;
	};

	// -----------------------------------------------------------------------
	// Date Range Filtering
	// -----------------------------------------------------------------------

	const filterByDateRange = (range: DateRange): VectorEntry[] => {
		ensureInitialized();

		logger.debug('Filtering memory by date range', {
			after: range.after,
			before: range.before,
		});

		const results = store.filterByDateRange(range);

		logger.debug(
			`Date range filter returned ${results.length} matching memories`,
		);
		return results;
	};

	// -----------------------------------------------------------------------
	// Advanced / Combined Search
	// -----------------------------------------------------------------------

	const advancedSearch = async (
		searchOptions: SearchOptions,
	): Promise<AdvancedSearchResult[]> => {
		ensureInitialized();

		let resolvedOptions = searchOptions;
		if (!searchOptions.queryEmbedding && searchOptions.text?.query) {
			const trimmedQuery = searchOptions.text.query.trim();
			if (trimmedQuery.length > 0) {
				try {
					const queryEmbedding = await getEmbedding(trimmedQuery);
					resolvedOptions = { ...searchOptions, queryEmbedding };
				} catch {
					logger.debug(
						'Embedding failed for advancedSearch query — falling back to text-only',
					);
				}
			}
		}

		logger.debug('Advanced search on memory', {
			hasEmbedding: resolvedOptions.queryEmbedding !== undefined,
			hasText: resolvedOptions.text !== undefined,
			metadataFilterCount: resolvedOptions.metadata?.length ?? 0,
			hasDateRange: resolvedOptions.dateRange !== undefined,
			maxResults: resolvedOptions.maxResults ?? 10,
			rankBy: resolvedOptions.rankBy ?? 'average',
		});

		const results = store.advancedSearch(resolvedOptions);

		logger.debug(`Advanced search found ${results.length} matching memories`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Query DSL
	// -----------------------------------------------------------------------

	const queryDsl = async (dsl: string): Promise<AdvancedSearchResult[]> => {
		ensureInitialized();

		const parsed = parseQuery(dsl);

		logger.debug('Running DSL query', { dsl });

		const searchOptions: SearchOptions = {
			...(parsed.textSearch && parsed.textSearch.query.length > 0
				? {
						text: {
							query: parsed.textSearch.query,
							mode: parsed.textSearch.mode,
						},
					}
				: {}),
			...(parsed.metadataFilters && parsed.metadataFilters.length > 0
				? { metadata: parsed.metadataFilters }
				: {}),
			...(parsed.minScore !== undefined
				? { similarityThreshold: parsed.minScore }
				: {}),
		};

		// If the DSL has a text query, auto-embed it for vector search
		if (parsed.textSearch && parsed.textSearch.query.trim().length > 0) {
			try {
				const queryEmbedding = await getEmbedding(parsed.textSearch.query);
				(searchOptions as Record<string, unknown>).queryEmbedding =
					queryEmbedding;
			} catch {
				logger.debug(
					'Embedding failed for DSL query — falling back to text-only',
				);
			}
		}

		let results = store.advancedSearch(searchOptions);

		// Apply topic filter manually if present
		if (parsed.topicFilter && parsed.topicFilter.length > 0) {
			const topicEntries = store.filterByTopic([...parsed.topicFilter]);
			const topicIds = new Set(topicEntries.map((e) => e.id));
			results = results.filter((r) => topicIds.has(r.entry.id));
		}

		logger.debug(`DSL query returned ${results.length} results`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Accessors
	// -----------------------------------------------------------------------

	const getById = (id: string): VectorEntry | undefined => {
		ensureInitialized();
		return store.getById(id);
	};

	const getAll = (): VectorEntry[] => {
		ensureInitialized();
		return store.getAll();
	};

	const getTopics = (): TopicInfo[] => {
		ensureInitialized();
		return store.getTopics();
	};

	const filterByTopic = (topics: string[]): VectorEntry[] => {
		ensureInitialized();
		return store.filterByTopic(topics);
	};

	// -----------------------------------------------------------------------
	// Recommendation
	// -----------------------------------------------------------------------

	const recommend = async (
		query: string,
		recommendOptions?: Omit<RecommendOptions, 'queryEmbedding'>,
	): Promise<RecommendationResult[]> => {
		ensureInitialized();

		if (query.trim().length === 0) {
			logger.warn('recommend called with empty query — returning no results');
			return [];
		}

		logger.debug('Generating recommendations', {
			queryLength: query.length,
			maxResults: recommendOptions?.maxResults ?? 10,
		});

		const queryEmbedding = await getEmbedding(query);
		const results = store.recommend({
			...recommendOptions,
			queryEmbedding,
		});

		logger.debug(`Recommendations returned ${results.length} results`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Deduplication
	// -----------------------------------------------------------------------

	const findDuplicates = (threshold?: number): DuplicateGroup[] => {
		ensureInitialized();

		logger.debug('Finding duplicate entries', { threshold });
		const groups = store.findDuplicates(threshold);
		logger.debug(`Found ${groups.length} duplicate groups`);
		return groups;
	};

	const checkDuplicateFn = async (
		text: string,
	): Promise<DuplicateCheckResult> => {
		ensureInitialized();

		if (text.trim().length === 0) {
			return { isDuplicate: false };
		}

		const embedding = await getEmbedding(text);
		return store.checkDuplicate(embedding);
	};

	// -----------------------------------------------------------------------
	// Summarization
	// -----------------------------------------------------------------------

	const summarize = async (
		summarizeOptions: SummarizeOptions,
	): Promise<SummarizeResult> => {
		ensureInitialized();

		if (!textGenerator) {
			throw createMemoryError(
				'Summarization requires a textGenerator. Pass it in MemoryManagerOptions or call setTextGenerator().',
				{ code: 'MEMORY_NO_TEXT_GENERATOR' },
			);
		}

		if (summarizeOptions.ids.length < 2) {
			throw createMemoryError('Summarization requires at least 2 entry IDs', {
				code: 'MEMORY_SUMMARIZE_TOO_FEW',
			});
		}

		// Gather entry texts
		const sourceEntries: VectorEntry[] = [];
		for (const id of summarizeOptions.ids) {
			const entry = store.getById(id);
			if (!entry) {
				throw createMemoryError(`Entry "${id}" not found for summarization`, {
					code: 'MEMORY_ENTRY_NOT_FOUND',
				});
			}
			sourceEntries.push(entry);
		}

		const combinedText = sourceEntries
			.map((e, i) => `--- Entry ${i + 1} ---\n${e.text}`)
			.join('\n\n');

		const instruction =
			summarizeOptions.prompt ??
			'Summarize the following entries into a single concise summary that captures all key information:';

		const prompt = `${instruction}\n\n${combinedText}`;

		logger.debug('Generating summary', {
			entryCount: sourceEntries.length,
			promptLength: prompt.length,
		});

		const summaryText = await textGenerator.generate(
			prompt,
			summarizeOptions.systemPrompt,
		);

		// Embed and store the summary
		const summaryEmbedding = await getEmbedding(summaryText);
		const summaryMetadata: Record<string, string> = {
			...summarizeOptions.metadata,
			summarizedFrom: summarizeOptions.ids.join(','),
		};
		const summaryId = await store.add(
			summaryText,
			summaryEmbedding,
			summaryMetadata,
		);

		// Optionally delete originals
		const deleteOriginals = summarizeOptions.deleteOriginals ?? false;
		if (deleteOriginals) {
			await store.deleteBatch([...summarizeOptions.ids]);
		}

		logger.debug(`Created summary entry "${summaryId}"`, {
			deletedOriginals: deleteOriginals,
		});

		return {
			summaryId,
			summaryText,
			sourceIds: [...summarizeOptions.ids],
			deletedOriginals: deleteOriginals,
		};
	};

	// -----------------------------------------------------------------------
	// Text Generator
	// -----------------------------------------------------------------------

	const setTextGenerator = (provider: TextGenerationProvider): void => {
		textGenerator = provider;
		logger.debug('Text generation provider updated');
	};

	// -----------------------------------------------------------------------
	// Explicit Feedback
	// -----------------------------------------------------------------------

	const recordFeedback = (entryId: string, relevant: boolean): void => {
		ensureInitialized();

		const engine = store.learningEngine;
		if (!engine) {
			throw createMemoryError(
				'Cannot record feedback: adaptive learning is not enabled.',
				{ code: 'MEMORY_LEARNING_DISABLED' },
			);
		}

		engine.recordFeedback(entryId, relevant);
		logger.debug(
			`Recorded ${relevant ? 'positive' : 'negative'} feedback for "${entryId}"`,
		);
	};

	// -----------------------------------------------------------------------
	// Delete / clear
	// -----------------------------------------------------------------------

	const deleteEntry = async (id: string): Promise<boolean> => {
		ensureInitialized();
		const deleted = await store.delete(id);

		if (deleted) {
			logger.debug(`Deleted memory entry "${id}"`);
			eventBus?.publish('memory.delete', { id });
		} else {
			logger.debug(`Memory entry "${id}" not found for deletion`);
		}

		return deleted;
	};

	const deleteBatch = async (ids: string[]): Promise<number> => {
		ensureInitialized();
		const deleted = await store.deleteBatch(ids);
		logger.debug(`Deleted ${deleted} of ${ids.length} requested entries`);
		return deleted;
	};

	const clear = async (): Promise<void> => {
		ensureInitialized();
		await store.clear();
		logger.info('Memory store cleared');
	};

	// -----------------------------------------------------------------------
	// Return the record
	// -----------------------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		add,
		addBatch,
		search: searchFn,
		textSearch,
		filterByMetadata,
		filterByDateRange,
		advancedSearch,
		query: queryDsl,
		getById,
		getAll,
		getTopics,
		filterByTopic,
		recommend,
		findDuplicates,
		checkDuplicate: checkDuplicateFn,
		summarize,
		setTextGenerator,
		recordFeedback,
		delete: deleteEntry,
		deleteBatch,
		clear,
		get learningProfile() {
			return store.learningProfile;
		},
		get size() {
			return store.size;
		},
		get isInitialized() {
			return initialized;
		},
		get isDirty() {
			return store.isDirty;
		},
		get embeddingAgent() {
			return config.embeddingAgent;
		},
	});
}
