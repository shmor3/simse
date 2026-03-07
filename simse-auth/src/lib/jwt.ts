export interface JwtPayload {
	sub: string; // userId
	tid: string | null; // teamId
	role: string | null; // team role
	sid: string; // sessionId (family_id)
	exp: number; // expiry (unix seconds)
	iat: number; // issued at (unix seconds)
}

const ACCESS_TOKEN_TTL_SECONDS = 900; // 15 minutes
const ALGORITHM = { name: 'HMAC', hash: 'SHA-256' };

function base64UrlEncode(data: Uint8Array): string {
	return btoa(String.fromCharCode(...data))
		.replace(/\+/g, '-')
		.replace(/\//g, '_')
		.replace(/=+$/, '');
}

function base64UrlDecode(str: string): Uint8Array {
	const padded = str.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded);
	return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

async function importKey(secret: string): Promise<CryptoKey> {
	const enc = new TextEncoder();
	return crypto.subtle.importKey('raw', enc.encode(secret), ALGORITHM, false, [
		'sign',
		'verify',
	]);
}

export async function signJwt(
	payload: Omit<JwtPayload, 'exp' | 'iat'>,
	secret: string,
): Promise<{ token: string; expiresIn: number }> {
	const now = Math.floor(Date.now() / 1000);
	const fullPayload: JwtPayload = {
		...payload,
		iat: now,
		exp: now + ACCESS_TOKEN_TTL_SECONDS,
	};

	const enc = new TextEncoder();
	const header = base64UrlEncode(
		enc.encode(JSON.stringify({ alg: 'HS256', typ: 'JWT' })),
	);
	const body = base64UrlEncode(enc.encode(JSON.stringify(fullPayload)));
	const signingInput = `${header}.${body}`;

	const key = await importKey(secret);
	const signature = await crypto.subtle.sign(
		'HMAC',
		key,
		enc.encode(signingInput),
	);

	const sig = base64UrlEncode(new Uint8Array(signature));
	return {
		token: `${signingInput}.${sig}`,
		expiresIn: ACCESS_TOKEN_TTL_SECONDS,
	};
}

export async function verifyJwt(
	token: string,
	secret: string,
): Promise<JwtPayload | null> {
	try {
		const parts = token.split('.');
		if (parts.length !== 3) return null;

		const [header, body, sig] = parts;

		// Validate alg header — reject anything other than HS256
		const headerObj = JSON.parse(
			new TextDecoder().decode(base64UrlDecode(header)),
		) as { alg?: string };
		if (headerObj.alg !== 'HS256') return null;

		const enc = new TextEncoder();
		const signingInput = `${header}.${body}`;

		const key = await importKey(secret);
		const signatureBytes = base64UrlDecode(sig);

		const valid = await crypto.subtle.verify(
			'HMAC',
			key,
			signatureBytes,
			enc.encode(signingInput),
		);

		if (!valid) return null;

		const payload = JSON.parse(
			new TextDecoder().decode(base64UrlDecode(body)),
		) as Partial<JwtPayload>;

		// Validate required fields exist
		if (
			typeof payload.sub !== 'string' ||
			typeof payload.exp !== 'number' ||
			typeof payload.iat !== 'number' ||
			typeof payload.sid !== 'string'
		) {
			return null;
		}

		// Reject expired tokens
		if (payload.exp <= Math.floor(Date.now() / 1000)) return null;

		return payload as JwtPayload;
	} catch {
		// Malformed base64, invalid JSON, or other parsing errors
		return null;
	}
}
