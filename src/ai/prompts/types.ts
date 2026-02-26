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
