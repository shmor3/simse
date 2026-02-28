import type { CommandDefinition } from '../../ink-types.js';
import type { ToolRegistry } from '../../tool-registry.js';

export interface ToolsCommandContext {
	readonly getToolRegistry: () => ToolRegistry;
}

export function createToolsCommands(
	ctx: ToolsCommandContext,
): readonly CommandDefinition[] {
	return [
		{
			name: 'tools',
			usage: '/tools',
			description: 'List available tools',
			category: 'tools',
			execute: () => {
				const registry = ctx.getToolRegistry();
				const defs = registry.getToolDefinitions();
				if (defs.length === 0) {
					return { text: 'No tools registered.' };
				}
				const lines = defs.map(
					(t) => `  ${t.name.padEnd(24)} ${t.description ?? ''}`,
				);
				return {
					text: `${defs.length} tool(s) available:\n${lines.join('\n')}`,
				};
			},
		},
		{
			name: 'agents',
			usage: '/agents',
			description: 'List available agents',
			category: 'tools',
			execute: () => ({ text: 'No agents configured. Use /setup to connect an ACP server.' }),
		},
		{
			name: 'skills',
			usage: '/skills',
			description: 'List available skills',
			category: 'tools',
			execute: () => ({ text: 'No skills configured.' }),
		},
	];
}
