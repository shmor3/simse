// ---------------------------------------------------------------------------
// Librarian Definition — validation, topic matching, and persistence for
// configurable librarian JSON definitions.
// ---------------------------------------------------------------------------

import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import picomatch from 'picomatch';
import type { LibrarianDefinition } from './types.js';

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

export interface ValidationResult {
	readonly valid: boolean;
	readonly errors: readonly string[];
}

const NAME_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Validates a plain object against the LibrarianDefinition schema.
 */
export function validateDefinition(input: unknown): ValidationResult {
	const errors: string[] = [];

	if (!isRecord(input)) {
		return Object.freeze({ valid: false, errors: ['input must be an object'] });
	}

	// name
	if (typeof input.name !== 'string' || input.name.length === 0) {
		errors.push('name must be a non-empty string');
	} else if (!NAME_PATTERN.test(input.name)) {
		errors.push(
			'name must be kebab-case (lowercase alphanumeric and hyphens, starting with alphanumeric)',
		);
	}

	// description
	if (typeof input.description !== 'string' || input.description.length === 0) {
		errors.push('description must be a non-empty string');
	}

	// purpose
	if (typeof input.purpose !== 'string' || input.purpose.length === 0) {
		errors.push('purpose must be a non-empty string');
	}

	// topics
	if (!Array.isArray(input.topics) || input.topics.length === 0) {
		errors.push('topics must be a non-empty array of strings');
	} else if (!input.topics.every((t: unknown) => typeof t === 'string')) {
		errors.push('topics must be a non-empty array of strings');
	}

	// permissions
	if (!isRecord(input.permissions)) {
		errors.push(
			'permissions must be an object with boolean add, delete, reorganize',
		);
	} else {
		if (typeof input.permissions.add !== 'boolean') {
			errors.push('permissions.add must be a boolean');
		}
		if (typeof input.permissions.delete !== 'boolean') {
			errors.push('permissions.delete must be a boolean');
		}
		if (typeof input.permissions.reorganize !== 'boolean') {
			errors.push('permissions.reorganize must be a boolean');
		}
	}

	// thresholds
	if (!isRecord(input.thresholds)) {
		errors.push(
			'thresholds must be an object with positive numbers topicComplexity, escalateAt',
		);
	} else {
		if (
			typeof input.thresholds.topicComplexity !== 'number' ||
			input.thresholds.topicComplexity <= 0
		) {
			errors.push('thresholds.topicComplexity must be a positive number');
		}
		if (
			typeof input.thresholds.escalateAt !== 'number' ||
			input.thresholds.escalateAt <= 0
		) {
			errors.push('thresholds.escalateAt must be a positive number');
		}
	}

	// acp (optional)
	if (input.acp !== undefined) {
		if (!isRecord(input.acp)) {
			errors.push('acp must be an object with a non-empty command string');
		} else if (
			typeof input.acp.command !== 'string' ||
			input.acp.command.length === 0
		) {
			errors.push('acp.command must be a non-empty string');
		}
	}

	return Object.freeze({ valid: errors.length === 0, errors });
}

// ---------------------------------------------------------------------------
// Topic Matching
// ---------------------------------------------------------------------------

/**
 * Check if a topic matches any of the given glob patterns using picomatch.
 *
 * - `*` matches everything at one level.
 * - `code/*` matches `code/react`.
 * - `code/**` matches `code/react/hooks`.
 */
export function matchesTopic(
	patterns: readonly string[],
	topic: string,
): boolean {
	return picomatch.isMatch(topic, patterns as string[]);
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/**
 * Saves a librarian definition to `<librariansDir>/<name>.json`.
 * Creates the directory if it does not exist.
 */
export async function saveDefinition(
	librariansDir: string,
	definition: LibrarianDefinition,
): Promise<void> {
	await mkdir(librariansDir, { recursive: true });
	const filePath = join(librariansDir, `${definition.name}.json`);
	await writeFile(
		filePath,
		`${JSON.stringify(definition, null, '\t')}\n`,
		'utf-8',
	);
}

/**
 * Loads a single librarian definition from `<librariansDir>/<name>.json`.
 * Returns `undefined` if the file does not exist or fails validation.
 */
export async function loadDefinition(
	librariansDir: string,
	name: string,
): Promise<LibrarianDefinition | undefined> {
	const filePath = join(librariansDir, `${name}.json`);
	try {
		const raw = await readFile(filePath, 'utf-8');
		const parsed: unknown = JSON.parse(raw);
		const result = validateDefinition(parsed);
		if (!result.valid) {
			return undefined;
		}
		return parsed as LibrarianDefinition;
	} catch {
		return undefined;
	}
}

/**
 * Loads all valid librarian definitions from `<librariansDir>`.
 * Reads every `.json` file, validates each, and returns only the valid ones.
 * Returns an empty array if the directory does not exist.
 */
export async function loadAllDefinitions(
	librariansDir: string,
): Promise<LibrarianDefinition[]> {
	let entries: string[];
	try {
		entries = await readdir(librariansDir);
	} catch {
		return [];
	}

	const jsonFiles = entries.filter((f) => f.endsWith('.json'));
	const definitions: LibrarianDefinition[] = [];

	for (const file of jsonFiles) {
		const filePath = join(librariansDir, file);
		try {
			const raw = await readFile(filePath, 'utf-8');
			const parsed: unknown = JSON.parse(raw);
			const result = validateDefinition(parsed);
			if (result.valid) {
				definitions.push(parsed as LibrarianDefinition);
			}
		} catch {
			// Skip invalid files
		}
	}

	return definitions;
}
