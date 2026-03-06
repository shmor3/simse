interface SessionState {
	userId: string;
	tunnelWs: WebSocket | null;
	clientWs: WebSocket | null;
	connectedAt: number;
}

export class TunnelSession implements DurableObject {
	private state: DurableObjectState;
	private env: Env;
	private session: SessionState | null = null;

	constructor(state: DurableObjectState, env: Env) {
		this.state = state;
		this.env = env;
	}

	async fetch(request: Request): Promise<Response> {
		const url = new URL(request.url);
		const path = url.pathname;

		if (path === '/ws/tunnel') {
			return this.handleTunnelWebSocket(url);
		}

		if (path === '/ws/client') {
			return this.handleClientWebSocket();
		}

		if (path === '/status') {
			return Response.json({
				hasSession: this.session !== null,
				hasTunnel: this.session?.tunnelWs !== null,
				hasClient: this.session?.clientWs !== null,
			});
		}

		return new Response('not found', { status: 404 });
	}

	private handleTunnelWebSocket(url: URL): Response {
		const userId = url.searchParams.get('userId');
		if (!userId) {
			return new Response('missing userId', { status: 400 });
		}

		const pair = new WebSocketPair();
		const [client, server] = Object.values(pair);

		this.state.acceptWebSocket(server, ['tunnel']);

		this.session = {
			userId,
			tunnelWs: server,
			clientWs: this.session?.clientWs ?? null,
			connectedAt: Date.now(),
		};

		return new Response(null, { status: 101, webSocket: client });
	}

	private handleClientWebSocket(): Response {
		if (!this.session?.tunnelWs) {
			return new Response('no tunnel connected', { status: 503 });
		}

		const pair = new WebSocketPair();
		const [client, server] = Object.values(pair);

		this.state.acceptWebSocket(server, ['client']);
		this.session.clientWs = server;

		return new Response(null, { status: 101, webSocket: client });
	}

	async webSocketMessage(
		ws: WebSocket,
		message: string | ArrayBuffer,
	): Promise<void> {
		if (!this.session) return;

		const tags = this.state.getTags(ws);
		const msgStr =
			typeof message === 'string'
				? message
				: new TextDecoder().decode(message);

		if (tags.includes('tunnel')) {
			if (this.session.clientWs) {
				try {
					this.session.clientWs.send(msgStr);
				} catch {
					this.session.clientWs = null;
				}
			}
		} else if (tags.includes('client')) {
			if (this.session.tunnelWs) {
				try {
					this.session.tunnelWs.send(msgStr);
				} catch {
					this.session.tunnelWs = null;
				}
			}
		}
	}

	async webSocketClose(
		ws: WebSocket,
		_code: number,
		_reason: string,
		_wasClean: boolean,
	): Promise<void> {
		if (!this.session) return;

		const tags = this.state.getTags(ws);

		if (tags.includes('tunnel')) {
			this.session.tunnelWs = null;
			if (this.session.clientWs) {
				try {
					this.session.clientWs.close(1001, 'tunnel disconnected');
				} catch {
					// ignore
				}
				this.session.clientWs = null;
			}
			this.session = null;
		} else if (tags.includes('client')) {
			this.session.clientWs = null;
		}
	}

	async webSocketError(ws: WebSocket, _error: unknown): Promise<void> {
		await this.webSocketClose(ws, 1011, 'error', false);
	}
}
