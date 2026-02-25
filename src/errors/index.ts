// ---------------------------------------------------------------------------
// Error barrel — re-exports all error factories, type guards, and utilities
// ---------------------------------------------------------------------------

export {
	createSimseError,
	isSimseError,
	type SimseError,
	type SimseErrorOptions,
	toError,
	wrapError,
} from './base.js';
export {
	createChainError,
	createChainNotFoundError,
	createChainStepError,
	isChainError,
	isChainNotFoundError,
	isChainStepError,
} from './chain.js';
export {
	createConfigError,
	createConfigNotFoundError,
	createConfigParseError,
	createConfigValidationError,
	isConfigError,
	isConfigNotFoundError,
	isConfigParseError,
	isConfigValidationError,
} from './config.js';
export {
	createLoopAbortedError,
	createLoopError,
	createLoopTurnLimitError,
	isLoopAbortedError,
	isLoopError,
	isLoopTurnLimitError,
} from './loop.js';
export {
	createMCPConnectionError,
	createMCPError,
	createMCPServerNotConnectedError,
	createMCPToolError,
	createMCPTransportConfigError,
	isMCPConnectionError,
	isMCPError,
	isMCPServerNotConnectedError,
	isMCPToolError,
	isMCPTransportConfigError,
} from './mcp.js';
export {
	createEmbeddingError,
	createMemoryError,
	createVectorStoreCorruptionError,
	createVectorStoreIOError,
	isEmbeddingError,
	isMemoryError,
	isVectorStoreCorruptionError,
	isVectorStoreIOError,
} from './memory.js';
export {
	createProviderError,
	createProviderGenerationError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	isProviderError,
	isProviderGenerationError,
	isProviderTimeoutError,
	isProviderUnavailableError,
} from './provider.js';
export {
	createTaskCircularDependencyError,
	createTaskError,
	createTaskNotFoundError,
	isTaskCircularDependencyError,
	isTaskError,
	isTaskNotFoundError,
} from './tasks.js';
export {
	createTemplateError,
	createTemplateMissingVariablesError,
	isTemplateError,
	isTemplateMissingVariablesError,
} from './template.js';
export {
	createToolError,
	createToolExecutionError,
	createToolNotFoundError,
	isToolError,
	isToolExecutionError,
	isToolNotFoundError,
} from './tools.js';
export { createVFSError, isVFSError } from './vfs.js';
