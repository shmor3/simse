const COOKIE_NAME = 'simse_session';

export function setSessionCookie(sessionToken: string): string {
	return `${COOKIE_NAME}=${sessionToken}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=${30 * 24 * 60 * 60}`;
}

export function clearSessionCookie(): string {
	return `${COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0`;
}
