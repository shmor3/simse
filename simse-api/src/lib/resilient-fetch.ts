import type { CircuitBreaker } from './circuit-breaker';

const TIMEOUT_MS = 5_000;
const MAX_RETRIES = 2;
const BASE_DELAY_MS = 1_000;
const JITTER_FACTOR = 0.2;
const RETRYABLE_STATUSES = new Set([502, 503, 504]);

function jitteredDelay(baseMs: number): number {
	const jitter = baseMs * JITTER_FACTOR * (2 * Math.random() - 1);
	return Math.max(0, baseMs + jitter);
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function resilientFetch(
	url: string,
	init: RequestInit,
	breaker: CircuitBreaker,
): Promise<Response> {
	if (!breaker.canRequest()) {
		return new Response(
			JSON.stringify({
				error: {
					code: 'SERVICE_UNAVAILABLE',
					message: `${breaker.name} is temporarily unavailable`,
				},
			}),
			{
				status: 503,
				headers: {
					'Content-Type': 'application/json',
					'Retry-After': '30',
				},
			},
		);
	}

	const canRetry = init.method === 'GET' || init.method === 'HEAD';
	const maxAttempts = canRetry ? MAX_RETRIES + 1 : 1;

	let lastError: Error | null = null;
	let lastResponse: Response | null = null;

	for (let attempt = 0; attempt < maxAttempts; attempt++) {
		if (attempt > 0) {
			const delay = jitteredDelay(BASE_DELAY_MS * 2 ** (attempt - 1));
			await sleep(delay);

			if (!breaker.canRequest()) {
				break;
			}
		}

		try {
			const controller = new AbortController();
			const timeoutId = setTimeout(() => controller.abort(), TIMEOUT_MS);

			const response = await fetch(url, {
				...init,
				signal: controller.signal,
			});

			clearTimeout(timeoutId);

			if (
				RETRYABLE_STATUSES.has(response.status) &&
				canRetry &&
				attempt < maxAttempts - 1
			) {
				lastResponse = response;
				breaker.recordFailure();
				continue;
			}

			if (response.ok || response.status < 500) {
				breaker.recordSuccess();
			} else {
				breaker.recordFailure();
			}

			return response;
		} catch (error) {
			lastError = error as Error;
			breaker.recordFailure();

			if (!canRetry || attempt >= maxAttempts - 1) break;
		}
	}

	if (lastResponse) return lastResponse;

	const message =
		lastError?.name === 'AbortError'
			? 'Request timeout'
			: 'Service unavailable';
	return new Response(
		JSON.stringify({
			error: { code: 'SERVICE_UNAVAILABLE', message },
		}),
		{
			status: 503,
			headers: { 'Content-Type': 'application/json', 'Retry-After': '30' },
		},
	);
}
