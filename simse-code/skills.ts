/**
 * SimSE CLI — Skills System
 *
 * Skills are prompt-based context modifiers that temporarily transform
 * the agent into a specialized one. They are markdown files with YAML
 * frontmatter discovered from .simse/skills/<name>/SKILL.md.
 *
 * When invoked (via /skill-name or AI auto-invocation), the skill body
 * is injected into the conversation context for the agentic loop.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SkillConfig {
	readonly name: string;
	readonly description: string;
	readonly allowedTools: readonly string[];
	readonly argumentHint: string;
	readonly model: string | undefined;
	readonly serverName: string | undefined;
	readonly body: string;
	readonly filePath: string;
}

export interface SkillRegistryOptions {
	readonly skills: readonly SkillConfig[];
}

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface SkillRegistry {
	readonly get: (name: string) => SkillConfig | undefined;
	readonly getAll: () => readonly SkillConfig[];
	readonly has: (name: string) => boolean;
	readonly formatForSystemPrompt: () => string;
	readonly resolveBody: (skill: SkillConfig, args: string) => string;
	readonly skillCount: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSkillRegistry(
	options: SkillRegistryOptions,
): SkillRegistry {
	const skillMap = new Map<string, SkillConfig>();

	for (const skill of options.skills) {
		skillMap.set(skill.name, skill);
	}

	const get = (name: string): SkillConfig | undefined => skillMap.get(name);

	const getAll = (): readonly SkillConfig[] =>
		Object.freeze([...skillMap.values()]);

	const has = (name: string): boolean => skillMap.has(name);

	const formatForSystemPrompt = (): string => {
		if (skillMap.size === 0) return '';

		const lines: string[] = [
			'Available skills (the user can invoke with /skill-name):',
			'',
		];

		for (const skill of skillMap.values()) {
			const hint = skill.argumentHint ? ` ${skill.argumentHint}` : '';
			const desc = skill.description ? `: ${skill.description}` : '';
			lines.push(`- /${skill.name}${hint}${desc}`);
		}

		return lines.join('\n');
	};

	const resolveBody = (skill: SkillConfig, args: string): string =>
		skill.body.replace(/\$ARGUMENTS/g, args);

	return Object.freeze({
		get,
		getAll,
		has,
		formatForSystemPrompt,
		resolveBody,
		get skillCount() {
			return skillMap.size;
		},
	});
}
