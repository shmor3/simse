import type { LibrarianRegistry } from './librarian-registry.js';
import type {
	CirculationDesk,
	CirculationDeskThresholds,
	DuplicateCheckResult,
	Librarian,
	TurnContext,
	Volume,
} from './types.js';

export interface CirculationDeskOptions {
	readonly librarian?: Librarian;
	readonly registry?: LibrarianRegistry;
	readonly addVolume: (
		text: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
	readonly getVolumesForTopic: (topic: string) => Promise<Volume[]>;
	readonly deleteVolume?: (id: string) => Promise<void>;
	readonly getTotalVolumeCount?: () => Promise<number>;
	readonly getAllTopics?: () => Promise<string[]>;
	readonly thresholds?: CirculationDeskThresholds;
	readonly catalog?: import('./types.js').TopicCatalog;
}

type Job =
	| { type: 'extraction'; turn: TurnContext }
	| { type: 'compendium'; topic: string }
	| { type: 'reorganization'; topic: string }
	| { type: 'optimization'; topic: string };

export function createCirculationDesk(
	options: CirculationDeskOptions,
): CirculationDesk {
	const {
		librarian,
		registry,
		addVolume,
		checkDuplicate,
		getVolumesForTopic,
		deleteVolume,
		getTotalVolumeCount,
		getAllTopics,
		catalog,
	} = options;

	if (!librarian && !registry) {
		throw new Error('CirculationDesk requires either librarian or registry');
	}

	// Default librarian: explicit or from registry.
	// The guard above ensures at least one is defined.
	const defaultLibrarian: Librarian =
		// biome-ignore lint/style/noNonNullAssertion: guard above ensures registry exists when librarian is absent
		librarian ?? registry!.defaultLibrarian.librarian;

	const minEntries = options.thresholds?.compendium?.minEntries ?? 10;
	const maxVolumesPerTopic =
		options.thresholds?.reorganization?.maxVolumesPerTopic ?? 30;
	const optimizationConfig = options.thresholds?.optimization;
	const topicThreshold = optimizationConfig?.topicThreshold ?? 50;
	const globalThreshold = optimizationConfig?.globalThreshold ?? 500;

	const queue: Job[] = [];
	let isProcessing = false;
	let disposed = false;

	const checkEscalation = async (topic: string): Promise<void> => {
		if (!optimizationConfig || !deleteVolume) return;

		const topicVolumes = await getVolumesForTopic(topic);
		if (topicVolumes.length >= topicThreshold) {
			queue.push({ type: 'optimization', topic });
			return;
		}

		if (getTotalVolumeCount && getAllTopics) {
			const total = await getTotalVolumeCount();
			if (total >= globalThreshold) {
				for (const t of await getAllTopics()) {
					queue.push({ type: 'optimization', topic: t });
				}
			}
		}
	};

	/**
	 * Resolve the librarian to use for a given topic/content pair.
	 * When a registry is available, it arbitrates among registered librarians.
	 * Otherwise falls back to the default librarian.
	 */
	const resolveLibrarianForJob = async (
		topic: string,
		content: string,
	): Promise<{ librarian: Librarian; name: string }> => {
		if (!registry) {
			return { librarian: defaultLibrarian, name: 'default' };
		}
		const result = await registry.resolveLibrarian(topic, content);
		const managed = registry.get(result.winner);
		if (managed) {
			return { librarian: managed.librarian, name: result.winner };
		}
		return { librarian: defaultLibrarian, name: 'default' };
	};

	const spawningConfig = options.thresholds?.spawning;

	const checkSpawning = async (topic: string): Promise<void> => {
		if (!registry || !spawningConfig) return;
		const volumes = await getVolumesForTopic(topic);
		const threshold = spawningConfig.complexityThreshold ?? 100;
		if (volumes.length >= threshold) {
			await registry.spawnSpecialist(topic, volumes);
		}
	};

	const processJob = async (job: Job): Promise<void> => {
		try {
			switch (job.type) {
				case 'extraction': {
					const result = await defaultLibrarian.extract(job.turn);
					const extractedTopics = new Set<string>();
					for (const mem of result.memories) {
						const dup = await checkDuplicate(mem.text);
						if (dup.isDuplicate) continue;

						const topic = catalog ? catalog.resolve(mem.topic) : mem.topic;

						// Route through registry to determine which librarian owns the content
						const resolved = registry
							? await resolveLibrarianForJob(topic, mem.text)
							: null;

						await addVolume(mem.text, {
							topic,
							tags: mem.tags.join(','),
							entryType: mem.entryType,
							...(resolved && { librarian: resolved.name }),
						});
						extractedTopics.add(topic);
					}
					for (const topic of extractedTopics) {
						await checkEscalation(topic);
						await checkSpawning(topic);
					}
					break;
				}
				case 'compendium': {
					const volumes = await getVolumesForTopic(job.topic);
					if (volumes.length >= minEntries) {
						const resolved = await resolveLibrarianForJob(
							job.topic,
							volumes.map((v) => v.text).join('\n'),
						);
						await resolved.librarian.summarize(volumes, job.topic);
					}
					break;
				}
				case 'reorganization': {
					const volumes = await getVolumesForTopic(job.topic);
					if (volumes.length >= maxVolumesPerTopic) {
						const resolved = await resolveLibrarianForJob(
							job.topic,
							volumes.map((v) => v.text).join('\n'),
						);
						const plan = await resolved.librarian.reorganize(
							job.topic,
							volumes,
						);
						if (catalog) {
							for (const move of plan.moves) {
								catalog.relocate(move.volumeId, move.newTopic);
							}
							for (const merge of plan.merges) {
								catalog.merge(merge.source, merge.target);
							}
						}
					}
					break;
				}
				case 'optimization': {
					if (!deleteVolume || !optimizationConfig) break;
					const volumes = await getVolumesForTopic(job.topic);
					if (volumes.length === 0) break;
					const resolved = await resolveLibrarianForJob(
						job.topic,
						volumes.map((v) => v.text).join('\n'),
					);
					const result = await resolved.librarian.optimize(
						volumes,
						job.topic,
						optimizationConfig.modelId,
					);
					for (const id of result.pruned) {
						await deleteVolume(id);
					}
					if (result.summary) {
						await addVolume(result.summary, {
							topic: job.topic,
							entryType: 'compendium',
						});
					}
					if (catalog) {
						for (const move of result.reorganization.moves) {
							catalog.relocate(move.volumeId, move.newTopic);
						}
						for (const merge of result.reorganization.merges) {
							catalog.merge(merge.source, merge.target);
						}
					}
					break;
				}
			}
		} catch {
			// Failed jobs are logged and dropped (fire-and-forget)
		}
	};

	const drain = async (): Promise<void> => {
		if (isProcessing || disposed) return;
		isProcessing = true;
		try {
			while (queue.length > 0) {
				// biome-ignore lint/style/noNonNullAssertion: length check guarantees element exists
				const job = queue.shift()!;
				await processJob(job);
			}
		} finally {
			isProcessing = false;
		}
	};

	const flush = async (): Promise<void> => {
		queue.length = 0;
	};

	const dispose = (): void => {
		disposed = true;
		queue.length = 0;
	};

	return Object.freeze({
		enqueueExtraction: (turn: TurnContext) => {
			if (disposed) return;
			queue.push({ type: 'extraction', turn });
		},
		enqueueCompendium: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'compendium', topic });
		},
		enqueueReorganization: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'reorganization', topic });
		},
		enqueueOptimization: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'optimization', topic });
		},
		drain,
		flush,
		dispose,
		get pending() {
			return queue.length;
		},
		get processing() {
			return isProcessing;
		},
	});
}
