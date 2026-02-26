import { describe, expect, it } from 'bun:test';
import type {
	ACPStreamChunk,
	ACPStreamToolCall,
	ACPStreamToolCallUpdate,
} from '../src/ai/acp/types.js';

// ---------------------------------------------------------------------------
// ACPStreamChunk type tests
// ---------------------------------------------------------------------------

describe('ACPStreamChunk tool call types', () => {
	it('ACPStreamToolCall has correct shape', () => {
		const chunk: ACPStreamToolCall = {
			type: 'tool_call',
			toolCall: {
				toolCallId: 'tc_1',
				title: 'Read file',
				kind: 'read',
				status: 'pending',
			},
		};

		expect(chunk.type).toBe('tool_call');
		expect(chunk.toolCall.toolCallId).toBe('tc_1');
		expect(chunk.toolCall.title).toBe('Read file');
		expect(chunk.toolCall.kind).toBe('read');
		expect(chunk.toolCall.status).toBe('pending');
	});

	it('ACPStreamToolCallUpdate has correct shape', () => {
		const chunk: ACPStreamToolCallUpdate = {
			type: 'tool_call_update',
			update: {
				toolCallId: 'tc_1',
				status: 'completed',
				content: 'file contents here',
			},
		};

		expect(chunk.type).toBe('tool_call_update');
		expect(chunk.update.toolCallId).toBe('tc_1');
		expect(chunk.update.status).toBe('completed');
		expect(chunk.update.content).toBe('file contents here');
	});

	it('ACPStreamChunk union accepts all four variants', () => {
		const chunks: ACPStreamChunk[] = [
			{ type: 'delta', text: 'hello' },
			{
				type: 'tool_call',
				toolCall: {
					toolCallId: 'tc_1',
					title: 'Search',
					kind: 'search',
					status: 'in_progress',
				},
			},
			{
				type: 'tool_call_update',
				update: {
					toolCallId: 'tc_1',
					status: 'completed',
				},
			},
			{ type: 'complete', usage: undefined },
		];

		expect(chunks).toHaveLength(4);
		expect(chunks[0].type).toBe('delta');
		expect(chunks[1].type).toBe('tool_call');
		expect(chunks[2].type).toBe('tool_call_update');
		expect(chunks[3].type).toBe('complete');
	});

	it('tool_call chunk carries all ACPToolCall fields', () => {
		const chunk: ACPStreamChunk = {
			type: 'tool_call',
			toolCall: {
				toolCallId: 'tc_42',
				title: 'Execute bash',
				kind: 'execute',
				status: 'in_progress',
			},
		};

		if (chunk.type === 'tool_call') {
			expect(chunk.toolCall.toolCallId).toBe('tc_42');
			expect(chunk.toolCall.title).toBe('Execute bash');
			expect(chunk.toolCall.kind).toBe('execute');
			expect(chunk.toolCall.status).toBe('in_progress');
		} else {
			throw new Error('Expected tool_call chunk');
		}
	});

	it('tool_call_update chunk carries optional content', () => {
		const chunkWithContent: ACPStreamChunk = {
			type: 'tool_call_update',
			update: {
				toolCallId: 'tc_42',
				status: 'completed',
				content: { output: 'result data' },
			},
		};

		const chunkWithoutContent: ACPStreamChunk = {
			type: 'tool_call_update',
			update: {
				toolCallId: 'tc_42',
				status: 'failed',
			},
		};

		if (chunkWithContent.type === 'tool_call_update') {
			expect(chunkWithContent.update.content).toEqual({
				output: 'result data',
			});
		}

		if (chunkWithoutContent.type === 'tool_call_update') {
			expect(chunkWithoutContent.update.content).toBeUndefined();
		}
	});
});
