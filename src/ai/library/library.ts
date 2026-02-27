import {
	createEmbeddingError,
	createLibraryError,
	isEmbeddingError,
	toError,
} from '../../errors/index.js';
import type { EventBus } from '../../events/types.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import { parseQuery } from './query-dsl.js';
import { createShelf } from './shelf.js';
import { createStacks, type StacksOptions } from './stacks.js';
import type { StorageBackend } from './storage.js';
import type {
	AdvancedLookup,
	CirculationDeskThresholds,
	CompendiumOptions,
	CompendiumResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateVolumes,
	EmbeddingProvider,
	LibraryConfig,
	Lookup,
	MetadataFilter,
	PatronProfile,
	Recommendation,
	RecommendOptions,
	SearchOptions,
	Shelf,
	TextGenerationProvider,
	TextLookup,
	TextSearchOptions,
	TopicInfo,
	Volume,
} from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface LibraryOptions {
	/** Pluggable storage backend. Consumers must provide their own implementation. */
	storage: StorageBackend;
	/** Inject a custom logger. */
	logger?: Logger;
	/** Override stacks options (except storage and logger, which are set at this level). */
	stacksOptions?: Omit<StacksOptions, 'logger' | 'storage'>;
	/**
	 * Optional text generation provider used for compendium.
	 * Can also be set later via `setTextGenerator()`.
	 */
	textGenerator?: TextGenerationProvider;
	/** Optional event bus for publishing library lifecycle events. */
	eventBus?: EventBus;
	/** Thresholds for automatic compendium and reorganization via CirculationDesk. */
	readonly circulationDeskThresholds?: CirculationDeskThresholds;
}

// ---------------------------------------------------------------------------
// Library interface (was MemoryManager)
// ---------------------------------------------------------------------------

export interface Library {
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
	) => Promise<Lookup[]>;
	readonly textSearch: (options: TextSearchOptions) => TextLookup[];
	readonly filterByMetadata: (filters: MetadataFilter[]) => Volume[];
	readonly filterByDateRange: (range: DateRange) => Volume[];
	readonly advancedSearch: (
		options: SearchOptions,
	) => Promise<AdvancedLookup[]>;
	readonly query: (dsl: string) => Promise<AdvancedLookup[]>;
	readonly getById: (id: string) => Volume | undefined;
	readonly getAll: () => Volume[];
	readonly getTopics: () => TopicInfo[];
	readonly filterByTopic: (topics: string[]) => Volume[];
	readonly recommend: (
		query: string,
		options?: Omit<RecommendOptions, 'queryEmbedding'>,
	) => Promise<Recommendation[]>;
	readonly findDuplicates: (threshold?: number) => DuplicateVolumes[];
	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
	readonly compendium: (
		options: CompendiumOptions,
	) => Promise<CompendiumResult>;
	readonly setTextGenerator: (provider: TextGenerationProvider) => void;
	/** Record explicit user feedback on whether a volume was relevant. */
	readonly recordFeedback: (entryId: string, relevant: boolean) => void;
	readonly delete: (id: string) => Promise<boolean>;
	readonly deleteBatch: (ids: string[]) => Promise<number>;
	readonly clear: () => Promise<void>;
	/** Snapshot of the patron learning profile, or undefined if learning is disabled. */
	readonly patronProfile: PatronProfile | undefined;
	readonly size: number;
	readonly isInitialized: boolean;
	readonly isDirty: boolean;
	readonly embeddingAgent: string | undefined;
	readonly shelf: (name: string) => Shelf;
	readonly shelves: () => string[];
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a library that wraps a stacks store with automatic
 * embedding, search, deduplication, recommendation, and compendium.
 *
 * @param embedder - Provider that converts text into embedding vectors.
 * @param config - Library settings (similarity threshold, max results, embedding agent).
 * @param options - Storage backend, logger, stacks options, optional text generator.
 * @returns A frozen {@link Library}. Call `initialize()` before use.
 * @throws {EmbeddingError} When the embedding provider fails during add/search.
 * @throws {LibraryError} When the store is not initialized or text is empty.
 */
