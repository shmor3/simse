// ---------------------------------------------------------------------------
// VFS Validators — pre-commit content validation
// ---------------------------------------------------------------------------

import type { VFSSnapshot } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface VFSValidationIssue {
	readonly path: string;
	readonly severity: 'error' | 'warning';
	readonly code: string;
	readonly message: string;
	readonly line?: number;
}

export interface VFSValidator {
	readonly name: string;
	readonly extensions?: readonly string[];
	readonly validate: (
		path: string,
		text: string,
	) => readonly VFSValidationIssue[];
}

export interface VFSValidationResult {
	readonly issues: readonly VFSValidationIssue[];
	readonly errors: number;
	readonly warnings: number;
	readonly passed: boolean;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const getExtension = (path: string): string => {
	const lastDot = path.lastIndexOf('.');
	if (lastDot === -1) return '';
	return path.slice(lastDot).toLowerCase();
};

const matchesExtension = (
	path: string,
	extensions: readonly string[] | undefined,
): boolean => {
	if (!extensions || extensions.length === 0) return true;
	const ext = getExtension(path);
	return extensions.includes(ext);
};

// ---------------------------------------------------------------------------
// Built-in validators
// ---------------------------------------------------------------------------

export function createJSONSyntaxValidator(): VFSValidator {
	return Object.freeze({
		name: 'json-syntax',
		extensions: ['.json'],
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			try {
				JSON.parse(text);
				return [];
			} catch (err) {
				return Object.freeze([
					Object.freeze({
						path,
						severity: 'error' as const,
						code: 'JSON_SYNTAX_ERROR',
						message: `Invalid JSON: ${err instanceof Error ? err.message : String(err)}`,
					}),
				]);
			}
		},
	});
}

export function createTrailingWhitespaceValidator(): VFSValidator {
	return Object.freeze({
		name: 'trailing-whitespace',
		extensions: [
			'.ts',
			'.js',
			'.json',
			'.md',
			'.txt',
			'.html',
			'.css',
			'.yaml',
			'.yml',
		],
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			const issues: VFSValidationIssue[] = [];
			const lines = text.split('\n');
			for (let i = 0; i < lines.length; i++) {
				if (/[ \t]+$/.test(lines[i])) {
					issues.push(
						Object.freeze({
							path,
							severity: 'warning' as const,
							code: 'TRAILING_WHITESPACE',
							message: 'Trailing whitespace',
							line: i + 1,
						}),
					);
				}
			}
			return Object.freeze(issues);
		},
	});
}

export function createMixedIndentationValidator(): VFSValidator {
	return Object.freeze({
		name: 'mixed-indentation',
		extensions: ['.ts', '.js', '.json', '.html', '.css'],
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			const lines = text.split('\n');
			let hasTabs = false;
			let hasSpaces = false;

			for (const line of lines) {
				if (line.length === 0) continue;
				if (line[0] === '\t') hasTabs = true;
				if (line[0] === ' ' && line.length > 1 && line[1] === ' ')
					hasSpaces = true;
				if (hasTabs && hasSpaces) break;
			}

			if (hasTabs && hasSpaces) {
				return Object.freeze([
					Object.freeze({
						path,
						severity: 'error' as const,
						code: 'MIXED_INDENTATION',
						message: 'File mixes tabs and spaces for indentation',
					}),
				]);
			}
			return [];
		},
	});
}

export function createEmptyFileValidator(): VFSValidator {
	return Object.freeze({
		name: 'empty-file',
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			if (text.trim().length === 0) {
				return Object.freeze([
					Object.freeze({
						path,
						severity: 'warning' as const,
						code: 'EMPTY_FILE',
						message: 'File is empty or contains only whitespace',
					}),
				]);
			}
			return [];
		},
	});
}

export function createMixedLineEndingsValidator(): VFSValidator {
	return Object.freeze({
		name: 'mixed-line-endings',
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			const hasCRLF = text.includes('\r\n');
			const hasLF = /(?<!\r)\n/.test(text);

			if (hasCRLF && hasLF) {
				return Object.freeze([
					Object.freeze({
						path,
						severity: 'warning' as const,
						code: 'MIXED_LINE_ENDINGS',
						message: 'File has mixed line endings (CRLF and LF)',
					}),
				]);
			}
			return [];
		},
	});
}

export function createMissingTrailingNewlineValidator(): VFSValidator {
	return Object.freeze({
		name: 'missing-trailing-newline',
		extensions: [
			'.ts',
			'.js',
			'.json',
			'.md',
			'.txt',
			'.html',
			'.css',
			'.yaml',
			'.yml',
		],
		validate: (path: string, text: string): readonly VFSValidationIssue[] => {
			if (text.length > 0 && !text.endsWith('\n')) {
				return Object.freeze([
					Object.freeze({
						path,
						severity: 'warning' as const,
						code: 'MISSING_TRAILING_NEWLINE',
						message: 'File does not end with a newline',
					}),
				]);
			}
			return [];
		},
	});
}

// ---------------------------------------------------------------------------
// Default validator set
// ---------------------------------------------------------------------------

export function createDefaultValidators(): readonly VFSValidator[] {
	return Object.freeze([
		createJSONSyntaxValidator(),
		createTrailingWhitespaceValidator(),
		createMixedIndentationValidator(),
		createEmptyFileValidator(),
		createMixedLineEndingsValidator(),
		createMissingTrailingNewlineValidator(),
	]);
}

// ---------------------------------------------------------------------------
// Validate snapshot
// ---------------------------------------------------------------------------

export function validateSnapshot(
	snapshot: VFSSnapshot,
	validators?: readonly VFSValidator[],
): VFSValidationResult {
	const validatorList = validators ?? createDefaultValidators();
	const issues: VFSValidationIssue[] = [];

	for (const file of snapshot.files) {
		if (file.contentType !== 'text' || file.text === undefined) continue;

		for (const validator of validatorList) {
			if (!matchesExtension(file.path, validator.extensions)) continue;
			const fileIssues = validator.validate(file.path, file.text);
			for (const issue of fileIssues) {
				issues.push(issue);
			}
		}
	}

	const errors = issues.filter((i) => i.severity === 'error').length;
	const warnings = issues.length - errors;

	return Object.freeze({
		issues: Object.freeze(issues),
		errors,
		warnings,
		passed: errors === 0,
	});
}
