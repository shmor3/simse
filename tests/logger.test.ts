import { beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import {
	createConsoleTransport,
	createLogger,
	createMemoryTransport,
	getDefaultLogger,
	type LogEntry,
	type Logger,
	type LogTransport,
	type MemoryTransportHandle,
	setDefaultLogger,
} from '../src/logger.js';
import { isTransientError, retry, sleep } from '../src/utils/retry.js';

// ===========================================================================
// createMemoryTransport
// ===========================================================================

describe('createMemoryTransport', () => {
	it('should store written entries', () => {
		const transport = createMemoryTransport();
		const entry: LogEntry = {
			level: 'info',
			message: 'test message',
			timestamp: '2024-01-01T00:00:00.000Z',
		};

		transport.write(entry);

		expect(transport.entries).toHaveLength(1);
		expect(transport.entries[0]).toBe(entry);
	});

	it('should accumulate multiple entries', () => {
		const transport = createMemoryTransport();

		transport.write({ level: 'info', message: 'one', timestamp: 't1' });
		transport.write({ level: 'warn', message: 'two', timestamp: 't2' });
		transport.write({ level: 'error', message: 'three', timestamp: 't3' });

		expect(transport.entries).toHaveLength(3);
	});

	it('should clear all entries', () => {
		const transport = createMemoryTransport();

		transport.write({ level: 'info', message: 'a', timestamp: 't' });
		transport.write({ level: 'warn', message: 'b', timestamp: 't' });

		transport.clear();

		expect(transport.entries).toHaveLength(0);
	});

	it('should filter entries by level', () => {
		const transport = createMemoryTransport();

		transport.write({ level: 'debug', message: 'd', timestamp: 't' });
		transport.write({ level: 'info', message: 'i1', timestamp: 't' });
		transport.write({ level: 'warn', message: 'w', timestamp: 't' });
		transport.write({ level: 'info', message: 'i2', timestamp: 't' });
		transport.write({ level: 'error', message: 'e', timestamp: 't' });

		const infos = transport.filter('info');
		expect(infos).toHaveLength(2);
		expect(infos[0].message).toBe('i1');
		expect(infos[1].message).toBe('i2');

		expect(transport.filter('debug')).toHaveLength(1);
		expect(transport.filter('warn')).toHaveLength(1);
		expect(transport.filter('error')).toHaveLength(1);
		expect(transport.filter('none')).toHaveLength(0);
	});
});

// ===========================================================================
// createConsoleTransport
// ===========================================================================

describe('createConsoleTransport', () => {
	it('should call console.error for error level', () => {
		const spy = spyOn(console, 'error').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'error',
			message: 'error msg',
			timestamp: '2024-01-01T00:00:00.000Z',
		});

		expect(spy).toHaveBeenCalledTimes(1);
		spy.mockRestore();
	});

	it('should call console.warn for warn level', () => {
		const spy = spyOn(console, 'warn').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'warn',
			message: 'warn msg',
			timestamp: '2024-01-01T00:00:00.000Z',
		});

		expect(spy).toHaveBeenCalledTimes(1);
		spy.mockRestore();
	});

	it('should call console.debug for debug level', () => {
		const spy = spyOn(console, 'debug').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'debug',
			message: 'debug msg',
			timestamp: '2024-01-01T00:00:00.000Z',
		});

		expect(spy).toHaveBeenCalledTimes(1);
		spy.mockRestore();
	});

	it('should call console.log for info level', () => {
		const spy = spyOn(console, 'log').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'info',
			message: 'info msg',
			timestamp: '2024-01-01T00:00:00.000Z',
		});

		expect(spy).toHaveBeenCalledTimes(1);
		spy.mockRestore();
	});

	it('should include metadata in console output when present', () => {
		const spy = spyOn(console, 'log').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'info',
			message: 'with meta',
			timestamp: '2024-01-01T00:00:00.000Z',
			metadata: { key: 'value' },
		});

		expect(spy).toHaveBeenCalledTimes(1);
		// Should have two arguments: formatted string and metadata
		expect(spy.mock.calls[0].length).toBe(2);
		spy.mockRestore();
	});

	it('should not include metadata when it is empty', () => {
		const spy = spyOn(console, 'log').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'info',
			message: 'no meta',
			timestamp: '2024-01-01T00:00:00.000Z',
			metadata: {},
		});

		expect(spy).toHaveBeenCalledTimes(1);
		// Should have one argument: just the formatted string
		expect(spy.mock.calls[0].length).toBe(1);
		spy.mockRestore();
	});

	it('should include context prefix when present', () => {
		const spy = spyOn(console, 'log').mockImplementation(() => {});
		const transport = createConsoleTransport();

		transport.write({
			level: 'info',
			message: 'context msg',
			timestamp: 't',
			context: 'mymodule',
		});

		const output = spy.mock.calls[0][0] as string;
		expect(output).toContain('[mymodule]');
		spy.mockRestore();
	});
});

