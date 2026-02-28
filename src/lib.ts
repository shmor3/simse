// ---------------------------------------------------------------------------
// SimSE — public library API
//
// This module is the main entry point for consumers of the simse package.
// It re-exports every public function, type, and interface that users
// need to configure, build, and run chains programmatically.
// ---------------------------------------------------------------------------

// ---- Library / Vector Store (from simse-vector) ----------------------------
export type {
	// Stacks serialization
	AccessStats,
	// Types
	AdvancedLookup,
	ArbitrationResult,
	// Inverted index
	BM25Options,
	BM25Result,
	CirculationDesk,
	// Circulation desk
	CirculationDeskOptions,
	CirculationDeskThresholds,
	ClassificationResult,
	CompendiumOptions,
	CompendiumResult,
	// Preservation
	CompressionOptions,
	// Stacks persistence
	CorrelatedPair,
	CorrelationEntry,
	DateRange,
	DeserializedData,
	// Librarian registry
	DisposableConnection,
	DuplicateCheckResult,
	DuplicateVolumes,
	EmbeddingProvider,
	ExplicitFeedbackEntry,
	ExtractionMemory,
	ExtractionResult,
	FeedbackEntry,
	// Text search
	FuzzyScoreOptions,
	FuzzyScoreWeights,
	IndexEntry,
	IndexFile,
	InvertedIndex,
	// Patron learning
	LearningEngine,
	LearningOptions,
	LearningState,
	Librarian,
	LibrarianBid,
	LibrarianDefinition,
	LibrarianLibraryAccess,
	// Librarian
	LibrarianOptions,
	LibrarianRegistry,
	LibrarianRegistryOptions,
	// Library
	Library,
	LibraryConfig,
	// Library services
	LibraryContext,
	LibraryOptions,
	LibraryServices,
	LibraryServicesOptions,
	Lookup,
	// Cataloging
	MagnitudeCache,
	ManagedLibrarian,
	MetadataFilter,
	MetadataIndex,
	MetadataMatchMode,
	OptimizationResult,
	// Query DSL
	ParsedQuery,
	PatronProfile,
	// Prompt injection
	PromptInjectionOptions,
	QueryRecord,
	// Recommendation
	RecencyOptions,
	Recommendation,
	RecommendationScoreInput,
	RecommendationScoreResult,
	RecommendOptions,
	RelatedTopic,
	RelevanceFeedback,
	ReorganizationPlan,
	SearchOptions,
	SerializedData,
	SerializedQueryRecord,
	Shelf,
	// Stacks
	Stacks,
	StacksOptions,
	// Stacks search
	StacksSearchConfig,
	// Storage
	StorageBackend,
	// Text cache
	TextCache,
	TextCacheOptions,
	TextGenerationProvider,
	TextLookup,
	TextSearchMode,
	TextSearchOptions,
	TopicCatalog,
	// Topic catalog
	TopicCatalogOptions,
	TopicCatalogSection,
	TopicIndex,
	TopicIndexOptions,
	TopicInfo,
	TopicProfileEntry,
	TurnContext,
	// Librarian definition
	ValidationResult as LibrarianValidationResult,
	// Errors (vector-specific types)
	VectorError,
	VectorErrorOptions,
	Volume,
	WeightProfile,
} from 'simse-vector';
export {
	// Stacks search
	advancedStacksSearch,
	// Deduplication
	checkDuplicate,
	// Preservation
	compressText,
	// Cataloging
	computeMagnitude,
	// Recommendation
	computeRecommendationScore,
	// Stacks recommendation
	computeRecommendations,
	// Cosine similarity
	cosineSimilarity,
	// Circulation desk
	createCirculationDesk,
	// Librarian
	createDefaultLibrarian,
	// Errors (library errors from simse-vector)
	createEmbeddingError,
	// Inverted index
	createInvertedIndex,
	// Patron learning
	createLearningEngine,
	createLibrarian,
	// Librarian registry
	createLibrarianRegistry,
	// Library
	createLibrary,
	createLibraryError,
	// Library services
	createLibraryServices,
	createMagnitudeCache,
	createMetadataIndex,
	createNoopLogger,
	// Shelf
	createShelf,
	// Stacks
	createStacks,
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	// Text cache
	createTextCache,
	// Topic catalog
	createTopicCatalog,
	createTopicIndex,
	createVectorError,
	decodeEmbedding,
	decompressText,
	// Stacks serialization
	deserializeEntry,
	deserializeFromStorage,
	encodeEmbedding,
	filterVolumesByDateRange,
	filterVolumesByMetadata,
	findDuplicateVolumes,
	// Prompt injection
	formatMemoryContext,
	frequencyScore,
	// Text search
	fuzzyScore,
	isEmbeddingError,
	isGzipped,
	isLibraryError,
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
	// Stacks persistence
	isValidIndexEntry,
	isValidIndexFile,
	isValidLearningState,
	isVectorError,
	LEARNING_KEY,
	levenshteinDistance,
	levenshteinSimilarity,
	// Librarian definition
	loadAllDefinitions,
	loadDefinition,
	matchesAllMetadataFilters,
	matchesMetadataFilter,
	matchesTopic,
	ngramSimilarity,
	ngrams,
	normalizeWeights,
	// Query DSL
	parseQuery,
	recencyScore,
	saveDefinition,
	serializeEntry,
	serializeToStorage,
	stacksSearch,
	textSearchVolumes,
	tokenize,
	tokenizeForIndex,
	tokenOverlapScore,
	validateDefinition,
} from 'simse-vector';
// ---- Virtual Filesystem (from simse-vfs) ----------------------------------
export type {
	VFSCommitOperation,
	VFSCommitOptions,
	VFSCommitResult,
	VFSContentType,
	VFSCopyOptions,
	VFSDeleteOptions,
	VFSDiffHunk,
	VFSDiffLine,
	VFSDiffOptions,
	VFSDiffResult,
	VFSDirEntry,
	VFSDisk,
	VFSDiskOptions,
	VFSError,
	VFSErrorOptions,
	VFSHistoryEntry,
	VFSHistoryOptions,
	VFSLimits,
	VFSLoadOptions,
	VFSMkdirOptions,
	VFSNodeType,
	VFSReaddirOptions,
	VFSReadResult,
	VFSSearchOptions,
	VFSSearchResult,
	VFSSnapshot,
	VFSSnapshotDirectory,
	VFSSnapshotFile,
	VFSStat,
	VFSValidationIssue,
	VFSValidationResult,
	VFSValidator,
	VFSWriteEvent,
	VFSWriteOptions,
	VirtualFS,
	VirtualFSOptions,
} from 'simse-vfs';
export {
	ancestorPaths,
	baseName,
	createDefaultValidators,
	createEmptyFileValidator,
	createJSONSyntaxValidator,
	createMissingTrailingNewlineValidator,
	createMixedIndentationValidator,
	createMixedLineEndingsValidator,
	createTrailingWhitespaceValidator,
	createVFSDisk,
	createVFSError,
	createVirtualFS,
	isVFSError,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	VFS_ROOT,
	VFS_SCHEME,
	validatePath,
	validateSegment,
	validateSnapshot,
} from 'simse-vfs';
export type {
	ACPEmbedderOptions,
	ACPGeneratorOptions,
} from './ai/acp/acp-adapters.js';
export {
	createACPEmbedder,
	createACPGenerator,
} from './ai/acp/acp-adapters.js';
export type {
	ACPClient,
	ACPClientOptions,
	ACPStreamOptions,
} from './ai/acp/acp-client.js';
// ---- ACP (Agent Client Protocol) ------------------------------------------
export { createACPClient } from './ai/acp/acp-client.js';
export type {
	ACPConnection,
	ACPConnectionOptions,
	ACPPermissionOption,
	ACPPermissionRequestInfo,
	ACPPermissionToolCall,
} from './ai/acp/acp-connection.js';
export { createACPConnection } from './ai/acp/acp-connection.js';
export {
	extractToolCall,
	extractToolCallUpdate,
} from './ai/acp/acp-results.js';
export type {
	ACPAgentCapabilities,
	ACPAgentInfo,
	ACPChatMessage,
	ACPChatOptions,
	ACPClientCapabilities,
	ACPConfig,
	ACPContentBlock,
	ACPDataContent,
	ACPEmbedResult,
	ACPGenerateOptions,
	ACPGenerateResult,
	ACPInitializeResult,
	ACPMCPServerConfig,
	ACPModeInfo,
	ACPModelInfo,
	ACPModelsInfo,
	ACPModesInfo,
	ACPPermissionPolicy,
	ACPResourceContent,
	ACPResourceLinkContent,
	ACPSamplingParams,
	ACPServerEntry,
	ACPServerInfo,
	ACPServerStatus,
	ACPSessionInfo,
	ACPSessionListEntry,
	ACPSessionPromptResult,
	ACPStopReason,
	ACPStreamChunk,
	ACPStreamComplete,
	ACPStreamDelta,
	ACPStreamToolCall,
	ACPStreamToolCallUpdate,
	ACPTextContent,
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
	JsonRpcError,
	JsonRpcMessage,
	JsonRpcNotification,
	JsonRpcRequest,
	JsonRpcResponse,
} from './ai/acp/types.js';
// ---- Agent ----------------------------------------------------------------
export type {
	AgentExecutor,
	AgentExecutorOptions,
	AgentResult,
	AgentStepConfig,
	ParallelConfig,
	ParallelSubResult,
	ParallelSubStepConfig,
	SwarmMergeStrategy,
} from './ai/agent/index.js';
export { createAgentExecutor } from './ai/agent/index.js';
// ---- Chains ---------------------------------------------------------------
export type {
	Chain,
	ChainCallbacks,
	ChainOptions,
	ChainStepConfig,
	PromptTemplate,
	Provider,
	StepResult,
} from './ai/chain/index.js';
export {
	createChain,
	createChainFromDefinition,
	createPromptTemplate,
	formatSearchResults,
	isPromptTemplate,
	runNamedChain,
} from './ai/chain/index.js';
// ---- Conversation ---------------------------------------------------------
export type {
	ContextPruner,
	ContextPrunerOptions,
	Conversation,
	ConversationMessage,
	ConversationOptions,
	ConversationRole,
} from './ai/conversation/index.js';
export {
	createContextPruner,
	createConversation,
} from './ai/conversation/index.js';
// ---- Agentic Loop ---------------------------------------------------------
export type {
	AgenticLoop,
	AgenticLoopOptions,
	AgenticLoopResult,
	LoopCallbacks,
	LoopTurn,
	SubagentInfo,
	SubagentResult,
} from './ai/loop/index.js';
export { createAgenticLoop } from './ai/loop/index.js';
// ---- MCP (Model Context Protocol) ----------------------------------------
export type { MCPClient } from './ai/mcp/mcp-client.js';
export { createMCPClient } from './ai/mcp/mcp-client.js';
export type { MCPServerOptions, SimseMCPServer } from './ai/mcp/mcp-server.js';
export { createMCPServer } from './ai/mcp/mcp-server.js';
export type {
	MCPClientConfig,
	MCPCompletionRef,
	MCPCompletionRequest,
	MCPCompletionResult,
	MCPLoggingLevel,
	MCPLoggingMessage,
	MCPPromptInfo,
	MCPResourceInfo,
	MCPResourceSubscription,
	MCPResourceTemplateInfo,
	MCPRoot,
	MCPServerConfig,
	MCPServerConnection,
	MCPToolAnnotations,
	MCPToolCallMetrics,
	MCPToolInfo,
	MCPToolResult,
} from './ai/mcp/types.js';
// ---- Provider Prompts & System Prompt Builder -----------------------------
export type {
	AgentMode,
	DiscoveredInstruction,
	EnvironmentContext,
	InstructionDiscoveryOptions,
	ProviderPromptConfig,
	ProviderPromptResolver,
	SystemPromptBuildContext,
	SystemPromptBuilder,
	SystemPromptBuilderOptions,
} from './ai/prompts/index.js';
export {
	collectEnvironmentContext,
	createProviderPromptResolver,
	createSystemPromptBuilder,
	discoverInstructions,
} from './ai/prompts/index.js';
// ---- Task List ------------------------------------------------------------
export type {
	TaskCreateInput,
	TaskItem,
	TaskList,
	TaskListOptions,
	TaskStatus,
	TaskUpdateInput,
} from './ai/tasks/index.js';
export { createTaskList } from './ai/tasks/index.js';
// ---- Host Tools -----------------------------------------------------------
export type { BashToolOptions } from './ai/tools/host/bash.js';
export { registerBashTool } from './ai/tools/host/bash.js';
export type { FilesystemToolOptions } from './ai/tools/host/filesystem.js';
export { registerFilesystemTools } from './ai/tools/host/filesystem.js';
export type { FuzzyMatchResult } from './ai/tools/host/fuzzy-edit.js';
export { fuzzyMatch } from './ai/tools/host/fuzzy-edit.js';
export type { GitToolOptions } from './ai/tools/host/git.js';
export { registerGitTools } from './ai/tools/host/git.js';
// ---- Tool Registry --------------------------------------------------------
export type {
	BuiltinSubagentCallbacks,
	BuiltinSubagentOptions,
	DelegationCallbacks,
	DelegationInfo,
	DelegationResult,
	DelegationToolsOptions,
	RegisteredTool,
	SubagentCallbacks,
	SubagentToolsOptions,
	ToolAnnotations,
	ToolCallRequest,
	ToolCallResult,
	ToolCategory,
	ToolDefinition,
	ToolHandler,
	ToolMetrics,
	ToolParameter,
	ToolPermissionResolver,
	ToolRegistry,
	ToolRegistryOptions,
} from './ai/tools/index.js';
export {
	createToolRegistry,
	registerBuiltinSubagents,
	registerDelegationTools,
	registerLibraryTools,
	registerSubagentTools,
	registerTaskTools,
	registerVFSTools,
} from './ai/tools/index.js';
// ---- Permissions ----------------------------------------------------------
export type {
	ToolPermissionConfig,
	ToolPermissionPolicy,
	ToolPermissionRule,
} from './ai/tools/permissions.js';
export { createToolPermissionResolver } from './ai/tools/permissions.js';
// ---- Configuration --------------------------------------------------------
export type {
	ACPServerInput,
	AppConfig,
	ChainDefinition,
	ChainStepDefinition,
	DefineConfigOptions,
	ParallelConfigDefinition,
	ParallelSubStepDefinition,
	SimseConfig,
} from './config/settings.js';
export { defineConfig } from './config/settings.js';
// ---- Errors ---------------------------------------------------------------
export type { SimseError, SimseErrorOptions } from './errors/index.js';
export {
	// Chain
	createChainError,
	createChainNotFoundError,
	createChainStepError,
	// Resilience
	createCircuitBreakerOpenError,
	// Config
	createConfigError,
	createConfigNotFoundError,
	createConfigParseError,
	createConfigValidationError,
	// Loop
	createLoopAbortedError,
	createLoopError,
	createLoopTurnLimitError,
	createMCPConnectionError,
	// MCP
	createMCPError,
	createMCPServerNotConnectedError,
	createMCPToolError,
	createMCPTransportConfigError,
	// Provider
	createProviderError,
	createProviderGenerationError,
	createProviderHTTPError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	// Base
	createSimseError,
	// Tasks
	createTaskCircularDependencyError,
	createTaskError,
	createTaskNotFoundError,
	// Template
	createTemplateError,
	createTemplateMissingVariablesError,
	createTimeoutError,
	// Tools
	createToolError,
	createToolExecutionError,
	createToolNotFoundError,
	isChainError,
	isChainNotFoundError,
	isChainStepError,
	isCircuitBreakerOpenError,
	isConfigError,
	isConfigNotFoundError,
	isConfigParseError,
	isConfigValidationError,
	isLoopAbortedError,
	isLoopError,
	isLoopTurnLimitError,
	isMCPConnectionError,
	isMCPError,
	isMCPServerNotConnectedError,
	isMCPToolError,
	isMCPTransportConfigError,
	isProviderError,
	isProviderGenerationError,
	isProviderHTTPError,
	isProviderTimeoutError,
	isProviderUnavailableError,
	isSimseError,
	isTaskCircularDependencyError,
	isTaskError,
	isTaskNotFoundError,
	isTemplateError,
	isTemplateMissingVariablesError,
	isTimeoutError,
	isToolError,
	isToolExecutionError,
	isToolNotFoundError,
	toError,
	wrapError,
} from './errors/index.js';
// ---- Events ---------------------------------------------------------------
export type {
	EventBus,
	EventHandler,
	EventPayload,
	EventPayloadMap,
	EventType,
} from './events/index.js';
export { createEventBus } from './events/index.js';
// ---- Hooks ----------------------------------------------------------------
export type {
	BlockedResult,
	HookContextMap,
	HookHandler,
	HookResultMap,
	HookSystem,
	HookType,
} from './hooks/index.js';
export { createHookSystem } from './hooks/index.js';
// ---- Logger ---------------------------------------------------------------
export type {
	LogEntry,
	Logger,
	LoggerOptions,
	LogLevel,
	LogTransport,
	MemoryTransportHandle,
} from './logger.js';
export {
	createConsoleTransport,
	createLogger,
	createMemoryTransport,
	getDefaultLogger,
	setDefaultLogger,
} from './logger.js';
// ---- Server ---------------------------------------------------------------
export type {
	Session,
	SessionManager,
	SessionStatus,
	SimseServer,
	SimseServerConfig,
} from './server/index.js';
export {
	createSessionManager,
	createSimseServer,
} from './server/index.js';

// ---- Utilities ------------------------------------------------------------
export type {
	CircuitBreaker,
	CircuitBreakerOptions,
	CircuitBreakerState,
} from './utils/circuit-breaker.js';
export { createCircuitBreaker } from './utils/circuit-breaker.js';
export type {
	HealthMonitor,
	HealthMonitorOptions,
	HealthSnapshot,
	HealthStatus,
} from './utils/health-monitor.js';
export { createHealthMonitor } from './utils/health-monitor.js';
export type { RetryOptions } from './utils/retry.js';
export {
	createRetryAbortedError,
	createRetryExhaustedError,
	isRetryAbortedError,
	isRetryExhaustedError,
	isTransientError,
	retry,
	sleep,
} from './utils/retry.js';
export type { TimeoutOptions } from './utils/timeout.js';
export { withTimeout } from './utils/timeout.js';
