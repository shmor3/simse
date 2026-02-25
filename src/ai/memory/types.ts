// ---------------------------------------------------------------------------
// Memory / Vector Store Types
// ---------------------------------------------------------------------------
//
// All types are strictly readonly to enforce immutability throughout
// the codebase.  No classes — only plain data interfaces.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Embedding Provider
// ---------------------------------------------------------------------------

export interface EmbeddingProvider {
	readonly embed: (
		input: string | readonly string[],
		model?: string,
	) => Promise<{ readonly embeddings: ReadonlyArray<readonly number[]> }>;
}

// ---------------------------------------------------------------------------
// Vector Store Types
// ---------------------------------------------------------------------------

export interface VectorEntry {
	readonly id: string;
	readonly text: string;
	readonly embedding: readonly number[];
	readonly metadata: Readonly<Record<string, string>>;
	readonly timestamp: number;
}

export interface SearchResult {
	readonly entry: VectorEntry;
	readonly score: number;
}

// ---------------------------------------------------------------------------
// Text Search
// ---------------------------------------------------------------------------

/**
 * How the text query is matched against entry text.
 *
 * - `"fuzzy"` — Levenshtein / n-gram similarity (tolerates typos).
 * - `"substring"` — Case-insensitive substring containment.
 * - `"exact"` — Case-sensitive exact equality.
 * - `"regex"` — Match against a regular expression pattern.
 * - `"token"` — Tokenised word overlap (bag-of-words similarity).
 * - `"bm25"` — BM25 ranking via inverted term index.
 */
export type TextSearchMode =
	| 'fuzzy'
	| 'substring'
	| 'exact'
	| 'regex'
	| 'token'
	| 'bm25';

export interface TextSearchOptions {
	/** The query string (or regex pattern when mode is `"regex"`). */
	readonly query: string;
	/** Matching strategy. Defaults to `"fuzzy"`. */
	readonly mode?: TextSearchMode;
	/**
	 * Minimum similarity score to include in results (0–1).
	 * Only meaningful for `"fuzzy"` and `"token"` modes — the other modes
	 * are binary (match / no-match) and always return a score of 1 for hits.
	 * Defaults to `0.3`.
	 */
	readonly threshold?: number;
}

export interface TextSearchResult {
	readonly entry: VectorEntry;
	/** Relevance score between 0 and 1. */
	readonly score: number;
}

// ---------------------------------------------------------------------------
// Metadata Filtering
// ---------------------------------------------------------------------------

/**
 * How a metadata value is compared.
 *
 * - `"eq"` — Exact equality (default).
 * - `"neq"` — Not equal.
 * - `"contains"` — Value contains the filter string (case-insensitive).
 * - `"startsWith"` — Value starts with the filter string (case-insensitive).
 * - `"endsWith"` — Value ends with the filter string (case-insensitive).
 * - `"regex"` — Value matches a regular expression.
 * - `"exists"` — Key is present (value is ignored).
 * - `"notExists"` — Key is absent (value is ignored).
 * - `"gt"` — Numeric greater-than comparison.
 * - `"gte"` — Numeric greater-than-or-equal comparison.
 * - `"lt"` — Numeric less-than comparison.
 * - `"lte"` — Numeric less-than-or-equal comparison.
 * - `"in"` — Value is one of the strings in the array.
 * - `"notIn"` — Value is not one of the strings in the array.
 * - `"between"` — Numeric value is within a [min, max] range (inclusive).
 */
export type MetadataMatchMode =
	| 'eq'
	| 'neq'
	| 'contains'
	| 'startsWith'
	| 'endsWith'
	| 'regex'
	| 'exists'
	| 'notExists'
	| 'gt'
	| 'gte'
	| 'lt'
	| 'lte'
	| 'in'
	| 'notIn'
	| 'between';

export interface MetadataFilter {
	/** The metadata key to match on. */
	readonly key: string;
	/** The value to compare against (ignored for `"exists"` / `"notExists"`). Array form used by `"in"`, `"notIn"`, and `"between"`. */
	readonly value?: string | readonly string[];
	/** Comparison mode. Defaults to `"eq"`. */
	readonly mode?: MetadataMatchMode;
}

// ---------------------------------------------------------------------------
// Date Range Filtering
// ---------------------------------------------------------------------------

