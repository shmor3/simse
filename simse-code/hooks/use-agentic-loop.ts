import { useCallback, useRef, useState } from 'react';
import type { OutputItem, ToolCallState } from '../ink-types.js';

export function deriveToolSummary(
	name: string,
	output: string,
): string | undefined {
	if (!output) return undefined;
	const lines = output.split('\n');
	if (lines.length > 1) return `${lines.length} lines`;
	if (output.length > 100) return `${output.length} chars`;
	return undefined;
}

interface AgenticLoopState {
	readonly status: 'idle' | 'streaming' | 'tool-executing';
	readonly streamText: string;
	readonly activeToolCalls: readonly ToolCallState[];
	readonly completedItems: readonly OutputItem[];
}

interface UseAgenticLoopResult {
	readonly state: AgenticLoopState;
	readonly submit: (input: string) => Promise<void>;
	readonly abort: () => void;
}

/**
 * Hook for managing the agentic loop lifecycle.
 *
 * Full integration with createAgenticLoop will be wired in a follow-up
 * once providers are connected. For now, exports the state shape and helpers.
 */
export function useAgenticLoop(): UseAgenticLoopResult {
	const [state, setState] = useState<AgenticLoopState>({
		status: 'idle',
		streamText: '',
		activeToolCalls: [],
		completedItems: [],
	});

	const abortRef = useRef<AbortController | undefined>(undefined);

	const submit = useCallback(async (_input: string) => {
		const ctrl = new AbortController();
		abortRef.current = ctrl;

		setState((prev) => ({
			...prev,
			status: 'streaming',
			streamText: '',
			activeToolCalls: [],
		}));

		// TODO: Wire createAgenticLoop here with callbacks that
		// update streamText, activeToolCalls, and completedItems
		// via setState calls

		setState((prev) => ({
			...prev,
			status: 'idle',
		}));
	}, []);

	const abort = useCallback(() => {
		abortRef.current?.abort();
		setState((prev) => ({ ...prev, status: 'idle' }));
	}, []);

	return { state, submit, abort };
}
