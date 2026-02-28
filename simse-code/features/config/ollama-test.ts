/**
 * Ollama connection testing and model discovery.
 *
 * Provides utilities for verifying Ollama server connectivity and
 * listing available models with human-readable size formatting.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type OllamaConnectionResult =
	| { readonly ok: true; readonly version?: string }
	| { readonly ok: false; readonly error: string };

export interface OllamaModelInfo {
	readonly name: string;
	readonly size: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

/**
 * Format a byte count into a human-readable string (e.g., "1.9 GB").
 */
export function formatBytes(bytes: number): string {
	if (bytes >= GB) {
		return `${(bytes / GB).toFixed(1)} GB`;
	}
	if (bytes >= MB) {
		return `${(bytes / MB).toFixed(1)} MB`;
	}
	if (bytes >= KB) {
		return `${(bytes / KB).toFixed(1)} KB`;
	}
	return `${bytes} B`;
}

/**
 * Strip trailing slashes from a URL string.
 */
function stripTrailingSlashes(url: string): string {
	return url.replace(/\/+$/, '');
}

// ---------------------------------------------------------------------------
// Connection test
// ---------------------------------------------------------------------------

const DEFAULT_TIMEOUT_MS = 5000;

/**
 * Test whether an Ollama server is reachable at the given URL.
 *
 * Hits `GET <url>/api/tags` with an AbortController timeout.
 * Returns `{ ok: true, version? }` on success (version from the
 * `x-ollama-version` response header if present), or
 * `{ ok: false, error }` on any failure.
 */
export async function testOllamaConnection(
	url: string,
	timeoutMs: number = DEFAULT_TIMEOUT_MS,
): Promise<OllamaConnectionResult> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);

	try {
		const endpoint = `${stripTrailingSlashes(url)}/api/tags`;
		const response = await fetch(endpoint, { signal: controller.signal });

		if (!response.ok) {
			return Object.freeze({
				ok: false as const,
				error: `HTTP ${response.status}: ${response.statusText}`,
			});
		}

		const version =
			response.headers.get('x-ollama-version') ?? undefined;
		return Object.freeze({ ok: true as const, version });
	} catch (err: unknown) {
		if (
			err instanceof DOMException &&
			err.name === 'AbortError'
		) {
			return Object.freeze({
				ok: false as const,
				error: `Connection timed out after ${timeoutMs}ms`,
			});
		}
		const message =
			err instanceof Error ? err.message : 'Unknown error';
		return Object.freeze({ ok: false as const, error: message });
	} finally {
		clearTimeout(timer);
	}
}

// ---------------------------------------------------------------------------
// Model listing
// ---------------------------------------------------------------------------

interface OllamaTagsResponse {
	readonly models?: ReadonlyArray<{
		readonly name: string;
		readonly size: number;
	}>;
}

/**
 * List models available on an Ollama server.
 *
 * Hits `GET <url>/api/tags`, parses the JSON response, and returns
 * an array of `{ name, size }` where size is human-formatted.
 * Returns an empty array on any failure.
 */
export async function listOllamaModels(
	url: string,
	timeoutMs: number = DEFAULT_TIMEOUT_MS,
): Promise<readonly OllamaModelInfo[]> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);

	try {
		const endpoint = `${stripTrailingSlashes(url)}/api/tags`;
		const response = await fetch(endpoint, { signal: controller.signal });

		if (!response.ok) {
			return [];
		}

		const data = (await response.json()) as OllamaTagsResponse;
		if (!data.models || !Array.isArray(data.models)) {
			return [];
		}

		return data.models.map((model) =>
			Object.freeze({
				name: model.name,
				size: formatBytes(model.size),
			}),
		);
	} catch {
		return [];
	} finally {
		clearTimeout(timer);
	}
}
