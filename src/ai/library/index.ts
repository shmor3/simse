// ---------------------------------------------------------------------------
// simse library — public API surface
// ---------------------------------------------------------------------------

// ---- Logger / EventBus -----------------------------------------------------
export type { EventBus, Logger } from '../shared/logger.js';
export { createNoopLogger } from '../shared/logger.js';
// ---- Circulation desk (async job queue) ------------------------------------
export type { CirculationDeskOptions } from './circulation-desk.js';
export { createCirculationDesk } from './circulation-desk.js';
// ---- Client (JSON-RPC to Rust engine) ------------------------------------
export type { VectorClient, VectorClientOptions } from './client.js';
export { createVectorClient } from './client.js';
// ---- Errors ----------------------------------------------------------------
export * from './errors.js';
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
// ---- Prompt injection (memory context formatting) --------------------------
export type { PromptInjectionOptions } from './prompt-injection.js';
export { formatMemoryContext } from './prompt-injection.js';
// ---- Query DSL -------------------------------------------------------------
export type { ParsedQuery } from './query-dsl.js';
export { parseQuery } from './query-dsl.js';
// ---- Shelf (agent-scoped partition) ----------------------------------------
export { createShelf } from './shelf.js';
// ---- Stacks (vector store) -------------------------------------------------
export type { Stacks, StacksOptions } from './stacks.js';
export { createStacks } from './stacks.js';
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
	GraphBoostOptions,
	GraphEdge,
	GraphEdgeOrigin,
	GraphEdgeType,
	GraphNeighbor,
	GraphTraversalNode,
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
	RecencyOptions,
	Recommendation,
	RecommendationScores,
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
