// ---------------------------------------------------------------------------
// simse-vector — public API surface
// ---------------------------------------------------------------------------

// ---- Cataloging (topic, metadata, magnitude) -------------------------------
export type {
	MagnitudeCache,
	MetadataIndex,
	TopicIndex,
	TopicIndexOptions,
} from './cataloging.js';
export {
	computeMagnitude,
	createMagnitudeCache,
	createMetadataIndex,
	createTopicIndex,
} from './cataloging.js';
// ---- Circulation desk (async job queue) ------------------------------------
export type { CirculationDeskOptions } from './circulation-desk.js';
export { createCirculationDesk } from './circulation-desk.js';
// ---- Cosine similarity -----------------------------------------------------
export { cosineSimilarity } from './cosine.js';
// ---- Deduplication ---------------------------------------------------------
export { checkDuplicate, findDuplicateVolumes } from './deduplication.js';
// ---- Errors ----------------------------------------------------------------
export * from './errors.js';
// ---- Inverted index (BM25) ------------------------------------------------
export type {
	BM25Options,
	BM25Result,
	InvertedIndex,
} from './inverted-index.js';
export { createInvertedIndex, tokenizeForIndex } from './inverted-index.js';
// ---- Librarian (LLM-driven extraction, summarization) ---------------------
export type { LibrarianOptions } from './librarian.js';
export { createDefaultLibrarian, createLibrarian } from './librarian.js';
// ---- Librarian definition (validation, persistence) -----------------------
export type { ValidationResult } from './librarian-definition.js';
export {
	loadAllDefinitions,
	loadDefinition,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from './librarian-definition.js';
// ---- Librarian registry (multi-librarian management) ----------------------
export type {
	DisposableConnection,
	LibrarianRegistry,
	LibrarianRegistryOptions,
	ManagedLibrarian,
} from './librarian-registry.js';
export { createLibrarianRegistry } from './librarian-registry.js';
// ---- Library (high-level API) ----------------------------------------------
export type { Library, LibraryOptions } from './library.js';
export { createLibrary } from './library.js';
// ---- Library services (memory middleware) ----------------------------------
export type {
	LibraryContext,
	LibraryServices,
	LibraryServicesOptions,
} from './library-services.js';
export { createLibraryServices } from './library-services.js';
// ---- Logger / EventBus -----------------------------------------------------
export type { EventBus, Logger } from './logger.js';
export { createNoopLogger } from './logger.js';
// ---- Patron learning (adaptive engine) -------------------------------------
export type { LearningEngine } from './patron-learning.js';
export { createLearningEngine } from './patron-learning.js';
// ---- Preservation (embedding encode/decode, gzip) --------------------------
export type { CompressionOptions } from './preservation.js';
export {
	compressText,
	decodeEmbedding,
	decompressText,
	encodeEmbedding,
	isGzipped,
} from './preservation.js';
// ---- Prompt injection (memory context formatting) --------------------------
export type { PromptInjectionOptions } from './prompt-injection.js';
export { formatMemoryContext } from './prompt-injection.js';
// ---- Query DSL -------------------------------------------------------------
export type { ParsedQuery } from './query-dsl.js';
export { parseQuery } from './query-dsl.js';

// ---- Recommendation --------------------------------------------------------
export type {
	RecencyOptions,
	RecommendationScoreInput,
	RecommendationScoreResult,
} from './recommendation.js';
export {
	computeRecommendationScore,
	frequencyScore,
	normalizeWeights,
	recencyScore,
} from './recommendation.js';
// ---- Shelf (agent-scoped partition) ----------------------------------------
export { createShelf } from './shelf.js';
// ---- Stacks (vector store) -------------------------------------------------
export type { Stacks, StacksOptions } from './stacks.js';
export { createStacks } from './stacks.js';
// ---- Stacks persistence (index types + validators) -------------------------
export type {
	CorrelatedPair,
	CorrelationEntry,
	ExplicitFeedbackEntry,
	FeedbackEntry,
	IndexEntry,
	IndexFile,
	LearningState,
	SerializedQueryRecord,
	TopicProfileEntry,
} from './stacks-persistence.js';
export {
	isValidIndexEntry,
	isValidIndexFile,
	isValidLearningState,
} from './stacks-persistence.js';
// ---- Stacks recommendation -------------------------------------------------
export { computeRecommendations } from './stacks-recommend.js';
// ---- Stacks search ---------------------------------------------------------
export type { StacksSearchConfig } from './stacks-search.js';
export {
	advancedStacksSearch,
	filterVolumesByDateRange,
	filterVolumesByMetadata,
	stacksSearch,
	textSearchVolumes,
} from './stacks-search.js';

// ---- Stacks serialization --------------------------------------------------
export type {
	AccessStats,
	DeserializedData,
	SerializedData,
} from './stacks-serialize.js';
export {
	deserializeEntry,
	deserializeFromStorage,
	LEARNING_KEY,
	serializeEntry,
	serializeToStorage,
} from './stacks-serialize.js';
// ---- Storage backend -------------------------------------------------------
export type { StorageBackend } from './storage.js';
// ---- Text cache ------------------------------------------------------------
export type { TextCache, TextCacheOptions } from './text-cache.js';
export { createTextCache } from './text-cache.js';
// ---- Text search -----------------------------------------------------------
export type { FuzzyScoreOptions, FuzzyScoreWeights } from './text-search.js';
export {
	fuzzyScore,
	levenshteinDistance,
	levenshteinSimilarity,
	matchesAllMetadataFilters,
	matchesMetadataFilter,
	ngramSimilarity,
	ngrams,
	tokenize,
	tokenOverlapScore,
} from './text-search.js';
// ---- Topic catalog ---------------------------------------------------------
export type { TopicCatalogOptions } from './topic-catalog.js';
export { createTopicCatalog } from './topic-catalog.js';
// ---- Types -----------------------------------------------------------------
export type {
	AdvancedLookup,
	ArbitrationResult,
	CirculationDesk,
	CirculationDeskThresholds,
	ClassificationResult,
	CompendiumOptions,
	CompendiumResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateVolumes,
	EmbeddingProvider,
	ExtractionMemory,
	ExtractionResult,
	LearningOptions,
	Librarian,
	LibrarianBid,
	LibrarianDefinition,
	LibrarianLibraryAccess,
	LibraryConfig,
	Lookup,
	MetadataFilter,
	MetadataMatchMode,
	OptimizationResult,
	PatronProfile,
	QueryRecord,
	Recommendation,
	RecommendOptions,
	RelatedTopic,
	RelevanceFeedback,
	ReorganizationPlan,
	SearchOptions,
	Shelf,
	TextGenerationProvider,
	TextLookup,
	TextSearchMode,
	TextSearchOptions,
	TopicCatalog,
	TopicCatalogSection,
	TopicInfo,
	TurnContext,
	Volume,
	WeightProfile,
} from './types.js';
