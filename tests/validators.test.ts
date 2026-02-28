import { describe, expect, it } from 'bun:test';
import type { VFSSnapshot } from 'simse-vfs';
import {
	createDefaultValidators,
	createEmptyFileValidator,
	createJSONSyntaxValidator,
	createMissingTrailingNewlineValidator,
	createMixedIndentationValidator,
	createMixedLineEndingsValidator,
	createTrailingWhitespaceValidator,
	validateSnapshot,
} from 'simse-vfs';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSnapshot(
	files: { path: string; text: string; contentType?: 'text' | 'binary' }[],
): VFSSnapshot {
	return {
		files: files.map((f) => ({
			path: f.path,
			contentType: f.contentType ?? 'text',
			text: f.contentType === 'binary' ? undefined : f.text,
			base64: f.contentType === 'binary' ? f.text : undefined,
			createdAt: Date.now(),
			modifiedAt: Date.now(),
		})),
		directories: [],
	};
}

// ---------------------------------------------------------------------------
// JSON Syntax Validator
// ---------------------------------------------------------------------------

describe('createJSONSyntaxValidator', () => {
	const validator = createJSONSyntaxValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('has correct name and extensions', () => {
		expect(validator.name).toBe('json-syntax');
		expect(validator.extensions).toContain('.json');
	});

	it('passes valid JSON', () => {
		const issues = validator.validate('/data.json', '{"key": "value"}');
		expect(issues.length).toBe(0);
	});

	it('detects invalid JSON', () => {
		const issues = validator.validate('/bad.json', '{invalid}');
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('error');
		expect(issues[0].code).toBe('JSON_SYNTAX_ERROR');
		expect(issues[0].path).toBe('/bad.json');
	});

	it('passes empty JSON object', () => {
		const issues = validator.validate('/empty.json', '{}');
		expect(issues.length).toBe(0);
	});

	it('passes JSON array', () => {
		const issues = validator.validate('/arr.json', '[1, 2, 3]');
		expect(issues.length).toBe(0);
	});
});

// ---------------------------------------------------------------------------
// Trailing Whitespace Validator
// ---------------------------------------------------------------------------

describe('createTrailingWhitespaceValidator', () => {
	const validator = createTrailingWhitespaceValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('passes clean text', () => {
		const issues = validator.validate(
			'/clean.ts',
			'const x = 1;\nconst y = 2;\n',
		);
		expect(issues.length).toBe(0);
	});

	it('detects trailing spaces', () => {
		const issues = validator.validate(
			'/spaces.ts',
			'const x = 1;   \nconst y = 2;\n',
		);
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('warning');
		expect(issues[0].code).toBe('TRAILING_WHITESPACE');
		expect(issues[0].line).toBe(1);
	});

	it('detects trailing tabs', () => {
		const issues = validator.validate(
			'/tabs.ts',
			'const x = 1;\t\nconst y = 2;\n',
		);
		expect(issues.length).toBe(1);
		expect(issues[0].line).toBe(1);
	});

	it('reports multiple lines with trailing whitespace', () => {
		const issues = validator.validate('/multi.ts', 'a  \nb\nc  \n');
		expect(issues.length).toBe(2);
		expect(issues[0].line).toBe(1);
		expect(issues[1].line).toBe(3);
	});
});

// ---------------------------------------------------------------------------
// Mixed Indentation Validator
// ---------------------------------------------------------------------------

