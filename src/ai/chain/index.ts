// ---------------------------------------------------------------------------
// Chain module — barrel re-export
// ---------------------------------------------------------------------------

export type {
	AgentExecutor,
	AgentExecutorOptions,
	AgentResult,
	AgentStepConfig,
	ParallelConfig,
	ParallelSubResult,
	ParallelSubStepConfig,
	SwarmMergeStrategy,
} from '../agent/index.js';
export { createAgentExecutor } from '../agent/index.js';
export type { Chain, ChainOptions } from './chain.js';
export {
	createChain,
	createChainFromDefinition,
	runNamedChain,
} from './chain.js';
export type { FormatSearchResultsOptions } from './format.js';
export { formatSearchResults } from './format.js';
export type { PromptTemplate } from './prompt-template.js';
export { createPromptTemplate, isPromptTemplate } from './prompt-template.js';
export type {
	ChainCallbacks,
	ChainStepConfig,
	Provider,
	StepResult,
} from './types.js';
