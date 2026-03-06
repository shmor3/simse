const API_URL = 'https://api.simse.dev';

export type ApiResponse<T = unknown> = {
	data?: T;
	error?: { code: string; message: string };
};

export async function api(
	path: string,
	options?: RequestInit & { token?: string },
): Promise<Response> {
	const headers = new Headers(options?.headers);
	headers.set('Content-Type', 'application/json');
	if (options?.token) {
		headers.set('Authorization', `Bearer ${options.token}`);
	}

	return fetch(`${API_URL}${path}`, {
		...options,
		headers,
	});
}

export async function authenticatedApi(
	request: Request,
	path: string,
	options?: RequestInit,
): Promise<Response> {
	const token = getTokenFromCookie(request);
	if (!token) {
		throw new Response(null, {
			status: 302,
			headers: { Location: '/auth/login' },
		});
	}
	return api(path, { ...options, token });
}

function getTokenFromCookie(request: Request): string | null {
	const cookie = request.headers.get('Cookie');
	if (!cookie) return null;
	const match = cookie.match(/simse_session=([^;]+)/);
	return match?.[1] ?? null;
}