// ===========================================================================
// Logger
// ===========================================================================

describe('createLogger', () => {
	let transport: MemoryTransportHandle;
	let logger: Logger;

	beforeEach(() => {
		transport = createMemoryTransport();
		logger = createLogger({
			context: 'test',
			level: 'debug',
			transports: [transport],
		});
	});

	// -----------------------------------------------------------------------
	// Basic logging
	// -----------------------------------------------------------------------

	describe('basic logging', () => {
		it('should log debug messages', () => {
			logger.debug('debug message');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('debug');
			expect(transport.entries[0].message).toBe('debug message');
			expect(transport.entries[0].context).toBe('test');
			expect(transport.entries[0].timestamp).toBeDefined();
		});

		it('should log info messages', () => {
			logger.info('info message');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('info');
			expect(transport.entries[0].message).toBe('info message');
		});

		it('should log warn messages', () => {
			logger.warn('warn message');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('warn');
			expect(transport.entries[0].message).toBe('warn message');
		});

		it('should log error messages', () => {
			logger.error('error message');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('error');
			expect(transport.entries[0].message).toBe('error message');
		});

		it('should include metadata when provided', () => {
			logger.info('with metadata', { key: 'value', count: 42 });

			expect(transport.entries[0].metadata).toEqual({
				key: 'value',
				count: 42,
			});
		});

		it('should not include metadata when not provided', () => {
			logger.info('no metadata');
			expect(transport.entries[0].metadata).toBeUndefined();
		});

		it('should generate ISO timestamps', () => {
			logger.info('timestamp test');

			const timestamp = transport.entries[0].timestamp;
			// Should be a valid ISO string
			expect(() => new Date(timestamp)).not.toThrow();
			expect(new Date(timestamp).toISOString()).toBe(timestamp);
		});
	});

	// -----------------------------------------------------------------------
	// Error logging with Error objects
	// -----------------------------------------------------------------------

	describe('error logging with Error objects', () => {
		it('should extract error details from Error objects', () => {
			const err = new Error('something broke');
			logger.error('failure', err);

			const meta = transport.entries[0].metadata;
			expect(meta).toBeDefined();
			expect(meta?.errorName).toBe('Error');
			expect(meta?.errorMessage).toBe('something broke');
			expect(meta?.stack).toBeDefined();
		});

		it('should extract cause from Error objects with cause', () => {
			const cause = new Error('root cause');
			const err = new Error('wrapper', { cause });
			logger.error('chained error', err);

			const meta = transport.entries[0].metadata;
			expect(meta?.cause).toBe('root cause');
		});

		it('should handle non-Error cause', () => {
			const err = new Error('wrapper', { cause: 'string cause' });
			logger.error('string cause error', err);

			const meta = transport.entries[0].metadata;
			expect(meta?.cause).toBe('string cause');
		});

		it('should handle SimseError objects', () => {
			// Use factory, not constructor
			const { createSimseError } = require('../src/errors/index.js');
			const err = createSimseError('simse error', { code: 'TEST_CODE' });
			logger.error('simse failure', err);

			const meta = transport.entries[0].metadata;
			expect(meta?.errorName).toBe('SimseError');
			expect(meta?.errorMessage).toBe('simse error');
		});

		it('should accept metadata object instead of Error', () => {
			logger.error('with plain metadata', { status: 500 });

			const meta = transport.entries[0].metadata;
			expect(meta).toEqual({ status: 500 });
		});
	});

	// -----------------------------------------------------------------------
	// Log level filtering
	// -----------------------------------------------------------------------

	describe('log level filtering', () => {
		it('should filter out messages below the configured level', () => {
			const infoLogger = createLogger({
				level: 'info',
				transports: [transport],
			});

			infoLogger.debug('should be filtered');
			infoLogger.info('should appear');
			infoLogger.warn('should also appear');
			infoLogger.error('should definitely appear');

			expect(transport.entries).toHaveLength(3);
			expect(transport.entries[0].level).toBe('info');
			expect(transport.entries[1].level).toBe('warn');
			expect(transport.entries[2].level).toBe('error');
		});

		it("should log nothing at 'none' level", () => {
			const silentLogger = createLogger({
				level: 'none',
				transports: [transport],
			});

			silentLogger.debug('nope');
			silentLogger.info('nope');
			silentLogger.warn('nope');
			silentLogger.error('nope');

			expect(transport.entries).toHaveLength(0);
		});

		it("should log everything at 'debug' level", () => {
			const verboseLogger = createLogger({
				level: 'debug',
				transports: [transport],
			});

			verboseLogger.debug('d');
			verboseLogger.info('i');
			verboseLogger.warn('w');
			verboseLogger.error('e');

			expect(transport.entries).toHaveLength(4);
		});

		it("should only log errors at 'error' level", () => {
			const errorLogger = createLogger({
				level: 'error',
				transports: [transport],
			});

			errorLogger.debug('no');
			errorLogger.info('no');
			errorLogger.warn('no');
			errorLogger.error('yes');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('error');
		});

		it("should log warn and error at 'warn' level", () => {
			const warnLogger = createLogger({
				level: 'warn',
				transports: [transport],
			});

			warnLogger.debug('no');
			warnLogger.info('no');
			warnLogger.warn('yes');
			warnLogger.error('yes');

			expect(transport.entries).toHaveLength(2);
		});
	});

	// -----------------------------------------------------------------------
	// Level changes at runtime
	// -----------------------------------------------------------------------

	describe('setLevel / getLevel', () => {
		it('should change the log level at runtime', () => {
			logger.setLevel('error');

			logger.debug('filtered');
			logger.info('filtered');
			logger.warn('filtered');
			logger.error('visible');

			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('error');
		});

		it('should return the current log level', () => {
			expect(logger.getLevel()).toBe('debug');

			logger.setLevel('warn');
			expect(logger.getLevel()).toBe('warn');
		});
	});

	// -----------------------------------------------------------------------
	// Child loggers
	// -----------------------------------------------------------------------

	describe('child', () => {
		it('should create a child with appended context', () => {
			const child = logger.child('sub');

			child.info('child message');

			expect(transport.entries[0].context).toBe('test:sub');
		});

		it('should create nested children with concatenated contexts', () => {
			const child = logger.child('a').child('b').child('c');

			child.info('nested');

			expect(transport.entries[0].context).toBe('test:a:b:c');
		});

		it("should inherit the parent's log level", () => {
			logger.setLevel('warn');
			const child = logger.child('sub');

			child.debug('filtered');
			child.info('filtered');
			child.warn('visible');

			expect(transport.entries).toHaveLength(1);
		});

		it('should share transports with the parent', () => {
			const child = logger.child('sub');

			logger.info('parent');
			child.info('child');

			expect(transport.entries).toHaveLength(2);
			expect(transport.entries[0].context).toBe('test');
			expect(transport.entries[1].context).toBe('test:sub');
		});

		it('should handle child without parent context', () => {
			const noContext = createLogger({
				level: 'debug',
				transports: [transport],
			});

			const child = noContext.child('orphan');
			child.info('test');

			expect(transport.entries[0].context).toBe('orphan');
		});
	});

	// -----------------------------------------------------------------------
	// Multiple transports
	// -----------------------------------------------------------------------

	describe('multiple transports', () => {
		it('should write to all transports', () => {
			const transport2 = createMemoryTransport();
			const multiLogger = createLogger({
				level: 'debug',
				transports: [transport, transport2],
			});

			multiLogger.info('multi');

			expect(transport.entries).toHaveLength(1);
			expect(transport2.entries).toHaveLength(1);
		});

		it('should support adding transports dynamically', () => {
			const transport2 = createMemoryTransport();
			logger.addTransport(transport2);

			logger.info('added transport');

			expect(transport.entries).toHaveLength(1);
			expect(transport2.entries).toHaveLength(1);
		});

		it('should support clearing all transports', () => {
			logger.clearTransports();

			logger.info('void');

			// No transports to write to — just shouldn't error
			expect(transport.entries).toHaveLength(0);
		});
	});

	// -----------------------------------------------------------------------
	// Default logger singleton
	// -----------------------------------------------------------------------

	describe('default logger', () => {
		it('should return a logger instance from getDefaultLogger', () => {
			const defaultLogger = getDefaultLogger();
			expect(typeof defaultLogger.debug).toBe('function');
			expect(typeof defaultLogger.info).toBe('function');
			expect(typeof defaultLogger.warn).toBe('function');
			expect(typeof defaultLogger.error).toBe('function');
			expect(typeof defaultLogger.child).toBe('function');
			expect(typeof defaultLogger.setLevel).toBe('function');
			expect(typeof defaultLogger.getLevel).toBe('function');
		});

		it('should return the same instance on subsequent calls', () => {
			const a = getDefaultLogger();
			const b = getDefaultLogger();
			expect(a).toBe(b);
		});

		it('should allow replacing the default logger', () => {
			const custom = createLogger({
				context: 'custom',
				level: 'error',
				transports: [transport],
			});

			setDefaultLogger(custom);

			const retrieved = getDefaultLogger();
			expect(retrieved).toBe(custom);

			// Clean up: reset to a fresh default
			setDefaultLogger(createLogger({ context: 'simse', level: 'info' }));
		});
	});

	// -----------------------------------------------------------------------
	// Edge cases
	// -----------------------------------------------------------------------

	describe('edge cases', () => {
		it('should handle empty message strings', () => {
			logger.info('');
			expect(transport.entries[0].message).toBe('');
		});

		it('should handle very long messages', () => {
			const longMsg = 'x'.repeat(100_000);
			logger.info(longMsg);
			expect(transport.entries[0].message.length).toBe(100_000);
		});

		it('should handle messages with special characters', () => {
			logger.info('Special: \n\t\r\0 🎉 <script>alert(1)</script>');
			expect(transport.entries[0].message).toContain('🎉');
		});

		it('should handle undefined metadata gracefully', () => {
			logger.info('test', undefined);
			expect(transport.entries[0].metadata).toBeUndefined();
		});

		it('should handle empty metadata object', () => {
			logger.info('test', {});
			expect(transport.entries[0].metadata).toEqual({});
		});

		it('should handle metadata with nested objects', () => {
			logger.info('test', {
				nested: { deep: { value: 42 } },
				array: [1, 2, 3],
			});

			const meta = transport.entries[0].metadata;
			expect(meta).toEqual({
				nested: { deep: { value: 42 } },
				array: [1, 2, 3],
			});
		});

		it('should handle rapid successive logging', () => {
			for (let i = 0; i < 1000; i++) {
				logger.info(`message ${i}`);
			}
			expect(transport.entries).toHaveLength(1000);
		});

		it('should default to info level when not specified', () => {
			const defaultLogger = createLogger({ transports: [transport] });

			defaultLogger.debug('filtered');
			defaultLogger.info('visible');

			// debug should be filtered at default "info" level
			expect(transport.entries).toHaveLength(1);
			expect(transport.entries[0].level).toBe('info');
		});

		it('should default to console transport when no transports specified', () => {
			const spy = spyOn(console, 'log').mockImplementation(() => {});
			const defaultLogger = createLogger({ level: 'info' });

			defaultLogger.info('console test');

			expect(spy).toHaveBeenCalled();
			spy.mockRestore();
		});
	});

	// -----------------------------------------------------------------------
	// Custom transport
	// -----------------------------------------------------------------------

	describe('custom transport', () => {
		it('should work with a custom transport implementation', () => {
			const entries: string[] = [];

			const customTransport: LogTransport = {
				write(entry: LogEntry) {
					entries.push(`[${entry.level}] ${entry.message}`);
				},
			};

			const customLogger = createLogger({
				level: 'debug',
				transports: [customTransport],
			});

			customLogger.info('hello');
			customLogger.error('oops');

			expect(entries).toEqual(['[info] hello', '[error] oops']);
		});
	});
});