describe('createMixedIndentationValidator', () => {
	const validator = createMixedIndentationValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('passes tabs-only indentation', () => {
		const issues = validator.validate(
			'/tabs.ts',
			'\tconst x = 1;\n\tconst y = 2;\n',
		);
		expect(issues.length).toBe(0);
	});

	it('passes spaces-only indentation', () => {
		const issues = validator.validate(
			'/spaces.ts',
			'  const x = 1;\n  const y = 2;\n',
		);
		expect(issues.length).toBe(0);
	});

	it('detects mixed tabs and spaces', () => {
		const issues = validator.validate(
			'/mixed.ts',
			'\tconst x = 1;\n  const y = 2;\n',
		);
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('error');
		expect(issues[0].code).toBe('MIXED_INDENTATION');
	});

	it('ignores single-space lines (not indentation)', () => {
		const issues = validator.validate(
			'/single.ts',
			'\tconst x = 1;\n something\n',
		);
		expect(issues.length).toBe(0);
	});

	it('ignores empty lines', () => {
		const issues = validator.validate('/empty.ts', '\tline1\n\n\tline2\n');
		expect(issues.length).toBe(0);
	});
});

// ---------------------------------------------------------------------------
// Empty File Validator
// ---------------------------------------------------------------------------

describe('createEmptyFileValidator', () => {
	const validator = createEmptyFileValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('passes non-empty file', () => {
		const issues = validator.validate('/content.ts', 'export {};');
		expect(issues.length).toBe(0);
	});

	it('detects empty file', () => {
		const issues = validator.validate('/empty.ts', '');
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('warning');
		expect(issues[0].code).toBe('EMPTY_FILE');
	});

	it('detects whitespace-only file', () => {
		const issues = validator.validate('/ws.ts', '   \n\t\n  ');
		expect(issues.length).toBe(1);
		expect(issues[0].code).toBe('EMPTY_FILE');
	});

	it('has no extension filter (applies to all files)', () => {
		expect(validator.extensions).toBeUndefined();
	});
});

// ---------------------------------------------------------------------------
// Mixed Line Endings Validator
// ---------------------------------------------------------------------------

describe('createMixedLineEndingsValidator', () => {
	const validator = createMixedLineEndingsValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('passes LF-only text', () => {
		const issues = validator.validate('/lf.ts', 'line1\nline2\nline3\n');
		expect(issues.length).toBe(0);
	});

	it('passes CRLF-only text', () => {
		const issues = validator.validate(
			'/crlf.ts',
			'line1\r\nline2\r\nline3\r\n',
		);
		expect(issues.length).toBe(0);
	});

	it('detects mixed line endings', () => {
		const issues = validator.validate('/mixed.ts', 'line1\r\nline2\nline3\n');
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('warning');
		expect(issues[0].code).toBe('MIXED_LINE_ENDINGS');
	});
});

// ---------------------------------------------------------------------------
// Missing Trailing Newline Validator
// ---------------------------------------------------------------------------

describe('createMissingTrailingNewlineValidator', () => {
	const validator = createMissingTrailingNewlineValidator();

	it('returns frozen object', () => {
		expect(Object.isFrozen(validator)).toBe(true);
	});

	it('passes file ending with newline', () => {
		const issues = validator.validate('/good.ts', 'const x = 1;\n');
		expect(issues.length).toBe(0);
	});

	it('detects file missing trailing newline', () => {
		const issues = validator.validate('/bad.ts', 'const x = 1;');
		expect(issues.length).toBe(1);
		expect(issues[0].severity).toBe('warning');
		expect(issues[0].code).toBe('MISSING_TRAILING_NEWLINE');
	});

	it('passes empty file', () => {
		const issues = validator.validate('/empty.ts', '');
		expect(issues.length).toBe(0);
	});

	it('has file extension filter', () => {
		expect(validator.extensions).toBeDefined();
		expect(validator.extensions).toContain('.ts');
		expect(validator.extensions).toContain('.js');
	});
});

// ---------------------------------------------------------------------------
// createDefaultValidators
// ---------------------------------------------------------------------------

describe('createDefaultValidators', () => {
	it('returns frozen array', () => {
		const validators = createDefaultValidators();
		expect(Object.isFrozen(validators)).toBe(true);
	});

	it('includes all 6 built-in validators', () => {
		const validators = createDefaultValidators();
		expect(validators.length).toBe(6);
	});

	it('validators have unique names', () => {
		const validators = createDefaultValidators();
		const names = validators.map((v) => v.name);
		expect(new Set(names).size).toBe(names.length);
	});
});

