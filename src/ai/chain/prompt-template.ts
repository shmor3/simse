// ---------------------------------------------------------------------------
// Prompt Template — template creation, formatting, and type guard
// ---------------------------------------------------------------------------

import {
	createTemplateError,
	createTemplateMissingVariablesError,
} from '../../errors/index.js';

export interface PromptTemplate {
	format(values: Record<string, string>): string;
	getVariables(): string[];
	readonly hasVariables: boolean;
	readonly raw: string;
	readonly _isPromptTemplate: true;
}

export function createPromptTemplate(template: string): PromptTemplate {
	if (template.length === 0) {
		throw createTemplateError('Template string cannot be empty', {
			code: 'TEMPLATE_EMPTY',
		});
	}

	// Extract unique variable names from {variableName} placeholders
	const variables = [
		...new Set(
			[...template.matchAll(/\{([\w-]+)\}/g)].map((m) => {
				const varName = m[1];
				if (!varName) {
					throw createTemplateError(
						'Unexpected empty match in template variable extraction',
						{ code: 'TEMPLATE_PARSE_ERROR' },
					);
				}
				return varName;
			}),
		),
	];

	return Object.freeze({
		format(values: Record<string, string>): string {
			const missing = variables.filter((v) => !(v in values));
			if (missing.length > 0) {
				throw createTemplateMissingVariablesError(missing, {
					template,
				});
			}

			let result = template;
			for (const varName of variables) {
				const value = values[varName];
				if (typeof value !== 'string') {
					throw createTemplateError(
						`Template variable "${varName}" must be a string, got ${typeof value}`,
						{ code: 'TEMPLATE_INVALID_VALUE' },
					);
				}
				result = result.replaceAll(`{${varName}}`, value);
			}
			return result;
		},
		getVariables(): string[] {
			return [...variables];
		},
		get hasVariables(): boolean {
			return variables.length > 0;
		},
		get raw(): string {
			return template;
		},
		_isPromptTemplate: true as const,
	});
}

export function isPromptTemplate(value: unknown): value is PromptTemplate {
	const obj = value as Record<string, unknown>;
	return (
		typeof value === 'object' &&
		value !== null &&
		obj._isPromptTemplate === true &&
		typeof obj.format === 'function' &&
		typeof obj.getVariables === 'function'
	);
}