export function createLibrary(
	embedder: EmbeddingProvider,
	config: LibraryConfig,
	options: LibraryOptions,
): Library {
	const logger = (options.logger ?? getDefaultLogger()).child('library');
	const eventBus = options.eventBus;
	const store = createStacks({
		storage: options.storage,
		logger,
		...(options.stacksOptions ?? {}),
	});

	let initialized = false;
	let textGenerator: TextGenerationProvider | undefined =
		options?.textGenerator;

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	const ensureInitialized = (): void => {
		if (!initialized) {
			throw createLibraryError(
				'Library has not been initialized. Call initialize() first.',
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
			logger.debug('Initializing library', {
				embeddingAgent: config.embeddingAgent,
			});

			await store.load();
			initialized = true;

			logger.info(`Library initialized (${store.size} volumes loaded)`);
		})().finally(() => {
			initPromise = null;
		});

		return initPromise;
	};

	const dispose = async (): Promise<void> => {
		logger.debug('Disposing library');
		await store.dispose();
		shelfCache.clear();
		initialized = false;
		logger.debug('Library disposed');
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
			throw createLibraryError(
				'Cannot add empty or whitespace-only text to library',
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

		logger.debug(`Stored volume "${id}"`, {
			embeddingDim: embedding.length,
			metadataKeys: Object.keys(metadata),
		});

		eventBus?.publish('library.shelve', { id, contentLength: text.length });

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
				throw createLibraryError(
					`Cannot add empty or whitespace-only text to library (batch index ${i})`,
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

		logger.debug(`Stored batch of ${ids.length} volumes`);
		return ids;
	};

	// -----------------------------------------------------------------------
	// Vector Search (embedding-based)
	// -----------------------------------------------------------------------

	const searchFn = async (
		query: string,
		maxResults?: number,
		threshold?: number,
	): Promise<Lookup[]> => {
		ensureInitialized();

		if (query.trim().length === 0) {
			logger.warn('Search called with empty query — returning no results');
			return [];
		}

		logger.debug('Searching library', {
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
		logger.debug(`Found ${results.length} matching volumes`);
		eventBus?.publish('library.search', {
			query,
			resultCount: results.length,
			durationMs,
		});
		return results;
	};

	// -----------------------------------------------------------------------
	// Text Search (content-based, no embeddings)
	// -----------------------------------------------------------------------

	const textSearch = (searchOptions: TextSearchOptions): TextLookup[] => {
		ensureInitialized();

		if (searchOptions.query.trim().length === 0) {
			logger.warn('textSearch called with empty query — returning no results');
			return [];
		}

		logger.debug('Text searching library', {
			query: searchOptions.query,
			mode: searchOptions.mode ?? 'fuzzy',
			threshold: searchOptions.threshold ?? 0.3,
		});

		const results = store.textSearch(searchOptions);

		logger.debug(`Text search found ${results.length} matching volumes`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Metadata Filtering
	// -----------------------------------------------------------------------

	const filterByMetadata = (filters: MetadataFilter[]): Volume[] => {
		ensureInitialized();

		logger.debug('Filtering library by metadata', {
			filterCount: filters.length,
		});

		const results = store.filterByMetadata(filters);

		logger.debug(`Metadata filter returned ${results.length} matching volumes`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Date Range Filtering
	// -----------------------------------------------------------------------

	const filterByDateRange = (range: DateRange): Volume[] => {
		ensureInitialized();

		logger.debug('Filtering library by date range', {
			after: range.after,
			before: range.before,
		});

		const results = store.filterByDateRange(range);

		logger.debug(
			`Date range filter returned ${results.length} matching volumes`,
		);
		return results;
	};

	// -----------------------------------------------------------------------
	// Advanced / Combined Search
	// -----------------------------------------------------------------------

	const advancedSearch = async (
		searchOptions: SearchOptions,
	): Promise<AdvancedLookup[]> => {
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

		logger.debug('Advanced search on library', {
			hasEmbedding: resolvedOptions.queryEmbedding !== undefined,
			hasText: resolvedOptions.text !== undefined,
			metadataFilterCount: resolvedOptions.metadata?.length ?? 0,
			hasDateRange: resolvedOptions.dateRange !== undefined,
			maxResults: resolvedOptions.maxResults ?? 10,
			rankBy: resolvedOptions.rankBy ?? 'average',
		});

		const results = store.advancedSearch(resolvedOptions);

		logger.debug(`Advanced search found ${results.length} matching volumes`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Query DSL
	// -----------------------------------------------------------------------

	const queryDsl = async (dsl: string): Promise<AdvancedLookup[]> => {
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
			const topicVolumes = store.filterByTopic([...parsed.topicFilter]);
			const topicIds = new Set(topicVolumes.map((e) => e.id));
			results = results.filter((r) => topicIds.has(r.volume.id));
		}

		logger.debug(`DSL query returned ${results.length} results`);
		return results;
	};

	// -----------------------------------------------------------------------
	// Accessors
	// -----------------------------------------------------------------------

	const getById = (id: string): Volume | undefined => {
		ensureInitialized();
		return store.getById(id);
	};

	const getAll = (): Volume[] => {
		ensureInitialized();
		return store.getAll();
	};

	const getTopics = (): TopicInfo[] => {
		ensureInitialized();
		return store.getTopics();
	};

	const filterByTopic = (topics: string[]): Volume[] => {
		ensureInitialized();
		return store.filterByTopic(topics);
	};

	// -----------------------------------------------------------------------
	// Recommendation
	// -----------------------------------------------------------------------

	const recommend = async (
		query: string,
		recommendOptions?: Omit<RecommendOptions, 'queryEmbedding'>,
	): Promise<Recommendation[]> => {
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

	const findDuplicates = (threshold?: number): DuplicateVolumes[] => {
		ensureInitialized();

		logger.debug('Finding duplicate volumes', { threshold });
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
	// Compendium (was Summarization)
	// -----------------------------------------------------------------------

	const compendium = async (
		compendiumOptions: CompendiumOptions,
	): Promise<CompendiumResult> => {
		ensureInitialized();

		if (!textGenerator) {
			throw createLibraryError(
				'Compendium requires a textGenerator. Pass it in LibraryOptions or call setTextGenerator().',
				{ code: 'MEMORY_NO_TEXT_GENERATOR' },
			);
		}

		if (compendiumOptions.ids.length < 2) {
			throw createLibraryError('Compendium requires at least 2 volume IDs', {
				code: 'MEMORY_SUMMARIZE_TOO_FEW',
			});
		}

		// Gather volume texts
		const sourceVolumes: Volume[] = [];
		for (const id of compendiumOptions.ids) {
			const vol = store.getById(id);
			if (!vol) {
				throw createLibraryError(`Volume "${id}" not found for compendium`, {
					code: 'MEMORY_ENTRY_NOT_FOUND',
				});
			}
			sourceVolumes.push(vol);
		}

		const combinedText = sourceVolumes
			.map((e, i) => `--- Volume ${i + 1} ---\n${e.text}`)
			.join('\n\n');

		const instruction =
			compendiumOptions.prompt ??
			'Summarize the following volumes into a single concise summary that captures all key information:';

		const prompt = `${instruction}\n\n${combinedText}`;

		logger.debug('Generating compendium', {
			volumeCount: sourceVolumes.length,
			promptLength: prompt.length,
		});

		const compendiumText = await textGenerator.generate(
			prompt,
			compendiumOptions.systemPrompt,
		);

		// Embed and store the compendium
		const compendiumEmbedding = await getEmbedding(compendiumText);
		const compendiumMetadata: Record<string, string> = {
			...compendiumOptions.metadata,
			summarizedFrom: compendiumOptions.ids.join(','),
		};
		const compendiumId = await store.add(
			compendiumText,
			compendiumEmbedding,
			compendiumMetadata,
		);

		// Optionally delete originals
		const deleteOriginals = compendiumOptions.deleteOriginals ?? false;
		if (deleteOriginals) {
			await store.deleteBatch([...compendiumOptions.ids]);
		}

		logger.debug(`Created compendium volume "${compendiumId}"`, {
			deletedOriginals: deleteOriginals,
		});

		return {
			compendiumId,
			text: compendiumText,
			sourceIds: [...compendiumOptions.ids],
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
			throw createLibraryError(
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
			logger.debug(`Deleted volume "${id}"`);
			eventBus?.publish('library.withdraw', { id });
		} else {
			logger.debug(`Volume "${id}" not found for deletion`);
		}

		return deleted;
	};

	const deleteBatch = async (ids: string[]): Promise<number> => {
		ensureInitialized();
		const deleted = await store.deleteBatch(ids);
		logger.debug(`Deleted ${deleted} of ${ids.length} requested volumes`);
		return deleted;
	};

	const clear = async (): Promise<void> => {
		ensureInitialized();
		await store.clear();
		shelfCache.clear();
		logger.info('Library store cleared');
	};

	// -----------------------------------------------------------------------
	// Shelf (agent-scoped partitions)
	// -----------------------------------------------------------------------

	const shelfCache = new Map<string, Shelf>();

	const shelf = (name: string): Shelf => {
		ensureInitialized();
		let s = shelfCache.get(name);
		if (!s) {
			s = createShelf(name, manager);
			shelfCache.set(name, s);
		}
		return s;
	};

	const shelves = (): string[] => {
		ensureInitialized();
		const names = new Set<string>();
		for (const vol of store.getAll()) {
			if (vol.metadata.shelf) {
				names.add(vol.metadata.shelf);
			}
		}
		return [...names];
	};

	// -----------------------------------------------------------------------
	// Return the record
	// -----------------------------------------------------------------------

	const manager: Library = Object.freeze({
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
		compendium,
		setTextGenerator,
		recordFeedback,
		delete: deleteEntry,
		deleteBatch,
		clear,
		shelf,
		shelves,
		get patronProfile() {
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
	return manager;
}
