import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { ContextGrid, HelpView } from '../features/meta/components.js';
import type { MetaCommandContext } from '../features/meta/index.js';
import { createMetaCommands } from '../features/meta/index.js';

function createMockContext(
	overrides?: Partial<MetaCommandContext>,
): MetaCommandContext {
	let verbose = false;
	let planMode = false;
	const ctx: MetaCommandContext = {
		getCommands: () => metaCommands,
		setVerbose: (on) => {
			verbose = on;
		},
		getVerbose: () => verbose,
		setPlanMode: (on) => {
			planMode = on;
		},
		getPlanMode: () => planMode,
		clearConversation: () => {},
		getContextUsage: () => ({ usedChars: 50000, maxChars: 200000 }),
		...overrides,
	};
	return ctx;
}

const metaCommands = createMetaCommands(createMockContext());

describe('meta feature module', () => {
	test('exports an array of command definitions', () => {
		expect(Array.isArray(metaCommands)).toBe(true);
		expect(metaCommands.length).toBeGreaterThan(0);
	});

	test('all commands have category "meta"', () => {
		for (const cmd of metaCommands) {
			expect(cmd.category).toBe('meta');
		}
	});

	test('includes help command with alias', () => {
		const help = metaCommands.find((c) => c.name === 'help');
		expect(help).toBeDefined();
		expect(help?.aliases).toContain('?');
	});

	test('includes clear command', () => {
		expect(metaCommands.find((c) => c.name === 'clear')).toBeDefined();
	});

	test('includes exit command with aliases', () => {
		const exit = metaCommands.find((c) => c.name === 'exit');
		expect(exit).toBeDefined();
		expect(exit?.aliases).toContain('quit');
		expect(exit?.aliases).toContain('q');
	});

	test('includes compact command', () => {
		expect(metaCommands.find((c) => c.name === 'compact')).toBeDefined();
	});
});

describe('meta command state wiring', () => {
	test('/verbose toggles state', () => {
		const ctx = createMockContext();
		const cmds = createMetaCommands(ctx);
		const verbose = cmds.find((c) => c.name === 'verbose');
		expect(verbose).toBeDefined();

		expect(ctx.getVerbose()).toBe(false);
		verbose?.execute('');
		expect(ctx.getVerbose()).toBe(true);
		verbose?.execute('');
		expect(ctx.getVerbose()).toBe(false);
		verbose?.execute('on');
		expect(ctx.getVerbose()).toBe(true);
		verbose?.execute('off');
		expect(ctx.getVerbose()).toBe(false);
	});

	test('/plan toggles state', () => {
		const ctx = createMockContext();
		const cmds = createMetaCommands(ctx);
		const plan = cmds.find((c) => c.name === 'plan');
		expect(plan).toBeDefined();

		expect(ctx.getPlanMode()).toBe(false);
		plan?.execute('');
		expect(ctx.getPlanMode()).toBe(true);
		plan?.execute('off');
		expect(ctx.getPlanMode()).toBe(false);
	});

	test('/clear calls clearConversation', () => {
		let cleared = false;
		const ctx = createMockContext({
			clearConversation: () => {
				cleared = true;
			},
		});
		const cmds = createMetaCommands(ctx);
		cmds.find((c) => c.name === 'clear')?.execute('');
		expect(cleared).toBe(true);
	});

	test('/context returns real usage', () => {
		const ctx = createMockContext({
			getContextUsage: () => ({ usedChars: 80000, maxChars: 200000 }),
		});
		const cmds = createMetaCommands(ctx);
		const contextCmd = cmds.find((c) => c.name === 'context');
		expect(contextCmd).toBeDefined();
		const result = contextCmd?.execute('');
		expect(result).toBeDefined();
		expect(result?.element).toBeDefined();
	});
});

describe('ContextGrid', () => {
	test('renders percentage', () => {
		const { lastFrame } = render(
			<ContextGrid usedChars={80000} maxChars={200000} />,
		);
		expect(lastFrame()).toContain('40%');
	});
});

describe('HelpView', () => {
	test('renders command list', () => {
		const { lastFrame } = render(<HelpView commands={metaCommands} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('/help');
		expect(frame).toContain('/exit');
	});
});
