// ---------------------------------------------------------------------------
// SimSE — public library API
//
// This module is the main entry point for consumers of the simse package.
// It re-exports every public function, type, and interface that users
// need to configure, build, and run chains programmatically.
// ---------------------------------------------------------------------------

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
export type { LocalEmbedderOptions } from './ai/acp/local-embedder.js';
export { createLocalEmbedder } from './ai/acp/local-embedder.js';
export type { TEIEmbedderOptions } from './ai/acp/tei-bridge.js';
export { createTEIEmbedder } from './ai/acp/tei-bridge.js';
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
// ---- Library (was Memory / Vector Store) -----------------------------------
export type { CompressionOptions } from './ai/library/preservation.js';
export { cosineSimilarity } from './ai/library/cosine.js';
export type { TopicIndexOptions } from './ai/library/cataloging.js';
export type {
	BM25Options,
	BM25Result,
	InvertedIndex,
} from './ai/library/inverted-index.js';
export {
	createInvertedIndex,
	tokenizeForIndex,
} from './ai/library/inverted-index.js';
export type { LearningEngine } from './ai/library/patron-learning.js';
export { createLearningEngine } from './ai/library/patron-learning.js';
export type {
	Library,
	LibraryOptions,
} from './ai/library/library.js';
export {
	createLibrary,
} from './ai/library/library.js';
export type {
	LibraryContext,
	LibraryServices,
	LibraryServicesOptions,
} from './ai/library/library-services.js';
export {
	createLibraryServices,
} from './ai/library/library-services.js';
export type { PromptInjectionOptions } from './ai/library/prompt-injection.js';
export { formatMemoryContext } from './ai/library/prompt-injection.js';
export type { ParsedQuery } from './ai/library/query-dsl.js';
export { parseQuery } from './ai/library/query-dsl.js';
export type { RecencyOptions } from './ai/library/recommendation.js';
export type { StorageBackend } from './ai/library/storage.js';
export type { TextCache, TextCacheOptions } from './ai/library/text-cache.js';
export { createTextCache } from './ai/library/text-cache.js';
export { createShelf } from './ai/library/shelf.js';
export type { TopicCatalogOptions } from './ai/library/topic-catalog.js';
export { createTopicCatalog } from './ai/library/topic-catalog.js';
export type { LibrarianIdentity } from './ai/library/librarian.js';
export { createLibrarian, createDefaultLibrarian } from './ai/library/librarian.js';
export type { ValidationResult } from './ai/library/librarian-definition.js';
export {
	loadDefinition,
	loadAllDefinitions,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from './ai/library/librarian-definition.js';
export type {
	LibrarianRegistry,
	LibrarianRegistryOptions,
	ManagedLibrarian,
} from './ai/library/librarian-registry.js';
export { createLibrarianRegistry } from './ai/library/librarian-registry.js';
export type { CirculationDeskOptions } from './ai/library/circulation-desk.js';
export { createCirculationDesk } from './ai/library/circulation-desk.js';
export type {
	AdvancedLookup,
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
	ArbitrationResult,
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
} from './ai/library/types.js';
export { computeRecommendations } from './ai/library/stacks-recommend.js';
export type {
	StacksSearchConfig,
} from './ai/library/stacks-search.js';
export {
	advancedStacksSearch,
	filterVolumesByDateRange,
	filterVolumesByMetadata,
	stacksSearch,
	textSearchVolumes,
} from './ai/library/stacks-search.js';
export type {
	AccessStats,
	DeserializedData,
	SerializedData,
} from './ai/library/stacks-serialize.js';
export {
	deserializeFromStorage,
	serializeToStorage,
} from './ai/library/stacks-serialize.js';
export type {
	Stacks,
	StacksOptions,
} from './ai/library/stacks.js';
export {
	createStacks,
} from './ai/library/stacks.js';
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
// ---- Virtual Filesystem ---------------------------------------------------
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
} from './ai/vfs/index.js';
export {
	createDefaultValidators,
	createEmptyFileValidator,
	createJSONSyntaxValidator,
	createMissingTrailingNewlineValidator,
	createMixedIndentationValidator,
	createMixedLineEndingsValidator,
	createTrailingWhitespaceValidator,
	createVFSDisk,
	createVirtualFS,
	validateSnapshot,
} from './ai/vfs/index.js';
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
	createEmbeddingError,
	// Library (was Memory)
	createLibraryError,
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
	// Stacks errors
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	// VFS
	createVFSError,
	isChainError,
	isChainNotFoundError,
	isChainStepError,
	isCircuitBreakerOpenError,
	isConfigError,
	isConfigNotFoundError,
	isConfigParseError,
	isConfigValidationError,
	isEmbeddingError,
	isLoopAbortedError,
	isLoopError,
	isLoopTurnLimitError,
	isMCPConnectionError,
	isMCPError,
	isMCPServerNotConnectedError,
	isMCPToolError,
	isMCPTransportConfigError,
	isLibraryError,
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
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
	isVFSError,
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
