export async function hashPassword(password: string): Promise<string> {
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const key = await deriveKey(password, salt);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash);

	const saltB64 = btoa(String.fromCharCode(...salt));
	const hashB64 = btoa(String.fromCharCode(...hashArray));
	return `${saltB64}:${hashB64}`;
}

export async function verifyPassword(
	password: string,
	stored: string,
): Promise<boolean> {
	const [saltB64, hashB64] = stored.split(':');
	const salt = Uint8Array.from(atob(saltB64), (c) => c.charCodeAt(0));
	const storedHash = Uint8Array.from(atob(hashB64), (c) => c.charCodeAt(0));

	const key = await deriveKey(password, salt);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash);

	if (hashArray.length !== storedHash.length) return false;
	let diff = 0;
	for (let i = 0; i < hashArray.length; i++) {
		diff |= hashArray[i] ^ storedHash[i];
	}
	return diff === 0;
}

async function deriveKey(
	password: string,
	salt: Uint8Array,
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
		{ name: 'PBKDF2', salt, iterations: 100_000, hash: 'SHA-256' },
		keyMaterial,
		{ name: 'AES-GCM', length: 256 },
		true,
		['encrypt'],
	);
}
