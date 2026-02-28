/**
 * Helper for standardized error guard assertions in tests.
 *
 * Usage:
 *   await expectGuardedThrow(asyncFn, isChainError, "CHAIN_EMPTY");
 *   expectGuardedThrow(() => fnThatThrows(), isConfigValidationError, "CONFIG_VALIDATION");
 */

type Guard<T = any> = (err: unknown) => err is T;

/**
 * Runs a function (sync or async) and asserts it throws an error matching the guard.
 * Optionally checks error.code.
 *
 * @param fn - Function to run (sync or async)
 * @param guard - Type guard function (e.g., isChainError)
 * @param expectedCode - Optional error code to assert
 */
export async function expectGuardedThrow<T = any>(
	fn: (() => any) | (() => Promise<any>),
	guard: Guard<T>,
	expectedCode?: string,
): Promise<void> {
	let threw = false;
	try {
		await fn();
	} catch (err) {
		threw = true;
		if (!guard(err)) {
			throw new Error(
				`Thrown error did not match guard: ${guard.name || 'unknown guard'}.\nActual: ${JSON.stringify(err)}`,
			);
		}
		if (expectedCode !== undefined && (err as any).code !== expectedCode) {
			throw new Error(
				`Error code mismatch: expected "${expectedCode}", got "${(err as any).code}".`,
			);
		}
		// Passed guard and code check
		return;
	}
	if (!threw) {
		throw new Error('Function did not throw as expected.');
	}
}
