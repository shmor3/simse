import { describe, expect, it } from 'bun:test';
import type { SkillConfig } from '../skills.js';
import { createSkillRegistry } from '../skills.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSkill(overrides?: Partial<SkillConfig>): SkillConfig {
	return {
		name: overrides?.name ?? 'test-skill',
		description: overrides?.description ?? 'A test skill',
		allowedTools: overrides?.allowedTools ?? ['library_search'],
		argumentHint: overrides?.argumentHint ?? '<query>',
		model: overrides?.model,
		serverName: overrides?.serverName,
		body: overrides?.body ?? 'Skill body with $ARGUMENTS placeholder',
		filePath: overrides?.filePath ?? '/skills/test/SKILL.md',
	};
}

// ---------------------------------------------------------------------------
// createSkillRegistry
// ---------------------------------------------------------------------------

describe('createSkillRegistry', () => {
	it('should return a frozen object', () => {
		const registry = createSkillRegistry({ skills: [] });
		expect(Object.isFrozen(registry)).toBe(true);
	});

	it('should start empty when no skills provided', () => {
		const registry = createSkillRegistry({ skills: [] });
		expect(registry.skillCount).toBe(0);
		expect(registry.getAll()).toHaveLength(0);
	});

	// -- get / has / getAll ----------------------------------------------------

	it('should register and retrieve skills by name', () => {
		const skill = makeSkill({ name: 'commit' });
		const registry = createSkillRegistry({ skills: [skill] });

		expect(registry.has('commit')).toBe(true);
		expect(registry.get('commit')).toEqual(skill);
	});

	it('should return undefined for unknown skill names', () => {
		const registry = createSkillRegistry({ skills: [makeSkill()] });
		expect(registry.get('nonexistent')).toBeUndefined();
		expect(registry.has('nonexistent')).toBe(false);
	});

	it('should return all skills via getAll', () => {
		const skills = [
			makeSkill({ name: 'a' }),
			makeSkill({ name: 'b' }),
			makeSkill({ name: 'c' }),
		];
		const registry = createSkillRegistry({ skills });

		const all = registry.getAll();
		expect(all).toHaveLength(3);
		const names = all.map((s) => s.name);
		expect(names).toContain('a');
		expect(names).toContain('b');
		expect(names).toContain('c');
	});

	it('should return a frozen array from getAll', () => {
		const registry = createSkillRegistry({
			skills: [makeSkill()],
		});
		expect(Object.isFrozen(registry.getAll())).toBe(true);
	});

	it('should report correct skillCount', () => {
		const skills = [makeSkill({ name: 'a' }), makeSkill({ name: 'b' })];
		const registry = createSkillRegistry({ skills });
		expect(registry.skillCount).toBe(2);
	});

	// -- formatForSystemPrompt -------------------------------------------------

	it('should return empty string when no skills', () => {
		const registry = createSkillRegistry({ skills: [] });
		expect(registry.formatForSystemPrompt()).toBe('');
	});

	it('should format skills with name, argument hint, and description', () => {
		const registry = createSkillRegistry({
			skills: [
				makeSkill({
					name: 'commit',
					description: 'Create a commit',
					argumentHint: '<message>',
				}),
			],
		});

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('/commit');
		expect(prompt).toContain('<message>');
		expect(prompt).toContain('Create a commit');
	});

	it('should format skill without argument hint', () => {
		const registry = createSkillRegistry({
			skills: [
				makeSkill({
					name: 'help',
					description: 'Show help',
					argumentHint: '',
				}),
			],
		});

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('/help');
		expect(prompt).toContain('Show help');
	});

	it('should format skill without description', () => {
		const registry = createSkillRegistry({
			skills: [
				makeSkill({
					name: 'plain',
					description: '',
					argumentHint: '',
				}),
			],
		});

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('/plain');
	});

	it('should list multiple skills', () => {
		const registry = createSkillRegistry({
			skills: [
				makeSkill({ name: 'skill-a', description: 'Desc A' }),
				makeSkill({ name: 'skill-b', description: 'Desc B' }),
			],
		});

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('/skill-a');
		expect(prompt).toContain('/skill-b');
		expect(prompt).toContain('Desc A');
		expect(prompt).toContain('Desc B');
	});

	// -- resolveBody -----------------------------------------------------------

	it('should replace $ARGUMENTS in skill body', () => {
		const skill = makeSkill({
			body: 'Search for: $ARGUMENTS',
		});
		const registry = createSkillRegistry({ skills: [skill] });

		const resolved = registry.resolveBody(skill, 'auth flow');
		expect(resolved).toBe('Search for: auth flow');
	});

	it('should replace multiple $ARGUMENTS occurrences', () => {
		const skill = makeSkill({
			body: 'Query: $ARGUMENTS\nRepeat: $ARGUMENTS',
		});
		const registry = createSkillRegistry({ skills: [skill] });

		const resolved = registry.resolveBody(skill, 'test');
		expect(resolved).toBe('Query: test\nRepeat: test');
	});

	it('should handle empty arguments', () => {
		const skill = makeSkill({
			body: 'Do: $ARGUMENTS',
		});
		const registry = createSkillRegistry({ skills: [skill] });

		const resolved = registry.resolveBody(skill, '');
		expect(resolved).toBe('Do: ');
	});

	it('should leave body unchanged when no $ARGUMENTS placeholder', () => {
		const skill = makeSkill({
			body: 'No placeholders here',
		});
		const registry = createSkillRegistry({ skills: [skill] });

		const resolved = registry.resolveBody(skill, 'ignored');
		expect(resolved).toBe('No placeholders here');
	});

	// -- Edge cases ------------------------------------------------------------

	it('should handle duplicate skill names (last wins)', () => {
		const skills = [
			makeSkill({ name: 'dupe', description: 'first' }),
			makeSkill({ name: 'dupe', description: 'second' }),
		];
		const registry = createSkillRegistry({ skills });

		expect(registry.skillCount).toBe(1);
		expect(registry.get('dupe')?.description).toBe('second');
	});
});
