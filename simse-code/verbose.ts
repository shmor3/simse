/**
 * SimSE Code — Verbose/Details Toggle
 *
 * Controls verbose mode for showing full tool args, full output,
 * thinking text, and duration info.
 * No external deps.
 */

import type { VerboseState } from './app-context.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { VerboseState };

export interface VerboseStateOptions {
	readonly initial?: boolean;
	readonly onChange?: (verbose: boolean) => void;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createVerboseState(
	options?: VerboseStateOptions,
): VerboseState {
	const state: VerboseState = {
		isVerbose: options?.initial ?? false,
		toggle: () => {
			(state as { isVerbose: boolean }).isVerbose = !state.isVerbose;
			options?.onChange?.(state.isVerbose);
		},
		set: (value: boolean) => {
			(state as { isVerbose: boolean }).isVerbose = value;
			options?.onChange?.(state.isVerbose);
		},
	};

	return Object.freeze(state);
}