// ===========================================================================
// retry()
// ===========================================================================

describe('retry', () => {
	// -----------------------------------------------------------------------
	// Success cases
	// -----------------------------------------------------------------------

	describe('success cases', () => {
		it('should return the result on first attempt success', async () => {
			const fn = mock((..._: any[]): any => {}).mockResolvedValue('success');

			const result = await retry(fn, { maxAttempts: 3 });

			expect(result).toBe('success');
			expect(fn).toHaveBeenCalledTimes(1);
			expect(fn).toHaveBeenCalledWith(1);
		});

		it('should pass the attempt number to the function', async () => {
			const attempts: number[] = [];
			const fn = mock((..._: any[]): any => {}).mockImplementation(
				async (attempt: number) => {
					attempts.push(attempt);
					if (attempt < 3) throw new Error('not yet');
					return 'done';
				},
			);

			const result = await retry(fn, {
				maxAttempts: 3,
				baseDelayMs: 0,
			});

			expect(result).toBe('done');
			expect(attempts).toEqual([1, 2, 3]);
		});

		it('should succeed after retries', async () => {
			let attempt = 0;
			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				attempt++;
				if (attempt < 3) throw new Error('transient');
				return 'recovered';
			});

			const result = await retry(fn, {
				maxAttempts: 5,
				baseDelayMs: 0,
			});

			expect(result).toBe('recovered');
			expect(fn).toHaveBeenCalledTimes(3);
		});

		it('should work with maxAttempts of 1 (no retries)', async () => {
			const fn = mock((..._: any[]): any => {}).mockResolvedValue('one-shot');

			const result = await retry(fn, { maxAttempts: 1 });

			expect(result).toBe('one-shot');
			expect(fn).toHaveBeenCalledTimes(1);
		});
	});

	// -----------------------------------------------------------------------
	// Failure cases
	// -----------------------------------------------------------------------

	describe('failure cases', () => {
		it('should throw RetryExhaustedError when all attempts fail', async () => {
			const { expectGuardedThrow } = require('./utils/error-helpers');
			const { isRetryExhaustedError } = require('../src/utils/retry.js');
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('always fails'),
			);

			await expectGuardedThrow(
				() => retry(fn, { maxAttempts: 3, baseDelayMs: 0 }),
				isRetryExhaustedError,
				'RETRY_EXHAUSTED',
			);

			expect(fn).toHaveBeenCalledTimes(3);
		});

		it('should include attempt count in RetryExhaustedError', async () => {
			const { isRetryExhaustedError } = require('../src/utils/retry.js');
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			try {
				await retry(fn, { maxAttempts: 4, baseDelayMs: 0 });
				throw new Error('should have thrown');
			} catch (e) {
				expect(isRetryExhaustedError(e)).toBe(true);
				expect(e.attempts).toBe(4);
				expect(e.code).toBe('RETRY_EXHAUSTED');
			}
		});

		it('should include the last error as cause', async () => {
			const lastError = new Error('final failure');
			let count = 0;
			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				count++;
				if (count < 3) throw new Error(`failure ${count}`);
				throw lastError;
			});

			try {
				await retry(fn, { maxAttempts: 3, baseDelayMs: 0 });
				throw new Error('should have thrown');
			} catch (e) {
				expect(e.cause).toBe(lastError);
			}
		});

		it('should throw immediately with maxAttempts of 1', async () => {
			const { expectGuardedThrow } = require('./utils/error-helpers');
			const { isRetryExhaustedError } = require('../src/utils/retry.js');
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('one-shot fail'),
			);

			await expectGuardedThrow(
				() => retry(fn, { maxAttempts: 1 }),
				isRetryExhaustedError,
				'RETRY_EXHAUSTED',
			);

			expect(fn).toHaveBeenCalledTimes(1);
		});
	});

	// -----------------------------------------------------------------------
	// shouldRetry predicate
	// -----------------------------------------------------------------------

	describe('shouldRetry predicate', () => {
		it('should abort immediately when shouldRetry returns false', async () => {
			const error = new Error('non-retryable');
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(error);

			await expect(
				retry(fn, {
					maxAttempts: 5,
					baseDelayMs: 0,
					shouldRetry: () => false,
				}),
			).rejects.toThrow('non-retryable');

			expect(fn).toHaveBeenCalledTimes(1);
		});

		it('should throw the original error (not wrapped) when shouldRetry returns false', async () => {
			const error = new Error('original');
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(error);

			try {
				await retry(fn, {
					maxAttempts: 5,
					baseDelayMs: 0,
					shouldRetry: () => false,
				});
				throw new Error('should have thrown');
			} catch (e) {
				expect(e).toBe(error);
				expect(e.code).not.toBe('RETRY_EXHAUSTED');
			}
		});

		it('should receive the error and attempt number', async () => {
			const calls: Array<{ error: unknown; attempt: number }> = [];
			let count = 0;

			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				count++;
				throw new Error(`error-${count}`);
			});

			await expect(
				retry(fn, {
					maxAttempts: 3,
					baseDelayMs: 0,
					shouldRetry: (error, attempt) => {
						calls.push({ error, attempt });
						return true;
					},
				}),
			).rejects.toThrow();

			expect(calls).toHaveLength(2); // Not called on last attempt
			expect((calls[0].error as Error).message).toBe('error-1');
			expect(calls[0].attempt).toBe(1);
			expect((calls[1].error as Error).message).toBe('error-2');
			expect(calls[1].attempt).toBe(2);
		});

		it('should selectively retry based on error type', async () => {
			let attempt = 0;
			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				attempt++;
				if (attempt === 1) throw new Error('transient');
				throw new TypeError('not retryable');
			});

			await expect(
				retry(fn, {
					maxAttempts: 5,
					baseDelayMs: 0,
					shouldRetry: (err) => !(err && err.name === 'TypeError'),
				}),
			).rejects.toThrow(expect.anything());

			expect(fn).toHaveBeenCalledTimes(2);
		});
	});

	// -----------------------------------------------------------------------
	// onRetry callback
	// -----------------------------------------------------------------------

	describe('onRetry callback', () => {
		it('should call onRetry before each retry', async () => {
			const onRetry = mock((..._: any[]): any => {});
			let attempt = 0;

			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				attempt++;
				if (attempt < 3) throw new Error('retry me');
				return 'done';
			});

			await retry(fn, {
				maxAttempts: 5,
				baseDelayMs: 0,
				onRetry,
			});

			expect(onRetry).toHaveBeenCalledTimes(2);

			// First retry: error from attempt 1, next attempt is 2
			expect(typeof onRetry.mock.calls[0][0]?.message).toBe('string');
			expect(onRetry.mock.calls[0][1]).toBe(2); // next attempt number
			expect(typeof onRetry.mock.calls[0][2]).toBe('number'); // delay

			// Second retry: error from attempt 2, next attempt is 3
			expect(onRetry.mock.calls[1][1]).toBe(3);
		});

		it('should not call onRetry on first attempt or success', async () => {
			const onRetry = mock((..._: any[]): any => {});
			const fn = mock((..._: any[]): any => {}).mockResolvedValue('ok');

			await retry(fn, { maxAttempts: 3, onRetry });

			expect(onRetry).not.toHaveBeenCalled();
		});

		it('should not call onRetry after the last failed attempt', async () => {
			const onRetry = mock((..._: any[]): any => {});
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, {
					maxAttempts: 3,
					baseDelayMs: 0,
					onRetry,
				}),
			).rejects.toThrow();

			// 3 attempts, 2 retries (not called after the 3rd attempt)
			expect(onRetry).toHaveBeenCalledTimes(2);
		});
	});

	// -----------------------------------------------------------------------
	// Delay / backoff
	// -----------------------------------------------------------------------

	describe('delay and backoff', () => {
		it('should apply increasing delays with exponential backoff', async () => {
			const delays: number[] = [];
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, {
					maxAttempts: 4,
					baseDelayMs: 100,
					backoffMultiplier: 2,
					jitterFactor: 0, // No jitter for predictable delays
					onRetry: (_err, _attempt, delay) => {
						delays.push(delay);
					},
				}),
			).rejects.toThrow();

			// Delays should be: 100, 200, 400
			expect(delays).toEqual([100, 200, 400]);
		});

		it('should cap delays at maxDelayMs', async () => {
			const delays: number[] = [];
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, {
					maxAttempts: 5,
					baseDelayMs: 100,
					backoffMultiplier: 10,
					maxDelayMs: 500,
					jitterFactor: 0,
					onRetry: (_err, _attempt, delay) => {
						delays.push(delay);
					},
				}),
			).rejects.toThrow();

			// 100, 500 (capped from 1000), 500 (capped from 10000), 500
			expect(delays).toEqual([100, 500, 500, 500]);
		});

		it('should handle zero baseDelayMs', async () => {
			const fn = mock((..._: any[]): any => {})
				.mockRejectedValueOnce(new Error('fail'))
				.mockResolvedValue('ok');

			const result = await retry(fn, {
				maxAttempts: 3,
				baseDelayMs: 0,
			});

			expect(result).toBe('ok');
		});

		it('should apply jitter within expected bounds', async () => {
			const delays: number[] = [];
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, {
					maxAttempts: 4,
					baseDelayMs: 1000,
					backoffMultiplier: 1,
					jitterFactor: 0.5,
					onRetry: (_err, _attempt, delay) => {
						delays.push(delay);
					},
				}),
			).rejects.toThrow();

			// With jitter factor 0.5 and base 1000, delays should be 1000 ± 500
			for (const d of delays) {
				expect(d).toBeGreaterThanOrEqual(500);
				expect(d).toBeLessThanOrEqual(1500);
			}
		});

		it('should clamp jitterFactor to [0, 1]', async () => {
			const delays: number[] = [];
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			// jitterFactor > 1 should be clamped to 1
			await expect(
				retry(fn, {
					maxAttempts: 3,
					baseDelayMs: 100,
					backoffMultiplier: 1,
					jitterFactor: 5, // Should be clamped to 1
					onRetry: (_err, _attempt, delay) => {
						delays.push(delay);
					},
				}),
			).rejects.toThrow();

			// With factor clamped to 1, delays should be 100 ± 100 = [0, 200]
			for (const d of delays) {
				expect(d).toBeGreaterThanOrEqual(0);
				expect(d).toBeLessThanOrEqual(200);
			}
		});
	});

	// -----------------------------------------------------------------------
	// AbortSignal
	// -----------------------------------------------------------------------

	describe('abort signal', () => {
		it('should abort before starting if signal is already aborted', async () => {
			const controller = new AbortController();
			controller.abort();

			const fn = mock((..._: any[]): any => {}).mockResolvedValue('ok');

			await expect(retry(fn, { signal: controller.signal })).rejects.toThrow(
				'aborted',
			);

			expect(fn).not.toHaveBeenCalled();
		});

		it('should abort between retries', async () => {
			const controller = new AbortController();
			let callCount = 0;

			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				callCount++;
				if (callCount === 1) {
					// Abort after first failure
					setTimeout(() => controller.abort(), 10);
					throw new Error('first fail');
				}
				return 'should not reach';
			});

			await expect(
				retry(fn, {
					maxAttempts: 5,
					baseDelayMs: 1000, // Long delay so abort happens during sleep
					signal: controller.signal,
				}),
			).rejects.toThrow('aborted');

			expect(fn).toHaveBeenCalledTimes(1);
		});
	});

	// -----------------------------------------------------------------------
	// Defaults
	// -----------------------------------------------------------------------

	describe('defaults', () => {
		it('should default to 3 max attempts', async () => {
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(retry(fn, { baseDelayMs: 0 })).rejects.toThrow();

			expect(fn).toHaveBeenCalledTimes(3);
		});

		it('should handle minimum maxAttempts clamping', async () => {
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, { maxAttempts: 0, baseDelayMs: 0 }),
			).rejects.toThrow();

			// maxAttempts of 0 should be clamped to 1
			expect(fn).toHaveBeenCalledTimes(1);
		});

		it('should handle negative maxAttempts clamping', async () => {
			const fn = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);

			await expect(
				retry(fn, { maxAttempts: -5, baseDelayMs: 0 }),
			).rejects.toThrow();

			expect(fn).toHaveBeenCalledTimes(1);
		});
	});

	// -----------------------------------------------------------------------
	// Edge cases
	// -----------------------------------------------------------------------

	describe('edge cases', () => {
		it('should handle functions that return void (undefined)', async () => {
			const fn = mock((..._: any[]): any => {}).mockResolvedValue(undefined);

			const result = await retry(fn);

			expect(result).toBeUndefined();
		});

		it('should handle functions that return null', async () => {
			const fn = mock((..._: any[]): any => {}).mockResolvedValue(null);

			const result = await retry(fn);

			expect(result).toBeNull();
		});

		it('should handle functions that return complex objects', async () => {
			const obj = { a: 1, b: [2, 3], c: { d: 4 } };
			const fn = mock((..._: any[]): any => {}).mockResolvedValue(obj);

			const result = await retry(fn);

			expect(result).toBe(obj);
		});

		it('should handle functions that throw non-Error values', async () => {
			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				throw 'string error';
			});

			await expect(retry(fn, { maxAttempts: 1 })).rejects.toThrow();
		});

		it('should handle async generator-like patterns', async () => {
			let value = 0;
			const fn = mock((..._: any[]): any => {}).mockImplementation(async () => {
				value++;
				if (value < 3) throw new Error('not ready');
				return value;
			});

			const result = await retry(fn, {
				maxAttempts: 5,
				baseDelayMs: 0,
			});

			expect(result).toBe(3);
		});
	});
});

