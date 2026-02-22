// ---------------------------------------------------------------------------
// Configuration Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createConfigError = (
	message: string,
	options: {
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: 'ConfigError',
		code: options.code ?? 'CONFIG_ERROR',
		statusCode: 400,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createConfigNotFoundError = (
	configPath: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createSimseError(`Configuration file not found: ${configPath}`, {
		name: 'ConfigNotFoundError',
		code: 'CONFIG_NOT_FOUND',
		statusCode: 400,
		cause: options.cause,
		metadata: { configPath },
	});

export const createConfigValidationError = (
	issues: Array<{ path: string; message: string }>,
	options: { cause?: unknown } = {},
): SimseError & {
	readonly issues: ReadonlyArray<{ path: string; message: string }>;
} => {
	const summary =
		issues.length === 1
			? issues[0].message
			: `${issues.length} validation errors`;

	const frozenIssues = Object.freeze([...issues]);

	const err = createSimseError(`Invalid configuration: ${summary}`, {
		name: 'ConfigValidationError',
		code: 'CONFIG_VALIDATION',
		statusCode: 400,
		cause: options.cause,
		metadata: { issues: frozenIssues },
	}) as SimseError & {
		readonly issues: ReadonlyArray<{ path: string; message: string }>;
	};

	Object.defineProperty(err, 'issues', {
		value: frozenIssues,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createConfigParseError = (
	configPath: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createSimseError(`Failed to parse configuration file: ${configPath}`, {
		name: 'ConfigParseError',
		code: 'CONFIG_PARSE',
		statusCode: 400,
		cause: options.cause,
		metadata: { configPath },
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isConfigError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('CONFIG_');

export const isConfigNotFoundError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'CONFIG_NOT_FOUND';

export const isConfigValidationError = (
	value: unknown,
): value is SimseError & {
	readonly issues: ReadonlyArray<{ path: string; message: string }>;
} => isSimseError(value) && value.code === 'CONFIG_VALIDATION';

export const isConfigParseError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'CONFIG_PARSE';
