const CURRENT_ITERATIONS = 600_000;
const LEGACY_ITERATIONS = 100_000;

export async function hashPassword(password: string): Promise<string> {
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const key = await deriveKey(password, salt, CURRENT_ITERATIONS);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash as ArrayBuffer);

	const saltB64 = btoa(String.fromCharCode(...salt));
	const hashB64 = btoa(String.fromCharCode(...hashArray));
	return `v2:${saltB64}:${hashB64}`;
}

export async function verifyPassword(
	password: string,
	stored: string,
): Promise<boolean> {
	let saltB64: string;
	let hashB64: string;
	let iterations: number;

	if (stored.startsWith('v2:')) {
		const parts = stored.slice(3).split(':');
		saltB64 = parts[0];
		hashB64 = parts[1];
		iterations = CURRENT_ITERATIONS;
	} else {
		[saltB64, hashB64] = stored.split(':');
		iterations = LEGACY_ITERATIONS;
	}

	const salt = Uint8Array.from(atob(saltB64), (c) => c.charCodeAt(0));
	const storedHash = Uint8Array.from(atob(hashB64), (c) => c.charCodeAt(0));

	const key = await deriveKey(password, salt, iterations);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash as ArrayBuffer);

	if (hashArray.length !== storedHash.length) return false;
	let diff = 0;
	for (let i = 0; i < hashArray.length; i++) {
		diff |= hashArray[i] ^ storedHash[i];
	}
	return diff === 0;
}

export function needsRehash(stored: string): boolean {
	return !stored.startsWith('v2:');
}

async function deriveKey(
	password: string,
	salt: Uint8Array,
	iterations: number,
): Promise<CryptoKey> {
	const enc = new TextEncoder();
	const keyMaterial = await crypto.subtle.importKey(
		'raw',
		enc.encode(password),
		'PBKDF2',
		false,
		['deriveBits', 'deriveKey'],
	);
	return crypto.subtle.deriveKey(
		{ name: 'PBKDF2', salt, iterations, hash: 'SHA-256' },
		keyMaterial,
		{ name: 'AES-GCM', length: 256 },
		true,
		['encrypt'],
	);
}