export interface DateRange {
	/** Inclusive lower bound (epoch milliseconds). */
	readonly after?: number;
	/** Inclusive upper bound (epoch milliseconds). */
	readonly before?: number;
}

// ---------------------------------------------------------------------------
// Advanced / Combined Search Options
// ---------------------------------------------------------------------------

export interface SearchOptions {
	// -- Vector similarity ------------------------------------------------
	/**
	 * Query embedding for cosine-similarity ranking.
	 * When omitted the search is purely text / metadata / date based.
	 */
	readonly queryEmbedding?: readonly number[];
	/** Minimum cosine similarity (0–1). Defaults to `0`. */
	readonly similarityThreshold?: number;

	// -- Text search ------------------------------------------------------
	/** Optional text-level search applied *in addition* to vector search. */
	readonly text?: TextSearchOptions;

	// -- Metadata filters -------------------------------------------------
	/**
	 * One or more metadata filters. All filters must match for an entry to
	 * be included (logical AND).
	 */
	readonly metadata?: readonly MetadataFilter[];

	// -- Date range -------------------------------------------------------
	/** Restrict results to entries within a timestamp window. */
	readonly dateRange?: DateRange;

	// -- Pagination / limits ---------------------------------------------
	/** Maximum number of results to return. Defaults to `10`. */
	readonly maxResults?: number;

	/**
	 * How vector and text scores are combined when both are present.
	 *
	 * - `"vector"` — Rank by vector similarity only (text is a filter).
	 * - `"text"` — Rank by text relevance only (vector is a filter).
	 * - `"average"` — Arithmetic mean of both scores.
	 * - `"multiply"` — Product of both scores (boosts entries that rank
	 *   highly on *both* axes).
	 * - `"weighted"` — Combine using explicit `rankWeights` for each component.
	 *
	 * Defaults to `"average"`.
	 */
	readonly rankBy?: 'vector' | 'text' | 'average' | 'multiply' | 'weighted';

	// -- Field boosting ---------------------------------------------------
	/**
	 * Multipliers applied to individual score components before combining.
	 *
	 * - `text` — Scales the text relevance score (default 1.0).
	 * - `metadata` — Bonus added when an entry passes metadata filters (default 1.0).
	 * - `topic` — Bonus added when an entry matches a topic filter (default 1.0).
	 */
	readonly fieldBoosts?: {
		readonly text?: number;
		readonly metadata?: number;
		readonly topic?: number;
	};

	/**
	 * Weights for the `"weighted"` ranking mode. Each weight controls
	 * the contribution of its corresponding score component.
	 *
	 * Defaults: `{ vector: 0.5, text: 0.3, metadata: 0.1, recency: 0.1 }`.
	 */
	readonly rankWeights?: {
		readonly vector?: number;
		readonly text?: number;
		readonly metadata?: number;
		readonly recency?: number;
	};

	// -- Topic filter (for boosting) --------------------------------------
	/**
	 * Topics to match for topic-based field boosting.
	 * Entries whose `metadata.topic` matches any of these topics receive the
	 * topic boost defined in `fieldBoosts.topic`.
	 */
	readonly topicFilter?: readonly string[];
}

