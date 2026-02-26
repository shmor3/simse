/**
 * SimSE Code — Plan Mode
 *
 * Read-only mode where write/bash tools are denied.
 * Integrates with the permission system and hook system.
 * No external deps.
 */

import type { PlanModeState } from './app-context.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { PlanModeState };

export interface PlanModeOptions {
	readonly initial?: boolean;
	readonly onChange?: (active: boolean) => void;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createPlanMode(options?: PlanModeOptions): PlanModeState {
	const state: PlanModeState = {
		isActive: options?.initial ?? false,
		toggle: () => {
			(state as { isActive: boolean }).isActive = !state.isActive;
			options?.onChange?.(state.isActive);
		},
		set: (value: boolean) => {
			(state as { isActive: boolean }).isActive = value;
			options?.onChange?.(state.isActive);
		},
	};

	return Object.freeze(state);
}
