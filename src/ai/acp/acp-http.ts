// ---------------------------------------------------------------------------
// ACP HTTP helpers — shared fetch utilities for the ACP client
// ---------------------------------------------------------------------------

import {
	createProviderError,
	createProviderGenerationError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	toError,
} from '../../errors/index.js';
import type { ACPServerEntry } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ResolvedServer {
	readonly entry: ACPServerEntry;
	readonly baseUrl: string;
}

// ---------------------------------------------------------------------------
// Header building
// ---------------------------------------------------------------------------

export function buildHeaders(server: ResolvedServer): Record<string, string> {
	const headers: Record<string, string> = {
		'Content-Type': 'application/json',
		Accept: 'application/json',
	};

	if (server.entry.apiKey) {
		headers.Authorization = `Bearer ${server.entry.apiKey}`;
	}

	return headers;
}

// ---------------------------------------------------------------------------
// Fetch with timeout
// ---------------------------------------------------------------------------

export async function fetchWithTimeout(
	url: string,
	init: RequestInit,
	timeoutMs: number,
): Promise<Response> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);

	try {
		return await fetch(url, { ...init, signal: controller.signal });
	} finally {
		clearTimeout(timer);
	}
}

// ---------------------------------------------------------------------------
// Error wrapping
// ---------------------------------------------------------------------------

export function wrapFetchError(
	operation: string,
	error: unknown,
	serverName: string,
	timeoutMs: number,
): ReturnType<typeof createProviderError> {
	const err = toError(error);
	const message = err.message.toLowerCase();

	if (err.name === 'AbortError' || message.includes('abort')) {
		return createProviderTimeoutError('acp', timeoutMs, { cause: error });
	}

	if (
		message.includes('econnrefused') ||
		message.includes('econnreset') ||
		message.includes('fetch failed') ||
		message.includes('network') ||
		message.includes('dns')
	) {
		return createProviderUnavailableError('acp', {
			cause: error,
			metadata: { serverName, operation },
		});
	}

	if (message.includes('timeout') || message.includes('etimedout')) {
		return createProviderTimeoutError('acp', timeoutMs, { cause: error });
	}

	return createProviderGenerationError(
		'acp',
		`ACP ${operation} failed: ${err.message}`,
		{ cause: error },
	);
}

// ---------------------------------------------------------------------------
// HTTP GET / POST
// ---------------------------------------------------------------------------

export async function httpGet<T>(
	server: ResolvedServer,
	path: string,
): Promise<T> {
	const url = `${server.baseUrl}${path}`;
	const timeoutMs = server.entry.timeoutMs ?? 30_000;

	let response: Response;
	try {
		response = await fetchWithTimeout(
			url,
			{
				method: 'GET',
				headers: buildHeaders(server),
			},
			timeoutMs,
		);
	} catch (error) {
		throw wrapFetchError('GET', error, server.entry.name, timeoutMs);
	}

	if (!response.ok) {
		const text = await response.text().catch(() => '');
		throw createProviderError(
			'acp',
			`GET ${path} failed (${response.status}): ${text}`,
			{
				code: 'PROVIDER_HTTP_ERROR',
				statusCode: response.status,
				metadata: { serverName: server.entry.name, path },
			},
		);
	}

	return (await response.json()) as T;
}

export async function httpPost<T>(
	server: ResolvedServer,
	path: string,
	body: unknown,
): Promise<T> {
	const url = `${server.baseUrl}${path}`;
	const timeoutMs = server.entry.timeoutMs ?? 30_000;

	let response: Response;
	try {
		response = await fetchWithTimeout(
			url,
			{
				method: 'POST',
				headers: buildHeaders(server),
				body: JSON.stringify(body),
			},
			timeoutMs,
		);
	} catch (error) {
		throw wrapFetchError('POST', error, server.entry.name, timeoutMs);
	}

	if (!response.ok) {
		const text = await response.text().catch(() => '');
		throw createProviderError(
			'acp',
			`POST ${path} failed (${response.status}): ${text}`,
			{
				code: 'PROVIDER_HTTP_ERROR',
				statusCode: response.status,
				metadata: { serverName: server.entry.name, path },
			},
		);
	}

	return (await response.json()) as T;
}
