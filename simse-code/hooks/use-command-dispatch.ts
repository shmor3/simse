import { useCallback } from 'react';
import type { CommandRegistry } from '../command-registry.js';
import type { CommandResult } from '../ink-types.js';

interface UseCommandDispatchResult {
	readonly dispatch: (input: string) => Promise<CommandResult | undefined>;
	readonly isCommand: (input: string) => boolean;
}

export function useCommandDispatch(
	registry: CommandRegistry,
): UseCommandDispatchResult {
	const isCommand = useCallback((input: string) => input.startsWith('/'), []);

	const dispatch = useCallback(
		async (input: string): Promise<CommandResult | undefined> => {
			const trimmed = input.trim();
			if (!trimmed.startsWith('/')) return undefined;

			const spaceIdx = trimmed.indexOf(' ');
			const name =
				spaceIdx === -1 ? trimmed.slice(1) : trimmed.slice(1, spaceIdx);
			const args = spaceIdx === -1 ? '' : trimmed.slice(spaceIdx + 1).trim();

			const command = registry.get(name);
			if (!command) {
				return {
					text: `Unknown command: /${name}. Type /help for available commands.`,
				};
			}

			const result = command.execute(args);
			return result instanceof Promise ? await result : (result ?? undefined);
		},
		[registry],
	);

	return { dispatch, isCommand };
}
