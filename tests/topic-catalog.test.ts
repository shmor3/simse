// tests/topic-catalog.test.ts
import { describe, expect, it } from 'bun:test';
import {
	createTopicCatalog,
	type TopicCatalog,
} from '../src/ai/library/topic-catalog.js';

describe('TopicCatalog', () => {
	it('resolve() registers a new topic', () => {
		const catalog = createTopicCatalog();
		const resolved = catalog.resolve('architecture/database');
		expect(resolved).toBe('architecture/database');
		expect(catalog.sections()).toContainEqual(
			expect.objectContaining({ topic: 'architecture/database' }),
		);
	});

	it('resolve() normalizes similar topics via Levenshtein', () => {
		const catalog = createTopicCatalog();
		catalog.resolve('architecture/database');
		// 'architecure/database' is a typo — should match existing
		const resolved = catalog.resolve('architecure/database');
		expect(resolved).toBe('architecture/database');
	});

	it('resolve() maps aliases to canonical names', () => {
		const catalog = createTopicCatalog();
		catalog.resolve('architecture/database');
		catalog.addAlias('db', 'architecture/database');
		const resolved = catalog.resolve('db');
		expect(resolved).toBe('architecture/database');
	});

	it('relocate() moves a volume to a new topic', () => {
		const catalog = createTopicCatalog();
		catalog.resolve('bugs/open');
		catalog.registerVolume('v1', 'bugs/open');
		catalog.relocate('v1', 'bugs/resolved');
		const volumes = catalog.volumes('bugs/resolved');
		expect(volumes).toContain('v1');
		expect(catalog.volumes('bugs/open')).not.toContain('v1');
	});

	it('merge() combines two sections', () => {
		const catalog = createTopicCatalog();
		catalog.resolve('arch/db');
		catalog.resolve('architecture/database');
		catalog.registerVolume('v1', 'arch/db');
		catalog.registerVolume('v2', 'architecture/database');
		catalog.merge('arch/db', 'architecture/database');
		const volumes = catalog.volumes('architecture/database');
		expect(volumes).toContain('v1');
		expect(volumes).toContain('v2');
	});

	it('sections() returns the full tree', () => {
		const catalog = createTopicCatalog();
		catalog.resolve('architecture/database/schema');
		catalog.resolve('architecture/api');
		const sections = catalog.sections();
		const topics = sections.map((s) => s.topic);
		expect(topics).toContain('architecture');
		expect(topics).toContain('architecture/database');
		expect(topics).toContain('architecture/database/schema');
		expect(topics).toContain('architecture/api');
	});
});
