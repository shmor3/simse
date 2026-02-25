// ---------------------------------------------------------------------------
// Agent module — barrel re-export
// ---------------------------------------------------------------------------

export type { AgentExecutor, AgentExecutorOptions } from './agent-executor.js';
export { createAgentExecutor } from './agent-executor.js';
export type {
	AgentResult,
	AgentStepConfig,
	ParallelConfig,
	ParallelSubResult,
	ParallelSubStepConfig,
	SwarmMergeStrategy,
} from './types.js';
