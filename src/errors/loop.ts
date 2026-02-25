// ---------------------------------------------------------------------------
// Agentic Loop Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createLoopError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'LoopError',
		code: options.code ?? 'LOOP_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createLoopTurnLimitError = (
	maxTurns: number,
	options: { cause?: unknown } = {},
): SimseError =>
	createLoopError(`Agentic loop hit turn limit: ${maxTurns}`, {
		name: 'LoopTurnLimitError',
		code: 'LOOP_TURN_LIMIT',
		cause: options.cause,
		metadata: { maxTurns },
	});

export const createLoopAbortedError = (
	turn: number,
	options: { cause?: unknown } = {},
): SimseError =>
	createLoopError(`Agentic loop aborted at turn ${turn}`, {
		name: 'LoopAbortedError',
		code: 'LOOP_ABORTED',
		cause: options.cause,
		metadata: { turn },
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isLoopError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('LOOP_');

export const isLoopTurnLimitError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'LOOP_TURN_LIMIT';

export const isLoopAbortedError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'LOOP_ABORTED';
