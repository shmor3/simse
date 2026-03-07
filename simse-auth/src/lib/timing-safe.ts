/**
 * Constant-time string comparison to prevent timing attacks.
 * Returns true if both strings are equal.
 */
export function timingSafeEqual(a: string, b: string): boolean {
	if (a.length !== b.length) {
		// Compare against self to burn same CPU time, then return false
		let sink = 0;
		for (let i = 0; i < a.length; i++) {
			sink |= a.charCodeAt(i) ^ a.charCodeAt(i);
		}
		void sink;
		return false;
	}
	let diff = 0;
	for (let i = 0; i < a.length; i++) {
		diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
	}
	return diff === 0;
}
