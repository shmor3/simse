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
	createLibraryError,
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	isEmbeddingError,
	isLibraryError,
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
	// Backward-compat aliases
	createMemoryError,
	createVectorStoreCorruptionError,
	createVectorStoreIOError,
	isMemoryError,
	isVectorStoreCorruptionError,
	isVectorStoreIOError,
} from './library.js';
export {
	createProviderError,
	createProviderGenerationError,
	createProviderHTTPError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	isProviderError,
	isProviderGenerationError,
	isProviderHTTPError,
	isProviderTimeoutError,
	isProviderUnavailableError,
} from './provider.js';
export {
	createCircuitBreakerOpenError,
	createTimeoutError,
	isCircuitBreakerOpenError,
	isTimeoutError,
} from './resilience.js';
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
