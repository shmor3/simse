// ---------------------------------------------------------------------------
// ACP stream delta extraction
// ---------------------------------------------------------------------------

import type { ACPMessage } from './types.js';

/**
 * Extract a text delta from an SSE event object.
 * Returns undefined if no delta is found.
 */
export function extractStreamDelta(
	event: Record<string, unknown>,
): string | undefined {
	if (event.delta && typeof event.delta === 'string') {
		return event.delta;
	}

	if (typeof event.data === 'object' && event.data !== null) {
		const data = event.data as Record<string, unknown>;
		if (typeof data.delta === 'string') return data.delta;
	}

	if (
		event.status === 'completed' &&
		event.output &&
		Array.isArray(event.output)
	) {
		let text = '';
		for (const msg of event.output as ACPMessage[]) {
			if (msg.role === 'agent') {
				for (const part of msg.parts) {
					if (part.type === 'text') {
						text += (part as { type: 'text'; text: string }).text;
					}
				}
			}
		}
		return text || undefined;
	}

	return undefined;
}