// ---------------------------------------------------------------------------
// validateSnapshot
// ---------------------------------------------------------------------------

describe('validateSnapshot', () => {
	it('passes clean snapshot', () => {
		const snap = makeSnapshot([
			{ path: '/index.ts', text: 'const x = 1;\n' },
			{ path: '/data.json', text: '{"key": "value"}\n' },
		]);

		const result = validateSnapshot(snap);
		expect(result.passed).toBe(true);
		expect(result.errors).toBe(0);
		expect(result.warnings).toBe(0);
		expect(result.issues.length).toBe(0);
	});

	it('returns frozen result', () => {
		const snap = makeSnapshot([{ path: '/f.ts', text: 'ok\n' }]);
		const result = validateSnapshot(snap);
		expect(Object.isFrozen(result)).toBe(true);
		expect(Object.isFrozen(result.issues)).toBe(true);
	});

	it('detects errors in JSON files', () => {
		const snap = makeSnapshot([
			{ path: '/bad.json', text: '{not valid json}' },
		]);

		const result = validateSnapshot(snap);
		expect(result.passed).toBe(false);
		expect(result.errors).toBeGreaterThanOrEqual(1);
	});

	it('collects warnings without failing', () => {
		const snap = makeSnapshot([{ path: '/warn.ts', text: 'code  \n' }]);

		const result = validateSnapshot(snap);
		expect(result.passed).toBe(true);
		expect(result.warnings).toBeGreaterThanOrEqual(1);
	});

	it('skips binary files', () => {
		const snap = makeSnapshot([
			{ path: '/image.png', text: 'not-real-png', contentType: 'binary' },
		]);

		const result = validateSnapshot(snap);
		expect(result.passed).toBe(true);
		expect(result.issues.length).toBe(0);
	});

	it('uses default validators when none provided', () => {
		const snap = makeSnapshot([{ path: '/bad.json', text: '{invalid}' }]);

		const result = validateSnapshot(snap);
		expect(result.errors).toBeGreaterThanOrEqual(1);
	});

	it('uses custom validators when provided', () => {
		const customValidator = {
			name: 'custom',
			validate: (path: string, _text: string) => [
				{
					path,
					severity: 'error' as const,
					code: 'CUSTOM',
					message: 'Custom error',
				},
			],
		};

		const snap = makeSnapshot([{ path: '/any.txt', text: 'anything' }]);

		const result = validateSnapshot(snap, [customValidator]);
		expect(result.passed).toBe(false);
		expect(result.errors).toBe(1);
		expect(result.issues[0].code).toBe('CUSTOM');
	});

	it('filters validators by extension', () => {
		const tsOnly = {
			name: 'ts-only',
			extensions: ['.ts'] as readonly string[],
			validate: (path: string, _text: string) => [
				{
					path,
					severity: 'warning' as const,
					code: 'TS_ONLY',
					message: 'Only TS files',
				},
			],
		};

		const snap = makeSnapshot([
			{ path: '/code.ts', text: 'ts code\n' },
			{ path: '/readme.md', text: 'markdown\n' },
		]);

		const result = validateSnapshot(snap, [tsOnly]);
		expect(result.issues.length).toBe(1);
		expect(result.issues[0].path).toBe('/code.ts');
	});

	it('handles empty snapshot', () => {
		const snap: VFSSnapshot = { files: [], directories: [] };
		const result = validateSnapshot(snap);
		expect(result.passed).toBe(true);
		expect(result.issues.length).toBe(0);
	});

	it('reports multiple issues from multiple validators', () => {
		const snap = makeSnapshot([{ path: '/bad.json', text: '{invalid}  ' }]);

		const result = validateSnapshot(snap);
		// Should have at least JSON error + trailing whitespace warning
		expect(result.issues.length).toBeGreaterThanOrEqual(2);
	});
});
