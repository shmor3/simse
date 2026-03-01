// ---------------------------------------------------------------------------
// Stacks — thin async client delegating to the Rust vector engine via JSON-RPC
// ---------------------------------------------------------------------------

import { createVectorClient, type VectorClient } from './client.js';
import { createNoopLogger, type Logger } from './logger.js';
import type {
	AdvancedLookup,
	DateRange,
	DuplicateCheckResult,
	DuplicateVolumes,
	LearningOptions,
	Lookup,
	MetadataFilter,
	PatronProfile,
	RecencyOptions,
	Recommendation,
	RecommendOptions,
	SearchOptions,
	TextLookup,
	TextSearchOptions,
	TopicInfo,
	Volume,
} from './types.js';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface StacksOptions {
	/** Path to the Rust engine binary. */
	readonly enginePath: string;
	/** Directory for Rust-side persistence. If provided, data persists across restarts. */
	readonly storagePath?: string;
	/** Cosine similarity threshold for duplicate detection. 0 to disable. */
	readonly duplicateThreshold?: number;
	/** Behavior on duplicate: 'skip' | 'warn' | 'error'. Defaults to 'warn'. */
	readonly duplicateBehavior?: 'skip' | 'warn' | 'error';
	/** Max regex pattern length (prevents ReDoS). Defaults to 256. */
	readonly maxRegexPatternLength?: number;
	/** Adaptive learning engine options. */
	readonly learning?: LearningOptions;
	/** Recency scoring options for recommendations. */
	readonly recency?: RecencyOptions;
	/** Logger. */
	readonly logger?: Logger;
}

// ---------------------------------------------------------------------------
// Stacks interface
// ---------------------------------------------------------------------------

export interface Stacks {
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
	) => Promise<Lookup[]>;
	readonly textSearch: (options: TextSearchOptions) => Promise<TextLookup[]>;
	readonly filterByMetadata: (
		filters: readonly MetadataFilter[],
	) => Promise<Volume[]>;
	readonly filterByDateRange: (range: DateRange) => Promise<Volume[]>;
	readonly advancedSearch: (
		options: SearchOptions,
	) => Promise<AdvancedLookup[]>;
	readonly getAll: () => Promise<Volume[]>;
	readonly getById: (id: string) => Promise<Volume | undefined>;
	readonly getTopics: () => Promise<TopicInfo[]>;
	readonly filterByTopic: (topics: readonly string[]) => Promise<Volume[]>;
	readonly findDuplicates: (threshold?: number) => Promise<DuplicateVolumes[]>;
	readonly checkDuplicate: (
		embedding: readonly number[],
	) => Promise<DuplicateCheckResult>;
	readonly recommend: (options?: RecommendOptions) => Promise<Recommendation[]>;
	/** Record explicit user feedback on whether a volume was relevant. */
	readonly recordFeedback: (
		entryId: string,
		relevant: boolean,
	) => Promise<void>;
	/** No longer exposed — the learning engine lives in Rust. */
	readonly learningEngine: undefined;
	/** Snapshot of the current patron learning profile. */
	readonly learningProfile: Promise<PatronProfile | undefined>;
	/** Number of volumes in the store. */
	readonly size: Promise<number>;
	/** Whether there are unsaved changes. */
	readonly isDirty: boolean;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a stacks store backed by the Rust vector engine.
 *
 * The Rust engine handles cosine-similarity search, text search, metadata
 * filtering, date-range filtering, duplicate detection, topic indexing,
 * recommendation scoring, and adaptive learning — all via JSON-RPC.
 *
 * @param options - Engine path (required), storage path, duplicate threshold,
 *   recency options, learning config, logger.
 * @returns A frozen {@link Stacks}. Call `load()` before use, `dispose()` when done.
 */
