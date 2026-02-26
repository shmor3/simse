/**
 * SimSE Code — Safe JSON I/O
 *
 * Read/write JSON files with error handling. No external deps.
 */

import {
	appendFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import { dirname } from 'node:path';

/**
 * Read and parse a JSON file. Returns undefined if the file doesn't exist
 * or can't be parsed.
 */
export function readJsonFile<T>(path: string): T | undefined {
	try {
		if (!existsSync(path)) return undefined;
		const raw = readFileSync(path, 'utf-8');
		return JSON.parse(raw) as T;
	} catch {
		return undefined;
	}
}

/**
 * Write a value as JSON to a file. Creates parent directories if needed.
 */
export function writeJsonFile(path: string, data: unknown): void {
	mkdirSync(dirname(path), { recursive: true });
	writeFileSync(path, JSON.stringify(data, null, '\t'), 'utf-8');
}

/**
 * Append a JSON line to a file (for JSONL/append-only logs).
 * Creates parent directories if needed.
 */
export function appendJsonLine(path: string, data: unknown): void {
	mkdirSync(dirname(path), { recursive: true });
	appendFileSync(path, `${JSON.stringify(data)}\n`, 'utf-8');
}

/**
 * Read a JSONL file, returning an array of parsed lines.
 * Returns empty array if the file doesn't exist or can't be read.
 */
export function readJsonLines<T>(path: string): T[] {
	try {
		if (!existsSync(path)) return [];
		const raw = readFileSync(path, 'utf-8');
		return raw
			.split('\n')
			.filter((line) => line.trim().length > 0)
			.map((line) => JSON.parse(line) as T);
	} catch {
		return [];
	}
}
