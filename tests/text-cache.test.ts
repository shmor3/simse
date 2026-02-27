import { describe, expect, it } from 'bun:test';
import { createTextCache } from '../src/ai/library/text-cache.js';

describe('createTextCache', () => {
	it('returns a frozen object', () => {
		const cache = createTextCache();
		expect(Object.isFrozen(cache)).toBe(true);
	});

	it('stores and retrieves text by ID', () => {
		const cache = createTextCache();
		cache.put('a', 'hello world');
		expect(cache.get('a')).toBe('hello world');
	});

	it('returns undefined for cache misses', () => {
		const cache = createTextCache();
		expect(cache.get('missing')).toBeUndefined();
	});

	it('tracks size correctly', () => {
		const cache = createTextCache();
		expect(cache.size).toBe(0);
		cache.put('a', 'one');
		cache.put('b', 'two');
		expect(cache.size).toBe(2);
	});

	it('evicts LRU entries when maxEntries exceeded', () => {
		const cache = createTextCache({ maxEntries: 2 });
		cache.put('a', 'first');
		cache.put('b', 'second');
		cache.put('c', 'third'); // should evict 'a'
		expect(cache.get('a')).toBeUndefined();
		expect(cache.get('b')).toBe('second');
		expect(cache.get('c')).toBe('third');
		expect(cache.size).toBe(2);
	});

	it('promotes accessed entries to most-recently-used', () => {
		const cache = createTextCache({ maxEntries: 2 });
		cache.put('a', 'first');
		cache.put('b', 'second');
		cache.get('a'); // promote 'a'
		cache.put('c', 'third'); // should evict 'b', not 'a'
		expect(cache.get('a')).toBe('first');
		expect(cache.get('b')).toBeUndefined();
		expect(cache.get('c')).toBe('third');
	});

	it('evicts when maxBytes exceeded', () => {
		const cache = createTextCache({ maxBytes: 20 });
		cache.put('a', 'aaaaaaaaaa'); // 10 bytes
		cache.put('b', 'bbbbbbbbbb'); // 10 bytes
		expect(cache.size).toBe(2);
		cache.put('c', 'cccccccccc'); // 10 bytes, should evict 'a'
		expect(cache.get('a')).toBeUndefined();
		expect(cache.size).toBe(2);
	});

	it('updates existing entry in place', () => {
		const cache = createTextCache();
		cache.put('a', 'old');
		cache.put('a', 'new');
		expect(cache.get('a')).toBe('new');
		expect(cache.size).toBe(1);
	});

	it('removes entries', () => {
		const cache = createTextCache();
		cache.put('a', 'hello');
		expect(cache.remove('a')).toBe(true);
		expect(cache.get('a')).toBeUndefined();
		expect(cache.size).toBe(0);
	});

	it('returns false when removing non-existent entry', () => {
		const cache = createTextCache();
		expect(cache.remove('missing')).toBe(false);
	});

	it('clears all entries', () => {
		const cache = createTextCache();
		cache.put('a', 'one');
		cache.put('b', 'two');
		cache.clear();
		expect(cache.size).toBe(0);
		expect(cache.bytes).toBe(0);
		expect(cache.get('a')).toBeUndefined();
	});

	it('tracks bytes correctly', () => {
		const cache = createTextCache();
		cache.put('a', 'hi'); // 2 bytes
		expect(cache.bytes).toBe(2);
		cache.put('b', 'hey'); // 3 bytes
		expect(cache.bytes).toBe(5);
		cache.remove('a');
		expect(cache.bytes).toBe(3);
	});

	it('uses default options when none provided', () => {
		const cache = createTextCache();
		// Should not throw for reasonable usage
		for (let i = 0; i < 100; i++) {
			cache.put(`key-${i}`, `value-${i}`);
		}
		expect(cache.size).toBe(100);
	});
});
