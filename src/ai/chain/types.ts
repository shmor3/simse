// ---------------------------------------------------------------------------
// Chain types — Provider, step config, step result, and callbacks
// ---------------------------------------------------------------------------

import type { ACPTokenUsage } from '../acp/types.js';
import type { ParallelConfig, ParallelSubResult } from '../agent/types.js';
import type { MCPToolCallMetrics } from '../mcp/types.js';
import type { PromptTemplate } from './prompt-template.js';

/**
 * Which AI backend a chain step executes against.
 */
export type Provider = 'acp' | 'mcp' | 'memory';

/**
 * Configuration for a single chain step.
 */
export interface ChainStepConfig {
	/** Unique name for this step. */
	readonly name: string;
	/** The prompt template to fill and send. */
	readonly template: PromptTemplate;
	/** Which AI provider to use for this step. */
	readonly provider?: Provider;
	/** ACP agent ID override for this step. */
	readonly agentId?: string;
	/** ACP server name override for this step. */
	readonly serverName?: string;
	/** Additional ACP run config passed to the agent. */
	readonly agentConfig?: Readonly<Record<string, unknown>>;
	/** System prompt prepended to the request (where supported). */
	readonly systemPrompt?: string;
	/** Transform the raw LLM output before passing to the next step. */
	readonly outputTransform?: (output: string) => string;
	/** Map previous step outputs to this step's template variables. */
	readonly inputMapping?: Readonly<Record<string, string>>;
	/** MCP: name of the connected MCP server to call. */
	readonly mcpServerName?: string;
	/** MCP: name of the tool to invoke on the MCP server. */
	readonly mcpToolName?: string;
	/** MCP: mapping from tool argument names to chain value keys. */
	readonly mcpArguments?: Readonly<Record<string, string>>;
	/** Store this step's output to the memory vector store. */
	readonly storeToMemory?: boolean;
	/** Metadata to attach when storing to memory. */
	readonly memoryMetadata?: Readonly<Record<string, string>>;
	/**
	 * When set, this step runs sub-steps concurrently instead of calling
	 * a single provider. The step's own template/provider are ignored
	 * when parallel is present.
	 */
	readonly parallel?: ParallelConfig;
}

/**
 * Result of executing a single chain step.
 */
export interface StepResult {
	/** Name of the step. */
	readonly stepName: string;
	/** Provider that was used. */
	readonly provider: Provider;
	/** Model / agent that was used. */
	readonly model: string;
	/** The fully-resolved prompt that was sent. */
	readonly input: string;
	/** The raw or transformed output from the provider. */
	readonly output: string;
	/** Wall-clock time for this step in milliseconds. */
	readonly durationMs: number;
	/** Zero-based index of this step in the chain. */
	readonly stepIndex: number;
	/** Token usage from ACP provider, if available. */
	readonly usage?: ACPTokenUsage;
	/** Tool call metrics from MCP provider. */
	readonly toolMetrics?: MCPToolCallMetrics;
	/** Sub-step results when this step ran in parallel mode. */
	readonly subResults?: readonly ParallelSubResult[];
}

/**
 * Callback hooks that fire during chain execution.
 * All callbacks are optional and async-safe.
 */
export interface ChainCallbacks {
	/** Fired before each step begins. */
	onStepStart?: (info: {
		stepName: string;
		stepIndex: number;
		totalSteps: number;
		provider: Provider;
		prompt: string;
	}) => void | Promise<void>;

	/** Fired after each step completes successfully. */
	onStepComplete?: (result: StepResult) => void | Promise<void>;

	/** Fired when a step fails (before the error is propagated). */
	onStepError?: (info: {
		stepName: string;
		stepIndex: number;
		error: Error;
	}) => void | Promise<void>;

	/** Fired when the entire chain completes successfully. */
	onChainComplete?: (results: StepResult[]) => void | Promise<void>;

	/** Fired when the chain fails (before the error is propagated). */
	onChainError?: (info: {
		error: Error;
		completedSteps: StepResult[];
	}) => void | Promise<void>;
}
