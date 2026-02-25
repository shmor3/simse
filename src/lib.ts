// ---------------------------------------------------------------------------
// SimSE — public library API
//
// This module is the main entry point for consumers of the simse package.
// It re-exports every public function, type, and interface that users
// need to configure, build, and run chains programmatically.
// ---------------------------------------------------------------------------

export type { ACPClient, ACPClientOptions } from './ai/acp/acp-client.js';
// ---- ACP (Agent Client Protocol) ------------------------------------------
export { createACPClient } from './ai/acp/acp-client.js';
export type {
	ACPConnection,
	ACPConnectionOptions,
} from './ai/acp/acp-connection.js';
export { createACPConnection } from './ai/acp/acp-connection.js';
export type {
	ACPAgentInfo,
	ACPChatMessage,
	ACPChatOptions,
	ACPConfig,
	ACPContentBlock,
	ACPDataContent,
	ACPAgentCapabilities,
	ACPEmbedResult,
	ACPGenerateOptions,
	ACPGenerateResult,
	ACPInitializeResult,
	ACPPermissionPolicy,
	ACPServerEntry,
	ACPServerInfo,
	ACPSessionPromptResult,
	ACPStopReason,
	ACPStreamChunk,
	ACPStreamComplete,
	ACPStreamDelta,
	ACPTextContent,
	ACPTokenUsage,
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
export type { MCPClient } from './ai/mcp/mcp-client.js';
// ---- MCP (Model Context Protocol) ----------------------------------------
export { createMCPClient } from './ai/mcp/mcp-client.js';
export type { SimseMCPServer } from './ai/mcp/mcp-server.js';
export { createMCPServer } from './ai/mcp/mcp-server.js';
export type {
	MCPClientConfig,
	MCPPromptInfo,
	MCPResourceInfo,
	MCPServerConfig,
	MCPServerConnection,
	MCPToolCallMetrics,
	MCPToolInfo,
	MCPToolResult,
} from './ai/mcp/types.js';
export type { CompressionOptions } from './ai/memory/compression.js';
export { cosineSimilarity } from './ai/memory/cosine.js';
export type { TopicIndexOptions } from './ai/memory/indexing.js';
export type { LearningEngine } from './ai/memory/learning.js';
export { createLearningEngine } from './ai/memory/learning.js';
export type {
	MemoryManager,
	MemoryManagerOptions,
} from './ai/memory/memory.js';
// ---- Memory / Vector Store ------------------------------------------------
export { createMemoryManager } from './ai/memory/memory.js';
export type { RecencyOptions } from './ai/memory/recommendation.js';
export type { StorageBackend } from './ai/memory/storage.js';
export type {
	AdvancedSearchResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateGroup,
	EmbeddingProvider,
	LearningOptions,
	LearningProfile,
	MemoryConfig,
	MetadataFilter,
	MetadataMatchMode,
	QueryRecord,
	RecommendationResult,
	RecommendOptions,
	RelevanceFeedback,
	SearchOptions,
	SearchResult,
	SummarizeOptions,
	SummarizeResult,
	TextGenerationProvider,
	TextSearchMode,
	TextSearchOptions,
	TextSearchResult,
	TopicInfo,
	VectorEntry,
	WeightProfile,
} from './ai/memory/types.js';
export type {
	VectorStore,
	VectorStoreOptions,
} from './ai/memory/vector-store.js';
export { createVectorStore } from './ai/memory/vector-store.js';
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
	// Config
	createConfigError,
	createConfigNotFoundError,
	createConfigParseError,
	createConfigValidationError,
	createEmbeddingError,
	createMCPConnectionError,
	// MCP
	createMCPError,
	createMCPServerNotConnectedError,
	createMCPToolError,
	createMCPTransportConfigError,
	// Memory
	createMemoryError,
	// Provider
	createProviderError,
	createProviderGenerationError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	// Base
	createSimseError,
	// Template
	createTemplateError,
	createTemplateMissingVariablesError,
	createVectorStoreCorruptionError,
	createVectorStoreIOError,
	// VFS
	createVFSError,
	isChainError,
	isChainNotFoundError,
	isChainStepError,
	isConfigError,
	isConfigNotFoundError,
	isConfigParseError,
	isConfigValidationError,
	isEmbeddingError,
	isMCPConnectionError,
	isMCPError,
	isMCPServerNotConnectedError,
	isMCPToolError,
	isMCPTransportConfigError,
	isMemoryError,
	isProviderError,
	isProviderGenerationError,
	isProviderTimeoutError,
	isProviderUnavailableError,
	isSimseError,
	isTemplateError,
	isTemplateMissingVariablesError,
	isVectorStoreCorruptionError,
	isVectorStoreIOError,
	isVFSError,
	toError,
	wrapError,
} from './errors/index.js';

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

// ---- Utilities ------------------------------------------------------------
export type { RetryOptions } from './utils/retry.js';
export {
	createRetryExhaustedError,
	isRetryAbortedError,
	isRetryExhaustedError,
	isTransientError,
	retry,
	sleep,
} from './utils/retry.js';
