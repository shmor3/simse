import { describe, expect, it } from 'bun:test';
import type {
	AgenticLoopOptions,
	LoopCallbacks,
} from '../src/ai/loop/types.js';
import { createEventBus } from '../src/events/event-bus.js';
import type { EventBus, EventType } from '../src/events/types.js';

// ---------------------------------------------------------------------------
// Integration-style tests for EventBus + AgenticLoop wiring.
//
// We cannot easily run a full agentic loop without mocking ACP, so these
// tests focus on:
//   1. AgenticLoopOptions accepts an eventBus field (type compilation check)
//   2. EventBus collects events from subscribeAll
//   3. LoopCallbacks and EventBus can coexist
// ---------------------------------------------------------------------------

describe('loop-events integration', () => {
	it('AgenticLoopOptions accepts an eventBus field', () => {
		// Type-level check: constructing a partial options object with eventBus
		// should compile without errors. We only verify the shape here.
		const bus = createEventBus();
		const partialOptions: Pick<AgenticLoopOptions, 'eventBus'> = {
			eventBus: bus,
		};
		expect(partialOptions.eventBus).toBeDefined();
	});

	it('eventBus collects events via subscribeAll', () => {
		const bus = createEventBus();
		const collected: Array<{ type: EventType; payload: unknown }> = [];

		bus.subscribeAll((type, payload) => {
			collected.push({ type, payload });
		});

		// Simulate the events the agentic loop would publish
		bus.publish('stream.delta', { text: 'Hello' });
		bus.publish('tool.call.start', {
			callId: 'tc-1',
			name: 'search',
			args: { query: 'test' },
		});
		bus.publish('tool.call.end', {
			callId: 'tc-1',
			name: 'search',
			output: 'result',
			isError: false,
			durationMs: 42,
		});
		bus.publish('turn.complete', { turn: 1, type: 'tool_use' });
		bus.publish('turn.complete', { turn: 2, type: 'text' });

		expect(collected).toHaveLength(5);
		expect(collected[0].type).toBe('stream.delta');
		expect(collected[1].type).toBe('tool.call.start');
		expect(collected[2].type).toBe('tool.call.end');
		expect(collected[3].type).toBe('turn.complete');
		expect(collected[4].type).toBe('turn.complete');
	});

	it('eventBus collects compaction events', () => {
		const bus = createEventBus();
		const collected: Array<{ type: EventType; payload: unknown }> = [];

		bus.subscribeAll((type, payload) => {
			collected.push({ type, payload });
		});

		bus.publish('compaction.start', {
			messageCount: 20,
			estimatedChars: 50000,
		});
		bus.publish('compaction.complete', { summaryLength: 500 });

		expect(collected).toHaveLength(2);
		expect(collected[0].type).toBe('compaction.start');
		expect(
			(collected[0].payload as { messageCount: number }).messageCount,
		).toBe(20);
		expect(collected[1].type).toBe('compaction.complete');
		expect(
			(collected[1].payload as { summaryLength: number }).summaryLength,
		).toBe(500);
	});

	it('LoopCallbacks and EventBus can coexist', () => {
		const bus = createEventBus();
		const busEvents: string[] = [];
		const callbackEvents: string[] = [];

		bus.subscribe('stream.delta', (payload) => {
			busEvents.push(payload.text);
		});
		bus.subscribe('tool.call.start', (payload) => {
			busEvents.push(`tool:${payload.name}`);
		});
		bus.subscribe('turn.complete', (payload) => {
			busEvents.push(`turn:${payload.turn}`);
		});

		// Simulate callbacks that would be passed alongside eventBus
		const callbacks: LoopCallbacks = {
			onStreamDelta: (text) => callbackEvents.push(text),
			onToolCallStart: (call) => callbackEvents.push(`tool:${call.name}`),
			onTurnComplete: (turn) => callbackEvents.push(`turn:${turn.turn}`),
		};

		// Simulate what the loop does: invoke both callbacks and eventBus
		const text = 'Hello world';
		callbacks.onStreamDelta?.(text);
		bus.publish('stream.delta', { text });

		const call = { id: 'tc-1', name: 'search', arguments: { q: 'test' } };
		callbacks.onToolCallStart?.(call);
		bus.publish('tool.call.start', {
			callId: call.id,
			name: call.name,
			args: call.arguments,
		});

		callbacks.onTurnComplete?.({
			turn: 1,
			type: 'tool_use',
			durationMs: 100,
		});
		bus.publish('turn.complete', { turn: 1, type: 'tool_use' });

		// Both should have received the same logical events
		expect(callbackEvents).toEqual(['Hello world', 'tool:search', 'turn:1']);
		expect(busEvents).toEqual(['Hello world', 'tool:search', 'turn:1']);
	});

	it('eventBus is optional — undefined does not cause errors', () => {
		// Simulate the pattern used in the loop: optional chaining on undefined
		const eventBus: EventBus | undefined = undefined;

		expect(() => {
			eventBus?.publish('stream.delta', { text: 'test' });
			eventBus?.publish('tool.call.start', {
				callId: 'x',
				name: 'y',
				args: {},
			});
			eventBus?.publish('turn.complete', { turn: 1, type: 'text' });
		}).not.toThrow();
	});

	it('typed subscribers receive correct payload shapes', () => {
		const bus = createEventBus();

		let streamText = '';
		let toolName = '';
		let toolOutput = '';
		let turnNum = 0;

		bus.subscribe('stream.delta', (p) => {
			streamText = p.text;
		});
		bus.subscribe('tool.call.start', (p) => {
			toolName = p.name;
		});
		bus.subscribe('tool.call.end', (p) => {
			toolOutput = p.output;
		});
		bus.subscribe('turn.complete', (p) => {
			turnNum = p.turn;
		});

		bus.publish('stream.delta', { text: 'chunk' });
		bus.publish('tool.call.start', {
			callId: 'c1',
			name: 'read_file',
			args: { path: '/tmp/x' },
		});
		bus.publish('tool.call.end', {
			callId: 'c1',
			name: 'read_file',
			output: 'contents',
			isError: false,
			durationMs: 10,
		});
		bus.publish('turn.complete', { turn: 3, type: 'text' });

		expect(streamText).toBe('chunk');
		expect(toolName).toBe('read_file');
		expect(toolOutput).toBe('contents');
		expect(turnNum).toBe(3);
	});
});
