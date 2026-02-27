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
	readonly getVolumesForTopic: (topic: string) => Volume[];
	readonly deleteVolume?: (id: string) => Promise<void>;
	readonly getTotalVolumeCount?: () => number;
	readonly getAllTopics?: () => string[];
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
	if (!options.librarian && !options.registry) {
		throw new Error(
			'CirculationDesk requires either librarian or registry',
		);
	}

	const {
		addVolume,
		checkDuplicate,
		getVolumesForTopic,
		deleteVolume,
		getTotalVolumeCount,
		getAllTopics,
		catalog,
	} = options;
	const minEntries = options.thresholds?.compendium?.minEntries ?? 10;
	const maxVolumesPerTopic =
		options.thresholds?.reorganization?.maxVolumesPerTopic ?? 30;
	const optimizationConfig = options.thresholds?.optimization;
	const topicThreshold = optimizationConfig?.topicThreshold ?? 50;
	const globalThreshold = optimizationConfig?.globalThreshold ?? 500;
	const spawningConfig = options.thresholds?.spawning;

	const getDefaultLibrarian = (): Librarian => {
		if (options.librarian) return options.librarian;
		if (options.registry) return options.registry.defaultLibrarian.librarian;
		throw new Error(
			'CirculationDesk requires either librarian or registry',
		);
	};

	const resolveLibrarianForTopic = async (
		topic: string,
		content: string,
	): Promise<{ librarian: Librarian; name: string }> => {
		if (!options.registry) {
			return { librarian: getDefaultLibrarian(), name: 'default' };
		}
		const result = await options.registry.resolveLibrarian(topic, content);
		const managed = options.registry.get(result.winner);
		return {
			librarian: managed?.librarian ?? getDefaultLibrarian(),
			name: result.winner,
		};
	};

	const queue: Job[] = [];
	let isProcessing = false;
	let disposed = false;

	const checkEscalation = (topic: string): void => {
		if (!optimizationConfig || !deleteVolume) return;

		const topicVolumes = getVolumesForTopic(topic);
		if (topicVolumes.length >= topicThreshold) {
			queue.push({ type: 'optimization', topic });
			return;
		}

		if (getTotalVolumeCount && getAllTopics) {
			const total = getTotalVolumeCount();
			if (total >= globalThreshold) {
				for (const t of getAllTopics()) {
					queue.push({ type: 'optimization', topic: t });
				}
			}
		}
	};

	const checkSpawning = (topic: string): void => {
		if (!spawningConfig || !options.registry) return;

		const complexityThreshold = spawningConfig.complexityThreshold ?? 50;
		const topicVolumes = getVolumesForTopic(topic);
		if (topicVolumes.length >= complexityThreshold) {
			// Fire-and-forget specialist spawning
			options.registry
				.spawnSpecialist(topic, topicVolumes)
				.catch(() => {});
		}
	};

	const processJob = async (job: Job): Promise<void> => {
		try {
			switch (job.type) {
				case 'extraction': {
					const result = await getDefaultLibrarian().extract(
						job.turn,
					);
					const extractedTopics = new Set<string>();
					for (const mem of result.memories) {
						const dup = await checkDuplicate(mem.text);
						if (dup.isDuplicate) continue;

						const topic = catalog
							? catalog.resolve(mem.topic)
							: mem.topic;

						const resolved =
							await resolveLibrarianForTopic(topic, mem.text);

						await addVolume(mem.text, {
							topic,
							tags: mem.tags.join(','),
							entryType: mem.entryType,
							librarian: resolved.name,
						});
						extractedTopics.add(topic);
					}
					for (const topic of extractedTopics) {
						checkEscalation(topic);
						checkSpawning(topic);
					}
					break;
				}
				case 'compendium': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= minEntries) {
						const resolved = await resolveLibrarianForTopic(
							job.topic,
							job.topic,
						);
						await resolved.librarian.summarize(volumes, job.topic);
					}
					break;
				}
				case 'reorganization': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= maxVolumesPerTopic) {
						const resolved = await resolveLibrarianForTopic(
							job.topic,
							job.topic,
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
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length === 0) break;
					const resolved = await resolveLibrarianForTopic(
						job.topic,
						job.topic,
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
