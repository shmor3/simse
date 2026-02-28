// ---------------------------------------------------------------------------
// Topic Catalog — hierarchical topic classification with normalization
// ---------------------------------------------------------------------------
//
// Manages a tree of topics with fuzzy matching (Levenshtein), aliases,
// volume tracking, merge, and relocate operations.
//
// Factory function, no classes.  Returns a frozen TopicCatalog interface.
// ---------------------------------------------------------------------------

import { levenshteinSimilarity } from './text-search.js';
import type { TopicCatalog, TopicCatalogSection } from './types.js';

export type { TopicCatalog, TopicCatalogSection } from './types.js';

export interface TopicCatalogOptions {
	/** Minimum Levenshtein similarity to match an existing topic. Defaults to 0.85. */
	readonly similarityThreshold?: number;
}

export function createTopicCatalog(
	options?: TopicCatalogOptions,
): TopicCatalog {
	const similarityThreshold = options?.similarityThreshold ?? 0.85;

	// topic -> Set<volumeId>
	const topicToVolumes = new Map<string, Set<string>>();
	// volumeId -> topic
	const volumeToTopic = new Map<string, string>();
	// alias -> canonical topic
	const aliases = new Map<string, string>();
	// topic -> Set<child topic>
	const topicToChildren = new Map<string, Set<string>>();

	const ensureTopicExists = (topic: string): void => {
		const normalized = topic.toLowerCase().trim();
		if (!topicToVolumes.has(normalized)) {
			topicToVolumes.set(normalized, new Set());
		}
		// Ensure all ancestors exist
		const parts = normalized.split('/');
		for (let i = 1; i < parts.length; i++) {
			const parent = parts.slice(0, i).join('/');
			const child = parts.slice(0, i + 1).join('/');
			if (!topicToVolumes.has(parent)) {
				topicToVolumes.set(parent, new Set());
			}
			let children = topicToChildren.get(parent);
			if (!children) {
				children = new Set();
				topicToChildren.set(parent, children);
			}
			children.add(child);
		}
	};

	const resolve = (proposedTopic: string): string => {
		const normalized = proposedTopic.toLowerCase().trim();

		// 1. Check aliases
		const aliased = aliases.get(normalized);
		if (aliased) return aliased;

		// 2. Check exact match
		if (topicToVolumes.has(normalized)) return normalized;

		// 3. Check similarity against existing topics
		let bestMatch: string | undefined;
		let bestScore = 0;
		for (const existing of topicToVolumes.keys()) {
			const score = levenshteinSimilarity(normalized, existing);
			if (score >= similarityThreshold && score > bestScore) {
				bestScore = score;
				bestMatch = existing;
			}
		}

		if (bestMatch) return bestMatch;

		// 4. Register as new topic
		ensureTopicExists(normalized);
		return normalized;
	};

	const registerVolume = (volumeId: string, topic: string): void => {
		const canonical = resolve(topic);
		// Remove from old topic if exists
		const oldTopic = volumeToTopic.get(volumeId);
		if (oldTopic) {
			topicToVolumes.get(oldTopic)?.delete(volumeId);
		}
		topicToVolumes.get(canonical)?.add(volumeId);
		volumeToTopic.set(volumeId, canonical);
	};

	const removeVolume = (volumeId: string): void => {
		const topic = volumeToTopic.get(volumeId);
		if (topic) {
			topicToVolumes.get(topic)?.delete(volumeId);
			volumeToTopic.delete(volumeId);
		}
	};

	const relocate = (volumeId: string, newTopic: string): void => {
		removeVolume(volumeId);
		registerVolume(volumeId, newTopic);
	};

	const merge = (sourceTopic: string, targetTopic: string): void => {
		const srcNorm = sourceTopic.toLowerCase().trim();
		const tgtNorm = resolve(targetTopic);
		const srcVolumes = topicToVolumes.get(srcNorm);
		if (!srcVolumes) return;

		const tgtVolumes = topicToVolumes.get(tgtNorm);
		if (!tgtVolumes) {
			ensureTopicExists(tgtNorm);
		}

		for (const volumeId of srcVolumes) {
			topicToVolumes.get(tgtNorm)?.add(volumeId);
			volumeToTopic.set(volumeId, tgtNorm);
		}

		srcVolumes.clear();
		// Add alias so future references to source go to target
		aliases.set(srcNorm, tgtNorm);
	};

	const sections = (): TopicCatalogSection[] => {
		const result: TopicCatalogSection[] = [];
		for (const [topic, vols] of topicToVolumes) {
			const parts = topic.split('/');
			const parent =
				parts.length > 1 ? parts.slice(0, -1).join('/') : undefined;
			const children = topicToChildren.get(topic);
			result.push({
				topic,
				parent,
				children: children ? [...children] : [],
				volumeCount: vols.size,
			});
		}
		return result;
	};

	const volumes = (topic: string): readonly string[] => {
		const normalized = topic.toLowerCase().trim();
		const vols = topicToVolumes.get(normalized);
		return vols ? [...vols] : [];
	};

	const addAlias = (alias: string, canonical: string): void => {
		aliases.set(alias.toLowerCase().trim(), canonical.toLowerCase().trim());
	};

	const getTopicForVolume = (volumeId: string): string | undefined => {
		return volumeToTopic.get(volumeId);
	};

	return Object.freeze({
		resolve,
		relocate,
		merge,
		sections,
		volumes,
		addAlias,
		registerVolume,
		removeVolume,
		getTopicForVolume,
	});
}
