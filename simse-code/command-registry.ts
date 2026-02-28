import type { CommandCategory, CommandDefinition } from './ink-types.js';

export interface CommandRegistry {
	readonly register: (command: CommandDefinition) => void;
	readonly registerAll: (commands: readonly CommandDefinition[]) => void;
	readonly get: (nameOrAlias: string) => CommandDefinition | undefined;
	readonly getAll: () => readonly CommandDefinition[];
	readonly getByCategory: (
		category: CommandCategory,
	) => readonly CommandDefinition[];
}

export function createCommandRegistry(): CommandRegistry {
	const commands = new Map<string, CommandDefinition>();
	const aliases = new Map<string, string>();

	function register(command: CommandDefinition): void {
		commands.set(command.name, command);
		if (command.aliases) {
			for (const alias of command.aliases) {
				aliases.set(alias, command.name);
			}
		}
	}

	function registerAll(cmds: readonly CommandDefinition[]): void {
		for (const cmd of cmds) register(cmd);
	}

	function get(nameOrAlias: string): CommandDefinition | undefined {
		return (
			commands.get(nameOrAlias) ?? commands.get(aliases.get(nameOrAlias) ?? '')
		);
	}

	function getAll(): readonly CommandDefinition[] {
		return [...commands.values()];
	}

	function getByCategory(
		category: CommandCategory,
	): readonly CommandDefinition[] {
		return [...commands.values()].filter((c) => c.category === category);
	}

	return Object.freeze({ register, registerAll, get, getAll, getByCategory });
}
