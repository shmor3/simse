import { describe, expect, it, mock, spyOn } from 'bun:test';
import { createEventBus } from '../src/events/event-bus.js';

describe('createEventBus', () => {
	it('delivers events to subscribers', () => {
		const bus = createEventBus();
		const received: string[] = [];

		bus.subscribe('session.created', (payload) => {
			received.push(payload.sessionId);
		});

		bus.publish('session.created', { sessionId: 'abc' });
		bus.publish('session.created', { sessionId: 'def' });

		expect(received).toEqual(['abc', 'def']);
	});

	it('delivers to multiple subscribers for the same event', () => {
		const bus = createEventBus();
		let countA = 0;
		let countB = 0;

		bus.subscribe('stream.delta', () => {
			countA++;
		});
		bus.subscribe('stream.delta', () => {
			countB++;
		});

		bus.publish('stream.delta', { text: 'hello' });

		expect(countA).toBe(1);
		expect(countB).toBe(1);
	});

	it('unsubscribes correctly', () => {
		const bus = createEventBus();
		const received: number[] = [];

		const unsub = bus.subscribe('turn.complete', (payload) => {
			received.push(payload.turn);
		});

		bus.publish('turn.complete', { turn: 1, type: 'text' });
		unsub();
		bus.publish('turn.complete', { turn: 2, type: 'text' });

		expect(received).toEqual([1]);
	});

	it('unsubscribe is idempotent', () => {
		const bus = createEventBus();
		const handler = mock(() => {});
		const unsub = bus.subscribe('abort', handler);

		unsub();
		unsub(); // calling twice should not throw

		bus.publish('abort', { reason: 'test' });
		expect(handler).not.toHaveBeenCalled();
	});

	it('subscribeAll receives all event types', () => {
		const bus = createEventBus();
		const events: Array<{ type: string; payload: unknown }> = [];

		bus.subscribeAll((type, payload) => {
			events.push({ type, payload });
		});

		bus.publish('session.created', { sessionId: 'x' });
		bus.publish('stream.delta', { text: 'hi' });
		bus.publish('abort', { reason: 'done' });

		expect(events).toHaveLength(3);
		expect(events[0].type).toBe('session.created');
		expect(events[1].type).toBe('stream.delta');
		expect(events[2].type).toBe('abort');
	});

	it('subscribeAll unsubscribes correctly', () => {
		const bus = createEventBus();
		const handler = mock((_type: string, _payload: unknown) => {});
		const unsub = bus.subscribeAll(handler);

		bus.publish('abort', { reason: 'a' });
		unsub();
		bus.publish('abort', { reason: 'b' });

		expect(handler).toHaveBeenCalledTimes(1);
	});

	it('does not throw when publishing with no subscribers', () => {
		const bus = createEventBus();

		expect(() => {
			bus.publish('session.created', { sessionId: 'lonely' });
		}).not.toThrow();
	});

	it('isolates handler errors from other handlers', () => {
		const bus = createEventBus();
		const consoleSpy = spyOn(console, 'error').mockImplementation(() => {});
		const received: string[] = [];

		bus.subscribe('stream.delta', () => {
			throw new Error('boom');
		});
		bus.subscribe('stream.delta', (payload) => {
			received.push(payload.text);
		});

		bus.publish('stream.delta', { text: 'ok' });

		expect(received).toEqual(['ok']);
		expect(consoleSpy).toHaveBeenCalledTimes(1);
		consoleSpy.mockRestore();
	});

	it('isolates global handler errors from other global handlers', () => {
		const bus = createEventBus();
		const consoleSpy = spyOn(console, 'error').mockImplementation(() => {});
		const received: string[] = [];

		bus.subscribeAll(() => {
			throw new Error('global boom');
		});
		bus.subscribeAll((type) => {
			received.push(type);
		});

		bus.publish('abort', { reason: 'test' });

		expect(received).toEqual(['abort']);
		expect(consoleSpy).toHaveBeenCalledTimes(1);
		consoleSpy.mockRestore();
	});

	it('isolates typed handler errors from global handlers', () => {
		const bus = createEventBus();
		const consoleSpy = spyOn(console, 'error').mockImplementation(() => {});
		const globalReceived: string[] = [];

		bus.subscribe('abort', () => {
			throw new Error('typed boom');
		});
		bus.subscribeAll((type) => {
			globalReceived.push(type);
		});

		bus.publish('abort', { reason: 'test' });

		expect(globalReceived).toEqual(['abort']);
		expect(consoleSpy).toHaveBeenCalledTimes(1);
		consoleSpy.mockRestore();
	});

	it('returns a frozen object', () => {
		const bus = createEventBus();

		expect(Object.isFrozen(bus)).toBe(true);
	});

	it('correctly types tool call payloads', () => {
		const bus = createEventBus();
		const calls: Array<{ id: string; name: string }> = [];

		bus.subscribe('tool.call.start', (payload) => {
			calls.push({ id: payload.callId, name: payload.name });
		});

		bus.publish('tool.call.start', {
			callId: 't1',
			name: 'search',
			args: { query: 'test' },
		});

		expect(calls).toEqual([{ id: 't1', name: 'search' }]);
	});
});
