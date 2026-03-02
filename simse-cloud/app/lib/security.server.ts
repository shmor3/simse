import { generateId } from './db.server';

/**
 * Check if a login is from a new device by comparing IP/User-Agent fingerprint.
 * Returns true if this appears to be a new device.
 */
export function getDeviceFingerprint(request: Request): string {
	const ip = request.headers.get('CF-Connecting-IP') ?? 'unknown';
	const ua = request.headers.get('User-Agent') ?? 'unknown';
	// Simple fingerprint — combine IP + first 50 chars of UA
	return `${ip}:${ua.slice(0, 50)}`;
}

/**
 * Record a login and check for suspicious activity.
 * Creates a notification if the device is new.
 */
export async function recordLogin(
	_db: D1Database,
	_userId: string,
	request: Request,
): Promise<{ isNewDevice: boolean }> {
	// Compute fingerprint for future comparison
	getDeviceFingerprint(request);

	// For now, we just return false. A real implementation would
	// store device fingerprints and compare against previous logins.

	return { isNewDevice: false };
}

/**
 * Create a security notification for a user.
 */
export async function createSecurityNotification(
	db: D1Database,
	userId: string,
	title: string,
	body: string,
): Promise<void> {
	await db
		.prepare(
			"INSERT INTO notifications (id, user_id, type, title, body) VALUES (?, ?, 'warning', ?, ?)",
		)
		.bind(generateId(), userId, title, body)
		.run();
}

/**
 * Create an email change token and send confirmation email.
 */
export async function initiateEmailChange(
	db: D1Database,
	userId: string,
	newEmail: string,
): Promise<string> {
	const tokenId = generateId();
	const code = String(
		crypto.getRandomValues(new Uint32Array(1))[0] % 1_000_000,
	).padStart(6, '0');

	await db
		.prepare(
			"INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, 'email_change', ?, datetime('now', '+15 minutes'))",
		)
		.bind(tokenId, userId, `${newEmail}:${code}`)
		.run();

	return code;
}
