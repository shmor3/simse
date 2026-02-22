// ---------------------------------------------------------------------------
// Chain Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createChainError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		chainName?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError & { readonly chainName?: string } => {
	const err = createSimseError(message, {
		name: options.name ?? 'ChainError',
		code: options.code ?? 'CHAIN_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: { ...options.metadata, chainName: options.chainName },
	}) as SimseError & { readonly chainName?: string };

	Object.defineProperty(err, 'chainName', {
		value: options.chainName,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createChainStepError = (
	stepName: string,
	stepIndex: number,
	message: string,
	options: {
		chainName?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError & {
	readonly chainName?: string;
	readonly stepName: string;
	readonly stepIndex: number;
} => {
	const err = createChainError(
		`Step "${stepName}" (index ${stepIndex}) failed: ${message}`,
		{
			name: 'ChainStepError',
			code: 'CHAIN_STEP_ERROR',
			chainName: options.chainName,
			cause: options.cause,
			metadata: { ...options.metadata, stepName, stepIndex },
		},
	) as SimseError & {
		readonly chainName?: string;
		readonly stepName: string;
		readonly stepIndex: number;
	};

	Object.defineProperties(err, {
		stepName: { value: stepName, writable: false, enumerable: true },
		stepIndex: { value: stepIndex, writable: false, enumerable: true },
	});

	return err;
};

export const createChainNotFoundError = (
	chainName: string,
): SimseError & { readonly chainName?: string } =>
	createChainError(`Chain "${chainName}" is not defined in configuration`, {
		name: 'ChainNotFoundError',
		code: 'CHAIN_NOT_FOUND',
		chainName,
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isChainError = (
	value: unknown,
): value is SimseError & { readonly chainName?: string } =>
	isSimseError(value) && value.code.startsWith('CHAIN_');

export const isChainStepError = (
	value: unknown,
): value is SimseError & {
	readonly chainName?: string;
	readonly stepName: string;
	readonly stepIndex: number;
} => isSimseError(value) && value.code === 'CHAIN_STEP_ERROR';

export const isChainNotFoundError = (
	value: unknown,
): value is SimseError & { readonly chainName?: string } =>
	isSimseError(value) && value.code === 'CHAIN_NOT_FOUND';
