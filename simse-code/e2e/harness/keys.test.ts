import { describe, expect, it } from 'bun:test';
import { ctrlKey, KEYS } from './keys.js';

describe('keys', () => {
	it('has enter as carriage return', () => {
		expect(KEYS.enter).toBe('\r');
	});

	it('has escape as 0x1b', () => {
		expect(KEYS.escape).toBe('\x1b');
	});

	it('has arrow keys as ANSI sequences', () => {
		expect(KEYS.up).toBe('\x1b[A');
		expect(KEYS.down).toBe('\x1b[B');
		expect(KEYS.right).toBe('\x1b[C');
		expect(KEYS.left).toBe('\x1b[D');
	});

	it('has backspace', () => {
		expect(KEYS.backspace).toBe('\x7f');
	});

	it('has tab', () => {
		expect(KEYS.tab).toBe('\t');
	});

	it('ctrlKey produces control characters', () => {
		expect(ctrlKey('c')).toBe('\x03');
		expect(ctrlKey('d')).toBe('\x04');
		expect(ctrlKey('a')).toBe('\x01');
		expect(ctrlKey('z')).toBe('\x1a');
	});
});
