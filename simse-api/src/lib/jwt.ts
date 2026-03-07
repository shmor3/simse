export interface JwtPayload {
	sub: string;
	tid: string | null;
	role: string | null;
	sid: string;
	exp: number;
	iat: number;
}

const ALGORITHM = { name: 'HMAC', hash: 'SHA-256' };

function base64UrlDecode(str: string): Uint8Array {
	const padded = str.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded);
	return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

async function importKey(secret: string): Promise<CryptoKey> {
	const enc = new TextEncoder();
	return crypto.subtle.importKey('raw', enc.encode(secret), ALGORITHM, false, [
		'verify',
	]);
}

export async function verifyJwt(
	token: string,
	secret: string,
): Promise<{ payload: JwtPayload; expired: boolean } | null> {
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
			typeof payload.sid !== 'string'
		) {
			return null;
		}

		const now = Math.floor(Date.now() / 1000);
		return { payload: payload as JwtPayload, expired: payload.exp <= now };
	} catch {
		return null;
	}
}
