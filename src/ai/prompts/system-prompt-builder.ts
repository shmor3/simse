// ---------------------------------------------------------------------------
// System Prompt Builder
//
// Assembles a system prompt from static and dynamic sections in a
// cache-friendly order: identity → mode instructions → tool guidelines →
// environment → instructions → custom sections → tool definitions → memory.
// ---------------------------------------------------------------------------

import type {
	AgentMode,
	SystemPromptBuildContext,
	SystemPromptBuilder,
	SystemPromptBuilderOptions,
} from './types.js';

// ---------------------------------------------------------------------------
// Default sections
// ---------------------------------------------------------------------------

const DEFAULT_IDENTITY = 'You are a software development assistant.';

const DEFAULT_MODE_INSTRUCTIONS: Readonly<Record<AgentMode, string>> = {
	build: `# Operating Mode: Build

Follow a gather-action-verify workflow:
1. **Gather**: Read relevant files, understand the codebase, and plan your approach before making changes.
2. **Action**: Make precise, minimal changes. Prefer editing existing files over creating new ones.
3. **Verify**: After changes, run relevant checks (typecheck, lint, tests) to confirm correctness.

Guidelines:
- Only modify code you have read and understood.
- Keep changes focused — do not add features, refactoring, or improvements beyond what was requested.
- Use parallel tool calls when operations are independent.
- When uncertain, ask for clarification rather than guessing.`,

	plan: `# Operating Mode: Plan

You are in planning mode. Research the codebase, analyze the task, and produce a structured implementation plan.

Constraints:
- Do NOT modify any files — read-only exploration only.
- Do NOT execute commands that change state (no writes, installs, or deletions).
- Output a clear, numbered implementation plan with file paths and descriptions of changes.`,

	explore: `# Operating Mode: Explore

You are in exploration mode. Quickly find information in the codebase and return concise answers.

Constraints:
- Do NOT modify any files — read-only exploration only.
- Be concise — answer the question directly without unnecessary elaboration.
- Use search tools (grep, glob) efficiently to locate relevant code.`,
};

const TOOL_GUIDELINES = `# Tool Usage Guidelines

- When multiple tool calls are independent, execute them in parallel.
- Use search tools (grep, glob) before reading files to find the right targets.
- Always read a file before editing it.
- Prefer editing existing files over creating new ones.
- After writing code, verify it compiles/passes checks when possible.`;

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a system prompt builder that assembles prompts from static
 * configuration and dynamic per-turn context.
 *
 * Sections are ordered for optimal cache efficiency: static content first,
 * dynamic content (memory, instructions) last.
 *
 * @param options - Identity text, mode instruction overrides, custom sections.
 * @returns A frozen {@link SystemPromptBuilder}.
 */
export function createSystemPromptBuilder(
	options?: SystemPromptBuilderOptions,
): SystemPromptBuilder {
	const identity = options?.identity ?? DEFAULT_IDENTITY;
	const modeOverrides = options?.modeInstructions ?? {};
	const customSections = options?.customSections ?? [];
	const toolRegistry = options?.toolRegistry;

	const getModeInstructions = (mode: AgentMode): string => {
		return modeOverrides[mode] ?? DEFAULT_MODE_INSTRUCTIONS[mode];
	};

	const build = (context?: SystemPromptBuildContext): string => {
		const mode = context?.mode ?? 'build';
		const sections: string[] = [];

		// 1. Identity (static, cacheable)
		sections.push(identity);

		// 2. Mode instructions
		sections.push(getModeInstructions(mode));

		// 3. Tool usage guidelines (static)
		sections.push(TOOL_GUIDELINES);

		// 4. Environment context (semi-dynamic)
		if (context?.environment) {
			const env = context.environment;
			const lines = [
				'# Environment',
				`- Platform: ${env.platform}`,
				`- Shell: ${env.shell}`,
				`- Working directory: ${env.cwd}`,
				`- Date: ${env.date}`,
			];
			if (env.gitBranch) {
				lines.push(`- Git branch: ${env.gitBranch}`);
			}
			if (env.gitStatus) {
				lines.push(
					env.gitStatus === 'clean'
						? '- Git status: clean'
						: `- Git status:\n${env.gitStatus}`,
				);
			}
			sections.push(lines.join('\n'));
		}

		// 5. Instruction files (SIMSE.md, CLAUDE.md, etc.)
		if (context?.instructions && context.instructions.length > 0) {
			const instrSection = context.instructions
				.map((i) => `## ${i.path}\n\n${i.content}`)
				.join('\n\n');
			sections.push(`# Project Instructions\n\n${instrSection}`);
		}

		// 6. Custom sections
		for (const section of customSections) {
			if (section) sections.push(section);
		}

		// 7. Tool definitions
		if (toolRegistry) {
			const toolDefs = toolRegistry.formatForSystemPrompt();
			if (toolDefs) {
				sections.push(toolDefs);
			}
		}

		// 8. Memory context (most dynamic, last for cache efficiency)
		if (context?.memoryContext) {
			sections.push(`# Memory Context\n\n${context.memoryContext}`);
		}

		return sections.join('\n\n');
	};

	return Object.freeze({ build });
}
