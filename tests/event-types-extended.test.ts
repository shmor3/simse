import { describe, expect, it, mock } from 'bun:test';
import { createEventBus } from '../src/events/event-bus.js';
import type { EventPayloadMap } from '../src/events/types.js';

// ---------------------------------------------------------------------------
// Event type tests
// ---------------------------------------------------------------------------

describe('extended EventPayloadMap types', () => {
	it('memory.add event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['memory.add']) => {});
		bus.subscribe('memory.add', handler);

		bus.publish('memory.add', { id: 'mem_1', contentLength: 42 });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			id: 'mem_1',
			contentLength: 42,
		});
	});

	it('memory.search event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['memory.search']) => {});
		bus.subscribe('memory.search', handler);

		bus.publish('memory.search', {
			query: 'test query',
			resultCount: 5,
			durationMs: 120,
		});

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			query: 'test query',
			resultCount: 5,
			durationMs: 120,
		});
	});

	it('memory.delete event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['memory.delete']) => {});
		bus.subscribe('memory.delete', handler);

		bus.publish('memory.delete', { id: 'mem_42' });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({ id: 'mem_42' });
	});

	it('subagent.start event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['subagent.start']) => {});
		bus.subscribe('subagent.start', handler);

		bus.publish('subagent.start', {
			subagentId: 'explore_1',
			type: 'explore',
			task: 'Find API endpoints',
		});

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			subagentId: 'explore_1',
			type: 'explore',
			task: 'Find API endpoints',
		});
	});

	it('subagent.complete event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock(
			(_payload: EventPayloadMap['subagent.complete']) => {},
		);
		bus.subscribe('subagent.complete', handler);

		bus.publish('subagent.complete', {
			subagentId: 'plan_1',
			type: 'plan',
			durationMs: 5000,
		});

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			subagentId: 'plan_1',
			type: 'plan',
			durationMs: 5000,
		});
	});

	it('subagent.error event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['subagent.error']) => {});
		bus.subscribe('subagent.error', handler);

		const error = new Error('Connection timeout');
		bus.publish('subagent.error', {
			subagentId: 'explore_2',
			type: 'explore',
			error,
		});

		expect(handler).toHaveBeenCalledTimes(1);
		const call = handler.mock.calls[0][0];
		expect(call.subagentId).toBe('explore_2');
		expect(call.type).toBe('explore');
		expect(call.error.message).toBe('Connection timeout');
	});

	it('loop.start event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['loop.start']) => {});
		bus.subscribe('loop.start', handler);

		bus.publish('loop.start', { userInput: 'Hello' });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({ userInput: 'Hello' });
	});

	it('loop.complete event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['loop.complete']) => {});
		bus.subscribe('loop.complete', handler);

		bus.publish('loop.complete', {
			totalTurns: 3,
			hitTurnLimit: false,
			aborted: false,
			totalDurationMs: 5000,
		});

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			totalTurns: 3,
			hitTurnLimit: false,
			aborted: false,
			totalDurationMs: 5000,
		});
	});

	it('stream.start event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['stream.start']) => {});
		bus.subscribe('stream.start', handler);

		bus.publish('stream.start', { turn: 1 });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({ turn: 1 });
	});

	it('stream.retry event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['stream.retry']) => {});
		bus.subscribe('stream.retry', handler);

		bus.publish('stream.retry', { turn: 1, attempt: 2, error: 'ECONNRESET' });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			turn: 1,
			attempt: 2,
			error: 'ECONNRESET',
		});
	});

	it('tool.timeout event has correct shape', () => {
		const bus = createEventBus();
		const handler = mock((_payload: EventPayloadMap['tool.timeout']) => {});
		bus.subscribe('tool.timeout', handler);

		bus.publish('tool.timeout', { name: 'slow_tool', timeoutMs: 5000 });

		expect(handler).toHaveBeenCalledTimes(1);
		expect(handler).toHaveBeenCalledWith({
			name: 'slow_tool',
			timeoutMs: 5000,
		});
	});

	it('subscribeAll receives memory and subagent events', () => {
		const bus = createEventBus();
		const events: Array<{ type: string; payload: unknown }> = [];

		bus.subscribeAll((type, payload) => {
			events.push({ type, payload });
		});

		bus.publish('memory.add', { id: 'x', contentLength: 10 });
		bus.publish('subagent.start', {
			subagentId: 's1',
			type: 'explore',
			task: 'test',
		});

		expect(events).toHaveLength(2);
		expect(events[0].type).toBe('memory.add');
		expect(events[1].type).toBe('subagent.start');
	});
});