// ===========================================================================
// sleep()
// ===========================================================================

describe('sleep', () => {
	it('should resolve after the given delay', async () => {
		const start = Date.now();
		await sleep(50);
		const elapsed = Date.now() - start;

		expect(elapsed).toBeGreaterThanOrEqual(30); // Allow some timing slack
	});

	it('should resolve immediately for 0 ms', async () => {
		const start = Date.now();
		await sleep(0);
		const elapsed = Date.now() - start;

		expect(elapsed).toBeLessThan(50);
	});

	it('should resolve immediately for negative ms', async () => {
		const start = Date.now();
		await sleep(-100);
		const elapsed = Date.now() - start;

		expect(elapsed).toBeLessThan(50);
	});

	it('should reject when aborted via signal', async () => {
		const controller = new AbortController();
		setTimeout(() => controller.abort(), 10);

		await expect(sleep(10000, controller.signal)).rejects.toThrow('aborted');
	});

	it('should reject immediately if signal is already aborted', async () => {
		const controller = new AbortController();
		controller.abort();

		await expect(sleep(10000, controller.signal)).rejects.toThrow('aborted');
	});

	it('should resolve normally without a signal', async () => {
		await expect(sleep(10)).resolves.toBeUndefined();
	});
});

// ===========================================================================
// isTransientError()
// ===========================================================================

