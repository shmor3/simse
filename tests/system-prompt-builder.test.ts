import { describe, expect, it } from 'bun:test';
import { createSystemPromptBuilder } from '../src/ai/prompts/system-prompt-builder.js';
import type {
	EnvironmentContext,
	SystemPromptBuildContext,
} from '../src/ai/prompts/types.js';

// ---------------------------------------------------------------------------
// createSystemPromptBuilder
// ---------------------------------------------------------------------------

describe('createSystemPromptBuilder', () => {
	it('returns a frozen object', () => {
		const builder = createSystemPromptBuilder();
		expect(Object.isFrozen(builder)).toBe(true);
	});

	it('includes default identity in output', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build();
		expect(prompt).toContain('You are a software development assistant.');
	});

	it('allows custom identity', () => {
		const builder = createSystemPromptBuilder({
			identity: 'You are a code reviewer.',
		});
		const prompt = builder.build();
		expect(prompt).toContain('You are a code reviewer.');
		expect(prompt).not.toContain('software development assistant');
	});

	it('includes build mode instructions by default', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build();
		expect(prompt).toContain('Operating Mode: Build');
		expect(prompt).toContain('gather-action-verify');
	});

	it('includes plan mode instructions when requested', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({ mode: 'plan' });
		expect(prompt).toContain('Operating Mode: Plan');
		expect(prompt).toContain('planning mode');
	});

	it('includes explore mode instructions when requested', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({ mode: 'explore' });
		expect(prompt).toContain('Operating Mode: Explore');
		expect(prompt).toContain('exploration mode');
	});

	it('allows mode instruction overrides', () => {
		const builder = createSystemPromptBuilder({
			modeInstructions: {
				build: 'Custom build instructions here',
			},
		});
		const prompt = builder.build({ mode: 'build' });
		expect(prompt).toContain('Custom build instructions here');
		expect(prompt).not.toContain('gather-action-verify');
	});

	it('includes tool usage guidelines', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build();
		expect(prompt).toContain('Tool Usage Guidelines');
	});

	it('includes environment context when provided', () => {
		const env: EnvironmentContext = {
			platform: 'linux',
			shell: '/bin/bash',
			cwd: '/home/user/project',
			date: '2026-02-26',
			gitBranch: 'main',
			gitStatus: 'clean',
		};
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({ environment: env });
		expect(prompt).toContain('Platform: linux');
		expect(prompt).toContain('Shell: /bin/bash');
		expect(prompt).toContain('Working directory: /home/user/project');
		expect(prompt).toContain('Date: 2026-02-26');
		expect(prompt).toContain('Git branch: main');
		expect(prompt).toContain('Git status: clean');
	});

	it('includes dirty git status', () => {
		const env: EnvironmentContext = {
			platform: 'linux',
			shell: '/bin/bash',
			cwd: '/tmp',
			date: '2026-01-01',
			gitBranch: 'feature',
			gitStatus: 'M src/foo.ts',
		};
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({ environment: env });
		expect(prompt).toContain('M src/foo.ts');
	});

	it('omits git info when not in a git repo', () => {
		const env: EnvironmentContext = {
			platform: 'linux',
			shell: '/bin/bash',
			cwd: '/tmp',
			date: '2026-01-01',
		};
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({ environment: env });
		expect(prompt).not.toContain('Git branch');
	});

	it('includes instruction files when provided', () => {
		const context: SystemPromptBuildContext = {
			instructions: [
				{ path: 'SIMSE.md', content: 'Project rules here' },
				{ path: 'CLAUDE.md', content: 'Claude instructions' },
			],
		};
		const builder = createSystemPromptBuilder();
		const prompt = builder.build(context);
		expect(prompt).toContain('Project Instructions');
		expect(prompt).toContain('SIMSE.md');
		expect(prompt).toContain('Project rules here');
		expect(prompt).toContain('CLAUDE.md');
		expect(prompt).toContain('Claude instructions');
	});

	it('includes custom sections', () => {
		const builder = createSystemPromptBuilder({
			customSections: ['Custom section A', 'Custom section B'],
		});
		const prompt = builder.build();
		expect(prompt).toContain('Custom section A');
		expect(prompt).toContain('Custom section B');
	});

	it('skips empty custom sections', () => {
		const builder = createSystemPromptBuilder({
			customSections: ['Valid section', ''],
		});
		const prompt = builder.build();
		expect(prompt).toContain('Valid section');
	});

	it('includes tool definitions from registry', () => {
		const mockRegistry = {
			formatForSystemPrompt: () => '<tools>mock tools</tools>',
		};
		const builder = createSystemPromptBuilder({ toolRegistry: mockRegistry });
		const prompt = builder.build();
		expect(prompt).toContain('<tools>mock tools</tools>');
	});

	it('skips tool definitions when registry returns empty string', () => {
		const mockRegistry = {
			formatForSystemPrompt: () => '',
		};
		const builder = createSystemPromptBuilder({ toolRegistry: mockRegistry });
		const prompt = builder.build();
		// Should not have a double newline from an empty section
		expect(prompt).not.toContain('\n\n\n\n');
	});

	it('includes memory context when provided', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({
			memoryContext: 'User prefers TypeScript and Bun.',
		});
		expect(prompt).toContain('Memory Context');
		expect(prompt).toContain('User prefers TypeScript and Bun.');
	});

	it('places memory context last for cache efficiency', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build({
			memoryContext: 'DYNAMIC_MEMORY_MARKER',
		});
		const memoryIndex = prompt.indexOf('DYNAMIC_MEMORY_MARKER');
		const identityIndex = prompt.indexOf(
			'You are a software development assistant.',
		);
		expect(memoryIndex).toBeGreaterThan(identityIndex);
	});

	it('builds with no context at all', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build();
		expect(prompt.length).toBeGreaterThan(0);
		expect(prompt).toContain('You are a software development assistant.');
	});

	it('builds with undefined context', () => {
		const builder = createSystemPromptBuilder();
		const prompt = builder.build(undefined);
		expect(prompt.length).toBeGreaterThan(0);
	});
});
