// ---------------------------------------------------------------------------
// SimSE — public library API
//
// This module is the main entry point for consumers of the simse package.
// It re-exports every public function, type, and interface that users
// need to configure, build, and run chains programmatically.
// ---------------------------------------------------------------------------

export type { ACPClient, ACPClientOptions } from './ai/acp/acp-client.js';
// ---- ACP (Agent Communication Protocol) -----------------------------------
export { createACPClient } from './ai/acp/acp-client.js';
export type {
	ACPAgentInfo,
	ACPChatMessage,
	ACPChatOptions,
	ACPConfig,
	ACPDataPart,
	ACPEmbedResult,
	ACPGenerateOptions,
	ACPGenerateResult,
	ACPMessage,
	ACPMessagePart,
	ACPRunError,
	ACPServerEntry,
	ACPStreamEvent,
	ACPTextPart,
} from './ai/acp/types.js';
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
	MCPToolInfo,
	MCPToolResult,
} from './ai/mcp/types.js';
export type { CompressionOptions } from './ai/memory/compression.js';
export { cosineSimilarity } from './ai/memory/cosine.js';
export type { TopicIndexOptions } from './ai/memory/indexing.js';
export type {
	MemoryManager,
	MemoryManagerOptions,
} from './ai/memory/memory.js';
// ---- Memory / Vector Store ------------------------------------------------
export { createMemoryManager } from './ai/memory/memory.js';
export type { RecencyOptions } from './ai/memory/recommendation.js';
export type {
	AdvancedSearchResult,
	DateRange,
	DuplicateCheckResult,
	DuplicateGroup,
	EmbeddingProvider,
	MemoryConfig,
	MetadataFilter,
	MetadataMatchMode,
	RecommendationResult,
	RecommendOptions,
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
// ---- Configuration --------------------------------------------------------
export type {
	ACPServerInput,
	AppConfig,
	ChainDefinition,
	ChainStepDefinition,
	DefineConfigOptions,
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