export function createStacks(options: StacksOptions): Stacks {
	const logger = options.logger ?? createNoopLogger();
	const client: VectorClient = createVectorClient({
		enginePath: options.enginePath,
		logger,
	});

	let dirty = false;

	// -------------------------------------------------------------------
	// Lifecycle
	// -------------------------------------------------------------------

	const load = async (): Promise<void> => {
		await client.request('store/initialize', {
			storagePath: options.storagePath,
			duplicateThreshold: options.duplicateThreshold,
			duplicateBehavior: options.duplicateBehavior,
			maxRegexPatternLength: options.maxRegexPatternLength,
			learningEnabled: options.learning?.enabled ?? true,
			recencyHalfLifeMs: options.recency?.halfLifeMs,
		});
	};

	const save = async (): Promise<void> => {
		await client.request('store/save');
		dirty = false;
	};

	const dispose = async (): Promise<void> => {
		await client.request('store/dispose').catch(() => {});
		await client.dispose();
	};

	// -------------------------------------------------------------------
	// CRUD
	// -------------------------------------------------------------------

	const add = async (
		text: string,
		embedding: readonly number[],
		metadata: Record<string, string> = {},
	): Promise<string> => {
		const result = await client.request<{ id: string }>('store/add', {
			text,
			embedding: [...embedding],
			metadata,
		});
		dirty = true;
		return result.id;
	};

	const addBatch = async (
		entries: ReadonlyArray<{
			text: string;
			embedding: readonly number[];
			metadata?: Record<string, string>;
		}>,
	): Promise<string[]> => {
		if (entries.length === 0) return [];
		const result = await client.request<{ ids: string[] }>('store/addBatch', {
			entries: entries.map((e) => ({
				text: e.text,
				embedding: [...e.embedding],
				metadata: e.metadata ?? {},
			})),
		});
		dirty = true;
		return result.ids;
	};

	const deleteVolume = async (id: string): Promise<boolean> => {
		const result = await client.request<{ deleted: boolean }>('store/delete', {
			id,
		});
		if (result.deleted) dirty = true;
		return result.deleted;
	};

	const deleteBatch = async (ids: readonly string[]): Promise<number> => {
		if (ids.length === 0) return 0;
		const result = await client.request<{ count: number }>(
			'store/deleteBatch',
			{ ids: [...ids] },
		);
		if (result.count > 0) dirty = true;
		return result.count;
	};

	const clear = async (): Promise<void> => {
		await client.request('store/clear');
		dirty = true;
	};

	// -------------------------------------------------------------------
	// Search
	// -------------------------------------------------------------------

	const search = async (
		queryEmbedding: readonly number[],
		maxResults: number,
		threshold: number,
	): Promise<Lookup[]> => {
		const result = await client.request<{ results: Lookup[] }>('store/search', {
			queryEmbedding: [...queryEmbedding],
			maxResults,
			threshold,
		});
		return result.results;
	};

	const textSearch = async (
		searchOptions: TextSearchOptions,
	): Promise<TextLookup[]> => {
		const result = await client.request<{ results: TextLookup[] }>(
			'store/textSearch',
			searchOptions,
		);
		return result.results;
	};

	const filterByMetadata = async (
		filters: readonly MetadataFilter[],
	): Promise<Volume[]> => {
		const result = await client.request<{ volumes: Volume[] }>(
			'store/filterByMetadata',
			{ filters: [...filters] },
		);
		return result.volumes;
	};

	const filterByDateRange = async (range: DateRange): Promise<Volume[]> => {
		const result = await client.request<{ volumes: Volume[] }>(
			'store/filterByDateRange',
			range,
		);
		return result.volumes;
	};

	const advancedSearch = async (
		searchOptions: SearchOptions,
	): Promise<AdvancedLookup[]> => {
		const params = { ...searchOptions };
		if (params.queryEmbedding) {
			(params as Record<string, unknown>).queryEmbedding = [
				...searchOptions.queryEmbedding!,
			];
		}
		const result = await client.request<{ results: AdvancedLookup[] }>(
			'store/advancedSearch',
			params,
		);
		return result.results;
	};

	// -------------------------------------------------------------------
	// Accessors
	// -------------------------------------------------------------------

	const getAll = async (): Promise<Volume[]> => {
		const result = await client.request<{ volumes: Volume[] }>('store/getAll');
		return result.volumes;
	};

	const getById = async (id: string): Promise<Volume | undefined> => {
		const result = await client.request<{ volume: Volume | null }>(
			'store/getById',
			{ id },
		);
		return result.volume ?? undefined;
	};

	const getTopics = async (): Promise<TopicInfo[]> => {
		const result = await client.request<{ topics: TopicInfo[] }>(
			'store/getTopics',
		);
		return result.topics;
	};

	const filterByTopic = async (
		topics: readonly string[],
	): Promise<Volume[]> => {
		const result = await client.request<{ volumes: Volume[] }>(
			'store/filterByTopic',
			{ topics: [...topics] },
		);
		return result.volumes;
	};

	// -------------------------------------------------------------------
	// Deduplication
	// -------------------------------------------------------------------

	const findDuplicates = async (
		threshold?: number,
	): Promise<DuplicateVolumes[]> => {
		const result = await client.request<{ groups: DuplicateVolumes[] }>(
			'store/findDuplicates',
			{ threshold },
		);
		return result.groups;
	};

	const checkDuplicate = async (
		embedding: readonly number[],
	): Promise<DuplicateCheckResult> => {
		return client.request<DuplicateCheckResult>('store/checkDuplicate', {
			embedding: [...embedding],
		});
	};

	// -------------------------------------------------------------------
	// Recommendation
	// -------------------------------------------------------------------

	const recommend = async (
		recommendOptions?: RecommendOptions,
	): Promise<Recommendation[]> => {
		const params = recommendOptions ? { ...recommendOptions } : {};
		if (recommendOptions?.queryEmbedding) {
			(params as Record<string, unknown>).queryEmbedding = [
				...recommendOptions.queryEmbedding,
			];
		}
		const result = await client.request<{ results: Recommendation[] }>(
			'store/recommend',
			params,
		);
		return result.results;
	};

	// -------------------------------------------------------------------
	// Feedback
	// -------------------------------------------------------------------

	const recordFeedback = async (
		entryId: string,
		relevant: boolean,
	): Promise<void> => {
		await client.request('learning/recordFeedback', { entryId, relevant });
	};

	// -------------------------------------------------------------------
	// Return frozen interface
	// -------------------------------------------------------------------

	return Object.freeze({
		load,
		save,
		dispose,
		add,
		addBatch,
		delete: deleteVolume,
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
		recordFeedback,
		get learningEngine() {
			return undefined;
		},
		get learningProfile(): Promise<PatronProfile | undefined> {
			return client
				.request<{ profile: PatronProfile | null }>('learning/profile')
				.then((r) => r.profile ?? undefined);
		},
		get size(): Promise<number> {
			return client
				.request<{ count: number }>('store/size')
				.then((r) => r.count);
		},
		get isDirty() {
			return dirty;
		},
	});
}