describe('isTransientError', () => {
	describe('SimseError instances', () => {
		it('should return true for PROVIDER_TIMEOUT', () => {
			const { createProviderTimeoutError } = require('../src/errors/index.js');
			const err = createProviderTimeoutError('local-server', 30000);
			expect(isTransientError(err)).toBe(true);
		});

		it('should return true for PROVIDER_UNAVAILABLE', () => {
			const {
				createProviderUnavailableError,
			} = require('../src/errors/index.js');
			const err = createProviderUnavailableError('remote-server');
			expect(isTransientError(err)).toBe(true);
		});

		it('should return true for MCP_CONNECTION_ERROR', () => {
			const { createMCPConnectionError } = require('../src/errors/index.js');
			const err = createMCPConnectionError('server', 'connection refused');
			expect(isTransientError(err)).toBe(true);
		});

		it('should return false for non-transient SimseError codes', () => {
			const { createSimseError } = require('../src/errors/index.js');
			const err = createSimseError('not transient', { code: 'CONFIG_ERROR' });
			expect(isTransientError(err)).toBe(false);
		});
	});

	describe('plain Error instances', () => {
		it('should return true for ECONNREFUSED', () => {
			expect(isTransientError(new Error('ECONNREFUSED 127.0.0.1:11434'))).toBe(
				true,
			);
		});

		it('should return true for ECONNRESET', () => {
			expect(isTransientError(new Error('socket: ECONNRESET'))).toBe(true);
		});

		it('should return true for ETIMEDOUT', () => {
			expect(isTransientError(new Error('ETIMEDOUT'))).toBe(true);
		});

		it('should return true for socket hang up', () => {
			expect(isTransientError(new Error('socket hang up'))).toBe(true);
		});

		it('should return true for network errors', () => {
			expect(isTransientError(new Error('Network request failed'))).toBe(true);
		});

		it('should return true for timeout errors', () => {
			expect(isTransientError(new Error('Request timeout'))).toBe(true);
		});

		it('should return true for 503 errors', () => {
			expect(isTransientError(new Error('Service unavailable (503)'))).toBe(
				true,
			);
		});

		it('should return true for 429 errors', () => {
			expect(isTransientError(new Error('Too Many Requests (429)'))).toBe(true);
		});

		it('should return false for generic errors', () => {
			expect(
				isTransientError(new Error('TypeError: x is not a function')),
			).toBe(false);
		});

		it('should return false for validation errors', () => {
			expect(
				isTransientError(new Error('Invalid input: field is required')),
			).toBe(false);
		});
	});

	describe('non-Error values', () => {
		it('should return false for strings', () => {
			expect(isTransientError('connection refused')).toBe(false);
		});

		it('should return false for numbers', () => {
			expect(isTransientError(503)).toBe(false);
		});

		it('should return false for null', () => {
			expect(isTransientError(null)).toBe(false);
		});

		it('should return false for undefined', () => {
			expect(isTransientError(undefined)).toBe(false);
		});

		it('should return false for plain objects', () => {
			expect(isTransientError({ message: 'timeout' })).toBe(false);
		});
	});

	describe('case insensitivity', () => {
		it('should detect transient patterns regardless of case', () => {
			expect(isTransientError(new Error('TIMEOUT occurred'))).toBe(true);
			expect(isTransientError(new Error('Timeout Occurred'))).toBe(true);
			expect(isTransientError(new Error('UNAVAILABLE service'))).toBe(true);
			expect(isTransientError(new Error('NETWORK failure'))).toBe(true);
		});
	});
});

// ===========================================================================
// RetryExhaustedError
// ===========================================================================

describe('RetryExhaustedError', () => {
	it('should be an instance of SimseError', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const { isSimseError } = require('../src/errors/index.js');
		const err = createRetryExhaustedError(3, new Error('last'));
		expect(isSimseError(err)).toBe(true);
		expect(err.code).toBe('RETRY_EXHAUSTED');
	});

	it('should store attempts count', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const err = createRetryExhaustedError(5, new Error('fail'));
		expect(err.attempts).toBe(5);
	});

	it('should store the last error as cause', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const lastError = new Error('the last one');
		const err = createRetryExhaustedError(3, lastError);
		expect(err.cause).toBe(lastError);
	});

	it('should have RETRY_EXHAUSTED code', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const err = createRetryExhaustedError(1, null);
		expect(err.code).toBe('RETRY_EXHAUSTED');
	});

	it('should include attempts in message', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const err = createRetryExhaustedError(7, null);
		expect(err.message).toContain('7');
	});

	it('should include attempts in metadata', () => {
		const { createRetryExhaustedError } = require('../src/utils/retry.js');
		const err = createRetryExhaustedError(4, null);
		expect(err.metadata).toEqual({ attempts: 4 });
	});
});
