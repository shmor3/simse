// ---------------------------------------------------------------------------
// Template Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createTemplateError = (
	message: string,
	options: {
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: 'TemplateError',
		code: options.code ?? 'TEMPLATE_ERROR',
		statusCode: 400,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createTemplateMissingVariablesError = (
	missingVariables: string[],
	options: { template?: string } = {},
): SimseError & { readonly missingVariables: readonly string[] } => {
	const frozenVars = Object.freeze([...missingVariables]);

	const err = createSimseError(
		`Missing template variable${missingVariables.length > 1 ? 's' : ''}: ${missingVariables.join(', ')}`,
		{
			name: 'TemplateMissingVariablesError',
			code: 'TEMPLATE_MISSING_VARS',
			statusCode: 400,
			metadata: {
				missingVariables: frozenVars,
				...(options.template ? { template: options.template } : {}),
			},
		},
	) as SimseError & { readonly missingVariables: readonly string[] };

	Object.defineProperty(err, 'missingVariables', {
		value: frozenVars,
		writable: false,
		enumerable: true,
	});

	return err;
};

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isTemplateError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('TEMPLATE_');

export const isTemplateMissingVariablesError = (
	value: unknown,
): value is SimseError & { readonly missingVariables: readonly string[] } =>
	isSimseError(value) && value.code === 'TEMPLATE_MISSING_VARS';
