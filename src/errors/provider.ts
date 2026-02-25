// ---------------------------------------------------------------------------
// Provider Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createProviderError = (
	provider: string,
	message: string,
	options: {
		name?: string;
		code?: string;
		statusCode?: number;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError & { readonly provider: string } => {
	const err = createSimseError(message, {
		name: options.name ?? 'ProviderError',
		code: options.code ?? 'PROVIDER_ERROR',
		statusCode: options.statusCode ?? 502,
		cause: options.cause,
		metadata: { ...options.metadata, provider },
	}) as SimseError & { readonly provider: string };

	Object.defineProperty(err, 'provider', {
		value: provider,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createProviderUnavailableError = (
	provider: string,
	options: { cause?: unknown; metadata?: Record<string, unknown> } = {},
): SimseError & { readonly provider: string } =>
	createProviderError(provider, `Provider "${provider}" is not available`, {
		name: 'ProviderUnavailableError',
		code: 'PROVIDER_UNAVAILABLE',
		statusCode: 503,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createProviderTimeoutError = (
	provider: string,
	timeoutMs: number,
	options: { cause?: unknown } = {},
): SimseError & { readonly provider: string; readonly timeoutMs: number } => {
	const err = createProviderError(
		provider,
		`Provider "${provider}" timed out after ${timeoutMs}ms`,
		{
			name: 'ProviderTimeoutError',
			code: 'PROVIDER_TIMEOUT',
			statusCode: 504,
			cause: options.cause,
			metadata: { timeoutMs },
		},
	) as SimseError & { readonly provider: string; readonly timeoutMs: number };

	Object.defineProperty(err, 'timeoutMs', {
		value: timeoutMs,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createProviderGenerationError = (
	provider: string,
	message: string,
	options: { cause?: unknown; model?: string } = {},
): SimseError & { readonly provider: string } =>
	createProviderError(provider, message, {
		name: 'ProviderGenerationError',
		code: 'PROVIDER_GENERATION_FAILED',
		statusCode: 502,
		cause: options.cause,
		metadata: options.model ? { model: options.model } : {},
	});

export const createProviderHTTPError = (
	provider: string,
	statusCode: number,
	message: string,
	options: { cause?: unknown; metadata?: Record<string, unknown> } = {},
): SimseError & { readonly provider: string } =>
	createProviderError(provider, message, {
		name: 'ProviderHTTPError',
		code: 'PROVIDER_HTTP_ERROR',
		statusCode,
		cause: options.cause,
		metadata: options.metadata,
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isProviderError = (
	value: unknown,
): value is SimseError & { readonly provider: string } =>
	isSimseError(value) && value.code.startsWith('PROVIDER_');

export const isProviderUnavailableError = (
	value: unknown,
): value is SimseError & { readonly provider: string } =>
	isSimseError(value) && value.code === 'PROVIDER_UNAVAILABLE';

export const isProviderTimeoutError = (
	value: unknown,
): value is SimseError & {
	readonly provider: string;
	readonly timeoutMs: number;
} => isSimseError(value) && value.code === 'PROVIDER_TIMEOUT';

export const isProviderGenerationError = (
	value: unknown,
): value is SimseError & { readonly provider: string } =>
	isSimseError(value) && value.code === 'PROVIDER_GENERATION_FAILED';

export const isProviderHTTPError = (
	value: unknown,
): value is SimseError & { readonly provider: string } =>
	isSimseError(value) && value.code === 'PROVIDER_HTTP_ERROR';
