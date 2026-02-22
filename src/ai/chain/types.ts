// ---------------------------------------------------------------------------
// Chain types — Provider, step config, step result, and callbacks
// ---------------------------------------------------------------------------

/**
 * Which AI backend a chain step executes against.
 */
export type Provider = 'acp' | 'mcp' | 'memory';

/**
 * Configuration for a single chain step.
 */
export interface ChainStepConfig {
	/** Unique name for this step. */
	name: string;
	/** The prompt template to fill and send. */
	template: import('./prompt-template.js').PromptTemplate;
	/** Which AI provider to use for this step. */
	provider?: Provider;
	/** ACP agent ID override for this step. */
	agentId?: string;
	/** ACP server name override for this step. */
	serverName?: string;
	/** Additional ACP run config passed to the agent. */
	agentConfig?: Record<string, unknown>;
	/** System prompt prepended to the request (where supported). */
	systemPrompt?: string;
	/** Transform the raw LLM output before passing to the next step. */
	outputTransform?: (output: string) => string;
	/** Map previous step outputs to this step's template variables. */
	inputMapping?: Record<string, string>;
	/** MCP: name of the connected MCP server to call. */
	mcpServerName?: string;
	/** MCP: name of the tool to invoke on the MCP server. */
	mcpToolName?: string;
	/** MCP: mapping from tool argument names to chain value keys. */
	mcpArguments?: Record<string, string>;
	/** Store this step's output to the memory vector store. */
	storeToMemory?: boolean;
	/** Metadata to attach when storing to memory. */
	memoryMetadata?: Record<string, string>;
}

/**
 * Result of executing a single chain step.
 */
export interface StepResult {
	/** Name of the step. */
	stepName: string;
	/** Provider that was used. */
	provider: Provider;
	/** Model / agent that was used. */
	model: string;
	/** The fully-resolved prompt that was sent. */
	input: string;
	/** The raw or transformed output from the provider. */
	output: string;
	/** Wall-clock time for this step in milliseconds. */
	durationMs: number;
	/** Zero-based index of this step in the chain. */
	stepIndex: number;
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
