// ---------------------------------------------------------------------------
// Provider Prompts & Instruction Discovery — Types
//
// Readonly interfaces for model-specific prompt resolution and
// instruction file discovery (CLAUDE.md, AGENTS.md, etc.).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Provider Prompt Resolution
// ---------------------------------------------------------------------------

/**
 * Configuration for provider-specific prompts.
 * Keys in the `prompts` record are glob patterns matched against model IDs.
 */
export interface ProviderPromptConfig {
	readonly prompts?: Readonly<Record<string, string>>;
	readonly defaultPrompt?: string;
}

/**
 * Resolves a model ID to the best-matching provider prompt.
 */
export interface ProviderPromptResolver {
	readonly resolve: (modelId: string) => string;
}

// ---------------------------------------------------------------------------
// Instruction Discovery
// ---------------------------------------------------------------------------

/**
 * Options for discovering instruction files in a project directory.
 */
export interface InstructionDiscoveryOptions {
	readonly patterns?: readonly string[];
	readonly rootDir: string;
}

/**
 * A discovered instruction file with its path and content.
 */
export interface DiscoveredInstruction {
	readonly path: string;
	readonly content: string;
}

// ---------------------------------------------------------------------------
// Environment Context
// ---------------------------------------------------------------------------

export interface EnvironmentContext {
	readonly platform: string;
	readonly shell: string;
	readonly cwd: string;
	readonly date: string;
	readonly gitBranch?: string;
	readonly gitStatus?: string;
}

// ---------------------------------------------------------------------------
// System Prompt Builder
// ---------------------------------------------------------------------------

/**
 * Agent operating mode.
 * - `build`: Default mode — gathers context, takes actions, verifies results.
 * - `plan`: Research and planning only — no code modifications.
 * - `explore`: Fast, read-only codebase exploration.
 */
export type AgentMode = 'build' | 'plan' | 'explore';

export interface SystemPromptBuilderOptions {
	/** Agent identity line. Default: "You are a software development assistant." */
	readonly identity?: string;
	/** Override mode instructions per mode. */
	readonly modeInstructions?: Readonly<Partial<Record<AgentMode, string>>>;
	/** Additional custom sections appended after tool guidelines. */
	readonly customSections?: readonly string[];
	/** Tool registry for generating tool definitions section. */
	readonly toolRegistry?: {
		readonly formatForSystemPrompt: () => string;
	};
}

export interface SystemPromptBuildContext {
	/** Current operating mode. Default: 'build'. */
	readonly mode?: AgentMode;
	/** Environment context (platform, shell, cwd, git). */
	readonly environment?: EnvironmentContext;
	/** Discovered instruction files (SIMSE.md, CLAUDE.md, etc.). */
	readonly instructions?: readonly DiscoveredInstruction[];
	/** Dynamic memory context injected per turn. */
	readonly memoryContext?: string;
}

export interface SystemPromptBuilder {
	readonly build: (context?: SystemPromptBuildContext) => string;
}
