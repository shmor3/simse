// ---------------------------------------------------------------------------
// Provider Prompt Resolver
//
// Matches model IDs against glob patterns to resolve provider-specific
// prompts.  First matching pattern wins; falls back to defaultPrompt
// or empty string.
//
// Glob syntax:  `*` matches any sequence of characters.
// All other regex metacharacters are escaped so patterns are safe.
// ---------------------------------------------------------------------------

import type { ProviderPromptConfig, ProviderPromptResolver } from './types.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert a simple glob pattern to a RegExp.
 * `*` becomes `.*`; all other regex-special characters are escaped.
 */
function globToRegExp(pattern: string): RegExp {
	const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
	const withWildcard = escaped.replace(/\*/g, '.*');
	return new RegExp(`^${withWildcard}$`);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createProviderPromptResolver(
	config: ProviderPromptConfig,
): ProviderPromptResolver {
	const entries = config.prompts
		? Object.entries(config.prompts).map(
				([pattern, prompt]) => [globToRegExp(pattern), prompt] as const,
			)
		: [];

	const resolve = (modelId: string): string => {
		for (const [regex, prompt] of entries) {
			if (regex.test(modelId)) return prompt;
		}
		return config.defaultPrompt ?? '';
	};

	return Object.freeze({ resolve });
}
