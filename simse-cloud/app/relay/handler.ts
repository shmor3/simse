interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}

async function validateToken(
	authApiUrl: string,
	token: string,
): Promise<ValidateResponse['data'] | null> {
	const res = await fetch(`${authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = (await res.json()) as ValidateResponse;
	return json.data;
}

async function resolveAuthApiUrl(env: Env): Promise<string | null> {
	return (
		(await env.SECRETS?.get('AUTH_API_URL')) ||
		(env as unknown as Record<string, string>).AUTH_API_URL ||
		null
	);
}

function jsonError(code: string, message: string, status: number): Response {
	return Response.json({ error: { code, message } }, { status });
}

async function handleWsTunnel(request: Request, env: Env): Promise<Response> {
	const url = new URL(request.url);
	const token = url.searchParams.get('token');
	if (!token) {
		return jsonError('MISSING_TOKEN', 'token query param required', 401);
	}

	const authApiUrl = await resolveAuthApiUrl(env);
	if (!authApiUrl) {
		return jsonError('MISCONFIGURED', 'Service misconfigured', 500);
	}

	const auth = await validateToken(authApiUrl, token);
	if (!auth) {
		return jsonError('UNAUTHORIZED', 'Invalid token', 401);
	}

	const id = env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = env.TUNNEL_SESSION.get(id);

	url.searchParams.set('userId', auth.userId);
	return stub.fetch(new Request(url.toString(), { headers: request.headers }));
}

async function handleWsClient(request: Request, env: Env): Promise<Response> {
	const url = new URL(request.url);
	const token = url.searchParams.get('token');
	if (!token) {
		return jsonError('MISSING_TOKEN', 'token query param required', 401);
	}

	const authApiUrl = await resolveAuthApiUrl(env);
	if (!authApiUrl) {
		return jsonError('MISCONFIGURED', 'Service misconfigured', 500);
	}

	const auth = await validateToken(authApiUrl, token);
	if (!auth) {
		return jsonError('UNAUTHORIZED', 'Invalid token', 401);
	}

	const id = env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = env.TUNNEL_SESSION.get(id);

	url.searchParams.set('userId', auth.userId);
	return stub.fetch(new Request(url.toString(), { headers: request.headers }));
}

async function handleTunnels(request: Request, env: Env): Promise<Response> {
	const authHeader = request.headers.get('Authorization');
	if (!authHeader?.startsWith('Bearer ')) {
		return jsonError('UNAUTHORIZED', 'Bearer token required', 401);
	}

	const authApiUrl = await resolveAuthApiUrl(env);
	if (!authApiUrl) {
		return jsonError('MISCONFIGURED', 'Service misconfigured', 500);
	}

	const token = authHeader.slice(7);
	const res = await fetch(`${authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) {
		return jsonError('UNAUTHORIZED', 'Invalid token', 401);
	}

	const auth = (await res.json()) as ValidateResponse;
	const userId = auth.data.userId;

	const id = env.TUNNEL_SESSION.idFromName(userId);
	const stub = env.TUNNEL_SESSION.get(id);
	const statusRes = await stub.fetch(new Request('https://internal/status'));
	const status = (await statusRes.json()) as {
		hasSession: boolean;
		hasTunnel: boolean;
		hasClient: boolean;
	};

	return Response.json({
		data: {
			tunnels: status.hasTunnel
				? [{ userId, hasTunnel: true, hasClient: status.hasClient }]
				: [],
		},
	});
}

/**
 * Handle relay routes. Returns a Response if matched, null otherwise.
 */
export async function handleRelayRequest(
	request: Request,
	env: Env,
): Promise<Response | null> {
	const url = new URL(request.url);

	if (request.method === 'GET' && url.pathname === '/ws/tunnel') {
		return handleWsTunnel(request, env);
	}
	if (request.method === 'GET' && url.pathname === '/ws/client') {
		return handleWsClient(request, env);
	}
	if (request.method === 'GET' && url.pathname === '/tunnels') {
		return handleTunnels(request, env);
	}

	return null;
}
