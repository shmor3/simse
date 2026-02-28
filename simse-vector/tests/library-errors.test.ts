import { describe, expect, it } from 'bun:test';
import {
	createEmbeddingError,
	createLibraryError,
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	isEmbeddingError,
	isLibraryError,
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
} from '../src/errors.js';

describe('Library errors', () => {
	it('createLibraryError creates a LIBRARY_ERROR', () => {
		const err = createLibraryError('test');
		expect(err.name).toBe('LibraryError');
		expect(err.code).toBe('LIBRARY_ERROR');
		expect(isLibraryError(err)).toBe(true);
	});

	it('createStacksCorruptionError creates a STACKS_CORRUPT', () => {
		const err = createStacksCorruptionError('path/to/store');
		expect(err.code).toBe('STACKS_CORRUPT');
		expect(isStacksCorruptionError(err)).toBe(true);
		expect(isLibraryError(err)).toBe(true);
	});

	it('createEmbeddingError still works', () => {
		const err = createEmbeddingError('embed failed');
		expect(err.code).toBe('EMBEDDING_ERROR');
		expect(isEmbeddingError(err)).toBe(true);
		expect(isLibraryError(err)).toBe(true);
	});

	it('isStacksError matches STACKS_ codes', () => {
		const err = createLibraryError('test', { code: 'STACKS_NOT_LOADED' });
		expect(isStacksError(err)).toBe(true);
	});

	it('createStacksError creates a STACKS_ERROR', () => {
		const err = createStacksError('test');
		expect(err.name).toBe('StacksError');
		expect(err.code).toBe('STACKS_ERROR');
		expect(isStacksError(err)).toBe(true);
		expect(isLibraryError(err)).toBe(true);
	});

	it('createStacksIOError creates a STACKS_IO', () => {
		const err = createStacksIOError('path/to/store', 'read');
		expect(err.code).toBe('STACKS_IO');
		expect(isStacksIOError(err)).toBe(true);
		expect(isLibraryError(err)).toBe(true);
		expect(err.storePath).toBe('path/to/store');
	});
});
