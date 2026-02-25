import { describe, expect, it } from 'bun:test';
import { createTopicIndex } from '../src/ai/memory/indexing.js';
import type { VectorEntry } from '../src/ai/memory/types.js';

function makeEntry(
	id: string,
	text: string,
	topics?: string[],
	singleTopic?: string,
): VectorEntry {
	const metadata: Record<string, string> = {};
	if (topics) metadata.topics = JSON.stringify(topics);
	if (singleTopic) metadata.topic = singleTopic;
	return {
		id,
		text,
		embedding: [0.1, 0.2, 0.3],
		metadata,
		timestamp: Date.now(),
	};
}

describe('hierarchical topic index', () => {
	it('auto-creates parent nodes', () => {
		const index = createTopicIndex();
		const entry = makeEntry('1', 'rust async', ['programming/rust/async']);
		index.addEntry(entry);

		const topics = index.getAllTopics();
		const paths = topics.map((t) => t.topic);
		expect(paths).toContain('programming');
		expect(paths).toContain('programming/rust');
		expect(paths).toContain('programming/rust/async');
	});

	it('ancestor query returns descendant entries', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'rust', ['programming/rust']));
		index.addEntry(makeEntry('2', 'python', ['programming/python']));
		index.addEntry(makeEntry('3', 'pasta', ['cooking/italian']));

		const progEntries = index.getEntries('programming');
		expect(progEntries).toContain('1');
		expect(progEntries).toContain('2');
		expect(progEntries).not.toContain('3');
	});

	it('tracks co-occurrence between topics', () => {
		const index = createTopicIndex();
		index.addEntry(
			makeEntry('1', 'web dev', [
				'programming/typescript',
				'programming/react',
			]),
		);
		index.addEntry(
			makeEntry('2', 'more web', [
				'programming/typescript',
				'programming/react',
			]),
		);

		const related = index.getRelatedTopics('programming/typescript');
		expect(related.length).toBeGreaterThan(0);
		const reactRelated = related.find((r) => r.topic === 'programming/react');
		expect(reactRelated).toBeDefined();
		expect(reactRelated!.coOccurrenceCount).toBe(2);
	});

	it('merges topics', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'js stuff', ['js']));
		index.addEntry(makeEntry('2', 'javascript stuff', ['javascript']));

		index.mergeTopic('js', 'javascript');

		expect(index.getEntries('js')).toHaveLength(0);
		expect(index.getEntries('javascript')).toContain('1');
		expect(index.getEntries('javascript')).toContain('2');
	});

	it('supports multi-topic entries via metadata.topics', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'fullstack', ['frontend', 'backend']));

		expect(index.getEntries('frontend')).toContain('1');
		expect(index.getEntries('backend')).toContain('1');
	});

	it('getChildren returns direct children only', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'a', ['lang/rust']));
		index.addEntry(makeEntry('2', 'b', ['lang/python']));
		index.addEntry(makeEntry('3', 'c', ['lang/python/django']));

		const children = index.getChildren('lang');
		expect(children).toContain('lang/rust');
		expect(children).toContain('lang/python');
		expect(children).not.toContain('lang/python/django');
	});

	it('backward compat: metadata.topic string still works', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'test', undefined, 'legacy-topic'));

		expect(index.getEntries('legacy-topic')).toContain('1');
	});

	it('backward compat: auto-extraction from text still works', () => {
		const index = createTopicIndex();
		// Entry with no topic metadata — should auto-extract from text
		index.addEntry({
			id: '1',
			text: 'programming programming programming code code',
			embedding: [0.1, 0.2, 0.3],
			metadata: {},
			timestamp: Date.now(),
		});

		const topics = index.getAllTopics();
		expect(topics.length).toBeGreaterThan(0);
	});

	it('removeEntry decrements co-occurrence', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'web', ['typescript', 'react']));
		index.addEntry(makeEntry('2', 'web2', ['typescript', 'react']));

		index.removeEntry('1');

		const related = index.getRelatedTopics('typescript');
		const reactRelated = related.find((r) => r.topic === 'react');
		expect(reactRelated?.coOccurrenceCount ?? 0).toBe(1);
	});

	it('TopicInfo includes parent and children', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'a', ['lang/rust']));

		const topics = index.getAllTopics();
		const langTopic = topics.find((t) => t.topic === 'lang');
		expect(langTopic).toBeDefined();
		expect(langTopic!.children).toContain('lang/rust');

		const rustTopic = topics.find((t) => t.topic === 'lang/rust');
		expect(rustTopic).toBeDefined();
		expect(rustTopic!.parent).toBe('lang');
	});
});
