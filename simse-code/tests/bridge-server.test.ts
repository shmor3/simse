import { describe, expect, it } from 'bun:test';
import {
	createBridgeHandler,
	createNdjsonTransport,
	type JsonRpcMessage,
	type NdjsonTransport,
} from '../bridge-server.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Captures lines written by the transport for assertion. */
function createCapturingTransport(): {
	transport: NdjsonTransport;
	lines: string[];
} {
	const lines: string[] = [];
	const writeLine = (line: string): void => {
		lines.push(line);
	};
	const transport = createNdjsonTransport(writeLine);
	return { transport, lines };
}

/** Parses the last captured line as JSON. */
function lastJson(lines: readonly string[]): Record<string, unknown> {
	return JSON.parse(lines[lines.length - 1]) as Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// createNdjsonTransport
// ---------------------------------------------------------------------------

describe('createNdjsonTransport', () => {
	it('returns a frozen transport object', () => {
		const { transport } = createCapturingTransport();
		expect(Object.isFrozen(transport)).toBe(true);
	});

	it('writeResponse serializes correct JSON-RPC response', () => {
		const { transport, lines } = createCapturingTransport();

		transport.writeResponse(42, { hello: 'world' });

		expect(lines.length).toBe(1);
		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(42);
		expect(parsed.result).toEqual({ hello: 'world' });
		expect(parsed.error).toBeUndefined();
	});

	it('writeError serializes correct JSON-RPC error', () => {
		const { transport, lines } = createCapturingTransport();

		transport.writeError(7, -32601, 'Method not found');

		expect(lines.length).toBe(1);
		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(7);
		expect(parsed.result).toBeUndefined();

		const error = parsed.error as Record<string, unknown>;
		expect(error.code).toBe(-32601);
		expect(error.message).toBe('Method not found');
	});

	it('writeError includes data when provided', () => {
		const { transport, lines } = createCapturingTransport();

		transport.writeError(1, -32603, 'Internal error', { detail: 'oops' });

		const parsed = lastJson(lines);
		const error = parsed.error as Record<string, unknown>;
		expect(error.data).toEqual({ detail: 'oops' });
	});

	it('writeNotification serializes notification without id', () => {
		const { transport, lines } = createCapturingTransport();

		transport.writeNotification('progress', { percent: 50 });

		expect(lines.length).toBe(1);
		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.method).toBe('progress');
		expect(parsed.params).toEqual({ percent: 50 });
		expect(parsed.id).toBeUndefined();
	});

	it('writeNotification omits params when not provided', () => {
		const { transport, lines } = createCapturingTransport();

		transport.writeNotification('heartbeat');

		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.method).toBe('heartbeat');
		expect(parsed.params).toBeUndefined();
	});
});

// ---------------------------------------------------------------------------
// createBridgeHandler
// ---------------------------------------------------------------------------

describe('createBridgeHandler', () => {
	it('handles initialize and returns protocolVersion and name', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		const msg: JsonRpcMessage = {
			jsonrpc: '2.0',
			id: 1,
			method: 'initialize',
			params: {},
		};
		handler(msg);

		expect(lines.length).toBe(1);
		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(1);

		const result = parsed.result as Record<string, unknown>;
		expect(result.protocolVersion).toBe(1);
		expect(result.name).toBe('simse-bridge');
	});

	it('returns METHOD_NOT_FOUND for unknown method', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		const msg: JsonRpcMessage = {
			jsonrpc: '2.0',
			id: 99,
			method: 'nonexistent.method',
			params: {},
		};
		handler(msg);

		expect(lines.length).toBe(1);
		const parsed = lastJson(lines);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(99);

		const error = parsed.error as Record<string, unknown>;
		expect(error.code).toBe(-32601);
		expect(error.message).toContain('nonexistent.method');
	});

	it('ignores messages without an id (notifications)', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		const msg: JsonRpcMessage = {
			jsonrpc: '2.0',
			method: 'some.notification',
			params: {},
		};
		handler(msg);

		expect(lines.length).toBe(0);
	});

	it('handles generate stub', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		handler({
			jsonrpc: '2.0',
			id: 2,
			method: 'generate',
			params: { prompt: 'hello' },
		});

		const parsed = lastJson(lines);
		const result = parsed.result as Record<string, unknown>;
		expect(result.stopReason).toBe('end_turn');
		expect(Array.isArray(result.content)).toBe(true);
	});

	it('handles library.search stub with empty results', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		handler({
			jsonrpc: '2.0',
			id: 3,
			method: 'library.search',
			params: { query: 'test' },
		});

		const parsed = lastJson(lines);
		const result = parsed.result as Record<string, unknown>;
		expect(result.results).toEqual([]);
	});

	it('handles tools.list stub with empty tools', () => {
		const { transport, lines } = createCapturingTransport();
		const handler = createBridgeHandler(transport);

		handler({
			jsonrpc: '2.0',
			id: 4,
			method: 'tools.list',
			params: {},
		});

		const parsed = lastJson(lines);
		const result = parsed.result as Record<string, unknown>;
		expect(result.tools).toEqual([]);
	});
});
