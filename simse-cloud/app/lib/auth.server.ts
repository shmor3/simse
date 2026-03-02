import { generateId } from './db.server';

/**
 * Hash a password using Web Crypto API (PBKDF2).
 * Format: base64(salt):base64(hash)
 */
export async function hashPassword(password: string): Promise<string> {
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const key = await deriveKey(password, salt);
	const hash = await crypto.subtle.exportKey('raw', key);
	return `${toBase64(salt)}:${toBase64(new Uint8Array(hash))}`;
}

export async function verifyPassword(
	password: string,
	stored: string,
): Promise<boolean> {
	const [saltB64, hashB64] = stored.split(':');
	const salt = fromBase64(saltB64);
	const key = await deriveKey(password, salt);
	const hash = new Uint8Array(await crypto.subtle.exportKey('raw', key));
	const expected = fromBase64(hashB64);

	if (hash.length !== expected.length) return false;
	let diff = 0;
	for (let i = 0; i < hash.length; i++) {
		diff |= hash[i] ^ expected[i];
	}
	return diff === 0;
}

async function deriveKey(
	password: string,
	salt: Uint8Array,
): Promise<CryptoKey> {
	const encoder = new TextEncoder();
	const baseKey = await crypto.subtle.importKey(
		'raw',
		encoder.encode(password),
		'PBKDF2',
		false,
		['deriveBits', 'deriveKey'],
	);
	return crypto.subtle.deriveKey(
		{
			name: 'PBKDF2',
			salt: salt as BufferSource,
			iterations: 100_000,
			hash: 'SHA-256',
		},
		baseKey,
		{ name: 'AES-GCM', length: 256 },
		true,
		['encrypt'],
	);
}

function toBase64(bytes: Uint8Array): string {
	let binary = '';
	for (const b of bytes) binary += String.fromCharCode(b);
	return btoa(binary);
}

function fromBase64(str: string): Uint8Array {
	const binary = atob(str);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}

export function generateCode(): string {
	const n = crypto.getRandomValues(new Uint32Array(1))[0] % 1_000_000;
	return String(n).padStart(6, '0');
}

export async function createSession(
	db: D1Database,
	userId: string,
): Promise<string> {
	const id = generateId();
	const expiresAt = new Date(
		Date.now() + 30 * 24 * 60 * 60 * 1000,
	).toISOString();
	await db
		.prepare('INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)')
		.bind(id, userId, expiresAt)
		.run();
	return id;
}

export async function deleteSession(
	db: D1Database,
	sessionId: string,
): Promise<void> {
	await db.prepare('DELETE FROM sessions WHERE id = ?').bind(sessionId).run();
}
