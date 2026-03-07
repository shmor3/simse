/**
 * Handle relay routes. Requests arrive pre-authenticated from simse-api
 * with userId in query params (WS) or X-User-Id header (REST).
 * Returns a Response if matched, null otherwise.
 */
export async function handleRelayRequest(
	request: Request,
	env: Env,
): Promise<Response | null> {
	const url = new URL(request.url);

	if (request.method === 'GET' && url.pathname === '/ws/tunnel') {
		return handleWs(request, env);
	}
	if (request.method === 'GET' && url.pathname === '/ws/client') {
		return handleWs(request, env);
	}
	if (request.method === 'GET' && url.pathname === '/tunnels') {
		return handleTunnels(request, env);
	}

	return null;
}

async function handleWs(request: Request, env: Env): Promise<Response> {
	const url = new URL(request.url);
	const userId = url.searchParams.get('userId');
	if (!userId) {
		return Response.json(
			{ error: { code: 'BAD_REQUEST', message: 'missing userId' } },
			{ status: 400 },
		);
	}

	const id = env.TUNNEL_SESSION.idFromName(userId);
	const stub = env.TUNNEL_SESSION.get(id);
	return stub.fetch(request);
}

async function handleTunnels(request: Request, env: Env): Promise<Response> {
	const userId = request.headers.get('X-User-Id');
	if (!userId) {
		return Response.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Missing user identity' } },
			{ status: 401 },
		);
	}

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
