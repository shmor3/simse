import type {
	CirculationDesk,
	CirculationDeskThresholds,
	DuplicateCheckResult,
	Librarian,
	TurnContext,
	Volume,
} from './types.js';

export interface CirculationDeskOptions {
	readonly librarian: Librarian;
	readonly addVolume: (
		text: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
	readonly getVolumesForTopic: (topic: string) => Volume[];
	readonly thresholds?: CirculationDeskThresholds;
	readonly catalog?: import('./types.js').TopicCatalog;
}

type Job =
	| { type: 'extraction'; turn: TurnContext }
	| { type: 'compendium'; topic: string }
	| { type: 'reorganization'; topic: string };

export function createCirculationDesk(
	options: CirculationDeskOptions,
): CirculationDesk {
	const {
		librarian,
		addVolume,
		checkDuplicate,
		getVolumesForTopic,
		catalog,
	} = options;
	const minEntries = options.thresholds?.compendium?.minEntries ?? 10;
	const maxVolumesPerTopic =
		options.thresholds?.reorganization?.maxVolumesPerTopic ?? 30;

	const queue: Job[] = [];
	let isProcessing = false;
	let disposed = false;

	const processJob = async (job: Job): Promise<void> => {
		try {
			switch (job.type) {
				case 'extraction': {
					const result = await librarian.extract(job.turn);
					for (const mem of result.memories) {
						const dup = await checkDuplicate(mem.text);
						if (dup.isDuplicate) continue;

						const topic = catalog
							? catalog.resolve(mem.topic)
							: mem.topic;

						await addVolume(mem.text, {
							topic,
							tags: mem.tags.join(','),
							entryType: mem.entryType,
						});
					}
					break;
				}
				case 'compendium': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= minEntries) {
						await librarian.summarize(volumes, job.topic);
					}
					break;
				}
				case 'reorganization': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= maxVolumesPerTopic) {
						const plan = await librarian.reorganize(
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
