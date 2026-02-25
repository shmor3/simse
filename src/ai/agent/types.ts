// ---------------------------------------------------------------------------
// Agent types — execution results, step config, parallel/swarm types
// ---------------------------------------------------------------------------

import type { ACPTokenUsage } from '../acp/types.js';
import type { PromptTemplate } from '../chain/prompt-template.js';
import type { Provider } from '../chain/types.js';
import type { MCPToolCallMetrics } from '../mcp/types.js';

/**
 * The raw result produced by executing a single agent call.
 */
export interface AgentResult {
	readonly output: string;
	readonly model: string;
	readonly usage?: ACPTokenUsage;
	readonly toolMetrics?: MCPToolCallMetrics;
}

/**
 * Configuration for a single agent execution call.
 * Contains only execution-relevant fields (no orchestration concerns).
 */
export interface AgentStepConfig {
	readonly name: string;
	readonly agentId?: string;
	readonly serverName?: string;
	readonly agentConfig?: Record<string, unknown>;
	readonly systemPrompt?: string;
	readonly mcpServerName?: string;
	readonly mcpToolName?: string;
	readonly mcpArguments?: Record<string, string>;
}

/**
 * Configuration for a single sub-step within a parallel group.
 */
export interface ParallelSubStepConfig {
	readonly name: string;
	readonly template: PromptTemplate;
	readonly provider?: Provider;
	readonly agentId?: string;
	readonly serverName?: string;
	readonly agentConfig?: Record<string, unknown>;
	readonly systemPrompt?: string;
	readonly outputTransform?: (output: string) => string;
	readonly mcpServerName?: string;
	readonly mcpToolName?: string;
	readonly mcpArguments?: Record<string, string>;
}

/**
 * Result produced by a single sub-step within a parallel group.
 */
export interface ParallelSubResult {
	readonly subStepName: string;
	readonly provider: Provider;
	readonly model: string;
	readonly input: string;
	readonly output: string;
	readonly durationMs: number;
	readonly usage?: ACPTokenUsage;
	readonly toolMetrics?: MCPToolCallMetrics;
}

/**
 * How parallel sub-step results are merged back into the chain's currentValues.
 *
 * - `'concat'`: join all sub-step outputs with a separator (default `'\n\n'`)
 * - `'keyed'`: store each sub-step output under `{stepName}.{subStepName}`,
 *   merged output is also concatenated under the parent step name
 * - function: custom reducer returning the merged string
 */
export type SwarmMergeStrategy =
	| 'concat'
	| 'keyed'
	| ((results: readonly ParallelSubResult[]) => string);

/**
 * Options controlling the parallel execution mode for a chain step.
 */
export interface ParallelConfig {
	readonly subSteps: readonly ParallelSubStepConfig[];
	readonly mergeStrategy?: SwarmMergeStrategy;
	readonly failTolerant?: boolean;
	readonly concatSeparator?: string;
}