export interface AdvancedSearchResult {
	readonly entry: VectorEntry;
	/** Final combined score used for ranking (0–1). */
	readonly score: number;
	/** Individual score components, present when the corresponding search
	 *  dimension was requested. */
	readonly scores: {
		readonly vector?: number;
		readonly text?: number;
	};
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

export interface DuplicateCheckResult {
	readonly isDuplicate: boolean;
	readonly existingEntry?: VectorEntry;
	readonly similarity?: number;
}

export interface DuplicateGroup {
	readonly representative: VectorEntry;
	readonly duplicates: readonly VectorEntry[];
	readonly averageSimilarity: number;
}

// ---------------------------------------------------------------------------
// Topic Info
// ---------------------------------------------------------------------------

export interface RelatedTopic {
	readonly topic: string;
	readonly coOccurrenceCount: number;
}

export interface TopicInfo {
	readonly topic: string;
	readonly entryCount: number;
	readonly entryIds: readonly string[];
	readonly parent?: string;
	readonly children: readonly string[];
}

// ---------------------------------------------------------------------------
// Recommendation
// ---------------------------------------------------------------------------

export interface WeightProfile {
	/** Weight for vector similarity score. Defaults to `0.6`. */
	readonly vector?: number;
	/** Weight for recency score. Defaults to `0.2`. */
	readonly recency?: number;
	/** Weight for frequency/access count score. Defaults to `0.2`. */
	readonly frequency?: number;
}

export interface RecommendOptions {
	/** Query embedding for vector similarity scoring. */
	readonly queryEmbedding?: readonly number[];
	/** Weight profile for combining scores. */
	readonly weights?: WeightProfile;
	/** Maximum number of results. Defaults to `10`. */
	readonly maxResults?: number;
	/** Minimum combined score to include in results (0–1). Defaults to `0`. */
	readonly minScore?: number;
	/** Metadata filters to pre-filter candidates. */
	readonly metadata?: readonly MetadataFilter[];
	/** Topic filter — only entries matching any of these topics. */
	readonly topics?: readonly string[];
	/** Date range filter. */
	readonly dateRange?: DateRange;
}

export interface RecommendationResult {
	readonly entry: VectorEntry;
	readonly score: number;
	readonly scores: {
		readonly vector?: number;
		readonly recency?: number;
		readonly frequency?: number;
	};
}

// ---------------------------------------------------------------------------
// Summarization
// ---------------------------------------------------------------------------

export interface TextGenerationProvider {
	readonly generate: (prompt: string, systemPrompt?: string) => Promise<string>;
}

export interface SummarizeOptions {
	/** IDs of entries to summarize (minimum 2). */
	readonly ids: readonly string[];
	/**
	 * Custom instruction prompt for the summarization.
	 * The combined entry texts are always appended after this prompt.
	 * When omitted a sensible default instruction is used.
	 */
	readonly prompt?: string;
	/** Optional system prompt passed to the text generation provider. */
	readonly systemPrompt?: string;
	/** If true, delete the original entries after summarization. Defaults to `false`. */
	readonly deleteOriginals?: boolean;
	/** Additional metadata to attach to the summary entry. */
	readonly metadata?: Readonly<Record<string, string>>;
}

export interface SummarizeResult {
	readonly summaryId: string;
	readonly summaryText: string;
	readonly sourceIds: readonly string[];
	readonly deletedOriginals: boolean;
}

// ---------------------------------------------------------------------------
// Learning / Adaptive Memory
// ---------------------------------------------------------------------------

/** Per-entry feedback tracking how many unique queries retrieve this entry. */
export interface RelevanceFeedback {
	/** Number of unique query embeddings that retrieved this entry. */
	readonly queryCount: number;
	/** Total times this entry was returned across all queries. */
	readonly totalRetrievals: number;
	/** Epoch ms when this entry was last returned by a query. */
	readonly lastQueryTimestamp: number;
	/** Computed diversity score (0–1). Higher = retrieved by many diverse queries. */
	readonly relevanceScore: number;
}

/** Snapshot of the learning engine's full state. */
export interface LearningProfile {
	readonly queryHistory: readonly QueryRecord[];
	readonly adaptedWeights: Readonly<Required<WeightProfile>>;
	readonly interestEmbedding: readonly number[] | undefined;
	readonly totalQueries: number;
	readonly lastUpdated: number;
}

/** Record of a single query for interest profiling. */
export interface QueryRecord {
	readonly embedding: readonly number[];
	readonly timestamp: number;
	readonly resultCount: number;
}

/** Configuration for the adaptive learning engine. */
export interface LearningOptions {
	/** Whether adaptive learning is enabled. Defaults to `true`. */
	readonly enabled?: boolean;
	/** Maximum number of query records retained. Defaults to `50`. */
	readonly maxQueryHistory?: number;
	/** Decay half-life in ms for interest profile weighting. Defaults to 7 days. */
	readonly queryDecayMs?: number;
	/** How fast weight profile adapts per query (0–1). Defaults to `0.05`. */
	readonly weightAdaptationRate?: number;
	/** Influence of interest embedding on recommendation boost (0–1). Defaults to `0.15`. */
	readonly interestBoostWeight?: number;
	/** Whether to persist learning state to disk. Defaults to `true`. */
	readonly feedbackPersistence?: boolean;
}

// ---------------------------------------------------------------------------
// Memory Config
// ---------------------------------------------------------------------------

export interface MemoryConfig {
	readonly enabled: boolean;
	/** ACP agent ID used for generating embeddings. */
	readonly embeddingAgent?: string;
	readonly similarityThreshold: number;
	readonly maxResults: number;
}
