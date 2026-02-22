// ---------------------------------------------------------------------------
// Chain module — barrel re-export
// ---------------------------------------------------------------------------

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
