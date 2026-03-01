/**
 * SimSE CLI — Knowledge Base Application
 *
 * A complete knowledge base application with a clean API that any
 * interface (web, CLI, TUI) can consume.
 */

import {
	type ACPGenerateOptions,
	type ACPGenerateResult,
	type ACPStreamChunk,
	type AppConfig,
	createChain,
	createLibrary,
	createPromptTemplate,
	createTaskList,
	type EmbeddingProvider,
	type Library,
	type Logger,
	type PatronProfile,
	type RetryOptions,
	type StorageBackend,
	type TaskList,
	type TextGenerationProvider,
} from 'simse';
import { type AgentService, createAgentService } from './agents.js';
import type { PromptConfig } from './config.js';
import { createToolService, type ToolService } from './tools.js';

// ---------------------------------------------------------------------------
// View types — UI-friendly projections of internal data
// ---------------------------------------------------------------------------

export interface VolumeView {
	readonly id: string;
	readonly text: string;
	readonly topic: string;
	readonly metadata: Readonly<Record<string, string>>;
	readonly timestamp: number;
}

export interface SearchResultView {
	readonly volume: VolumeView;
	readonly score: number;
}

export interface TopicView {
	readonly topic: string;
	readonly volumeCount: number;
}

// ---------------------------------------------------------------------------
// Generate options
// ---------------------------------------------------------------------------

export interface GenerateOptions extends ACPGenerateOptions {
	/** Skip library context injection and result storage. Default: false. */
	readonly skipLibrary?: boolean;
	/** Maximum library results to inject as context. Default: 5. */
	readonly libraryMaxResults?: number;
	/** Minimum similarity score for library results. Default: config threshold. */
	readonly libraryThreshold?: number;
}

export interface GenerateResult {
	/** The generated text content. */
	readonly content: string;
	/** The agent that produced the response. */
	readonly agentId: string;
	/** The server that handled the request. */
	readonly serverName: string;
	/** Library volumes injected as context (empty if skipLibrary). */
	readonly libraryContext: readonly SearchResultView[];
	/** ID of the stored volume (undefined if skipLibrary or storage failed). */
	readonly storedVolumeId?: string;
	/** The full ACP result for advanced consumers. */
	readonly raw: ACPGenerateResult;
}

// ---------------------------------------------------------------------------
// Stream result
// ---------------------------------------------------------------------------

export interface LibraryStreamResult {
	/** Library volumes injected as context (empty if skipLibrary). */
	readonly libraryContext: readonly SearchResultView[];
	/** The async generator of stream chunks. */
	readonly stream: AsyncGenerator<ACPStreamChunk>;
	/** Call after stream completes to store the full response in the library. */
	readonly storeResult: (fullText: string) => Promise<string | undefined>;
}

// ---------------------------------------------------------------------------
// Chain options
// ---------------------------------------------------------------------------

export interface ChainOptions {
	/** Skip library storage of the chain output. Default: false. */
	readonly skipLibrary?: boolean;
}

// ---------------------------------------------------------------------------
// App options
// ---------------------------------------------------------------------------

export interface AppOptions {
	/** Full application config (from defineConfig). */
	readonly config: AppConfig;
	/** Logger instance. */
	readonly logger: Logger;
	/** Storage backend for library persistence. */
	readonly storage: StorageBackend;
	/** Embedding provider — required for all library operations. */
	readonly embedder: EmbeddingProvider;
	/** Text generation provider — required for summarization. */
	readonly textGenerator?: TextGenerationProvider;
	/** Duplicate detection threshold (0-1). */
	readonly duplicateThreshold?: number;
	/** Duplicate detection behavior: skip, warn, or error. */
	readonly duplicateBehavior?: 'skip' | 'warn' | 'error';
	/** Whether vector store auto-saves on every mutation. */
	readonly autoSave?: boolean;
	/** Auto-flush interval in ms (0 = disabled, only used when autoSave is false). */
	readonly flushIntervalMs?: number;
	/** Default max results for search(). */
	readonly defaultSearchResults?: number;
	/** Default max results for recommend(). */
	readonly defaultRecommendResults?: number;
	/** Default max library results injected into generate() context. */
	readonly defaultLibraryResults?: number;
	/** Retry options for ACP client. */
	readonly retryOptions?: RetryOptions;
	/** Topic name for conversation Q&A pairs stored in the library. */
	readonly conversationTopic?: string;
	/** Topic name for chain results stored in the library. */
	readonly chainTopic?: string;
	/** System prompt prepended to all generate() calls. */
	readonly systemPrompt?: string;
	/** Max volumes per topic before auto-summarizing oldest entries (0 = disabled). */
	readonly autoSummarizeThreshold?: number;
}

// ---------------------------------------------------------------------------
// App interface
// ---------------------------------------------------------------------------

export interface KnowledgeBaseApp {
	// Lifecycle
	readonly initialize: () => Promise<void>;
	readonly dispose: () => Promise<void>;

	// Volumes
	readonly addVolume: (
		text: string,
		topic: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly deleteVolume: (id: string) => Promise<boolean>;
	readonly getVolume: (id: string) => VolumeView | undefined;
	readonly getAllVolumes: () => readonly VolumeView[];

	// Search & discovery
	readonly search: (
		query: string,
		maxResults?: number,
	) => Promise<readonly SearchResultView[]>;
	readonly recommend: (
		query: string,
		maxResults?: number,
	) => Promise<readonly SearchResultView[]>;
	readonly getTopics: () => readonly TopicView[];
	readonly getVolumesByTopic: (topic: string) => readonly VolumeView[];

	// Generation — library-aware by default
	readonly generate: (
		prompt: string,
		options?: GenerateOptions,
	) => Promise<GenerateResult>;

	// Streaming — passthrough (no library services)
	readonly generateStream: (
		prompt: string,
		options?: ACPGenerateOptions,
	) => AsyncGenerator<ACPStreamChunk>;

	// Streaming — library-aware (search context, stream response, store after)
	readonly generateLibraryStream: (
		prompt: string,
		options?: GenerateOptions,
	) => Promise<LibraryStreamResult>;

	// Chain — run a prompt through the ACP pipeline
	readonly runChain: (
		template: string,
		values: Record<string, string>,
		options?: ChainOptions,
	) => Promise<string>;

	// Named prompts — run a named prompt from project config
	readonly runNamedPrompt: (
		name: string,
		prompt: PromptConfig,
		values: Record<string, string>,
		options?: ChainOptions,
	) => Promise<string>;

	// Learning
	readonly getPatronProfile: () => PatronProfile | undefined;

	// Re-embed — clear and re-add all entries with the current embedding provider
	readonly reembed: (
		onProgress?: (done: number, total: number) => void,
	) => Promise<number>;

	// Services
	readonly agents: AgentService;
	readonly tools: ToolService;
	readonly library: Library;
	readonly tasks: TaskList;

	// Stats
	readonly volumeCount: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toVolumeView(entry: {
	id: string;
	text: string;
	metadata: Record<string, string>;
	timestamp: number;
}): VolumeView {
	return Object.freeze({
		id: entry.id,
		text: entry.text,
		topic: entry.metadata.topic ?? 'uncategorized',
		metadata: entry.metadata,
		timestamp: entry.timestamp,
	});
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createApp(appOptions: AppOptions): KnowledgeBaseApp {
	const { config, logger, embedder, textGenerator } = appOptions;

	const agents = createAgentService({
		config: config.acp,
		logger,
		clientOptions: appOptions.retryOptions
			? { retryOptions: appOptions.retryOptions }
			: undefined,
	});

	const tools = createToolService({
		mcpClientConfig: config.mcp.client,
		mcpServerConfig: config.mcp.server,
		acpClient: agents.client,
		logger,
	});

	const tasks = createTaskList();

	const library = createLibrary(embedder, config.memory, {
		storage: appOptions.storage,
		logger,
		textGenerator,
		stacksOptions: {
			autoSave: appOptions.autoSave,
			flushIntervalMs: appOptions.flushIntervalMs,
			duplicateThreshold: appOptions.duplicateThreshold,
			duplicateBehavior: appOptions.duplicateBehavior,
		},
	});

	// -- Lifecycle ------------------------------------------------------------

	const initialize = async (): Promise<void> => {
		await library.initialize();
		logger.info(`Knowledge base ready (${library.size} existing volumes)`);
	};

	const dispose = async (): Promise<void> => {
		await tools.disconnect();
		await library.dispose();
	};

	// -- Volumes --------------------------------------------------------------

	const autoSummarizeThreshold = appOptions.autoSummarizeThreshold ?? 0;

	const autoSummarizeTopic = async (topic: string): Promise<void> => {
		if (autoSummarizeThreshold <= 0 || !textGenerator) return;

		const topicVolumes = library.filterByTopic([topic]);
		if (topicVolumes.length <= autoSummarizeThreshold) return;

		// Summarize the oldest half, keep the newest half fresh
		const sorted = [...topicVolumes].sort((a, b) => a.timestamp - b.timestamp);
		const cutoff = Math.floor(sorted.length / 2);
		const toSummarize = sorted.slice(0, cutoff);

		if (toSummarize.length < 2) return;

		try {
			await library.compendium({
				ids: toSummarize.map((e) => e.id),
				deleteOriginals: true,
				metadata: { topic },
			});
			logger.debug(
				`Auto-summarized ${toSummarize.length} oldest volumes in topic "${topic}"`,
			);
		} catch (err) {
			logger.warn('Auto-summarization failed', {
				topic,
				error: err instanceof Error ? err.message : String(err),
			});
		}
	};

	const addVolume = async (
		text: string,
		topic: string,
		metadata?: Record<string, string>,
	): Promise<string> => {
		const id = await library.add(text, { topic, ...metadata });
		await autoSummarizeTopic(topic);
		return id;
	};

	const deleteVolume = (id: string): Promise<boolean> => library.delete(id);

	const getVolume = (id: string): VolumeView | undefined => {
		const entry = library.getById(id);
		return entry ? toVolumeView(entry) : undefined;
	};

	const getAllVolumes = (): readonly VolumeView[] =>
		Object.freeze(library.getAll().map(toVolumeView));

	// -- Search & discovery ---------------------------------------------------

	const search = async (
		query: string,
		maxResults?: number,
	): Promise<readonly SearchResultView[]> => {
		const results = await library.search(
			query,
			maxResults ?? appOptions.defaultSearchResults,
		);
		return Object.freeze(
			results.map((r) =>
				Object.freeze({ volume: toVolumeView(r.volume), score: r.score }),
			),
		);
	};

	const recommend = async (
		query: string,
		maxResults?: number,
	): Promise<readonly SearchResultView[]> => {
		const results = await library.recommend(query, {
			maxResults: maxResults ?? appOptions.defaultRecommendResults,
		});
		return Object.freeze(
			results.map((r) =>
				Object.freeze({ volume: toVolumeView(r.volume), score: r.score }),
			),
		);
	};

	const getTopics = (): readonly TopicView[] => {
		return Object.freeze(
			library
				.getTopics()
				.map((t) =>
					Object.freeze({ topic: t.topic, volumeCount: t.entryCount }),
				),
		);
	};

	const getVolumesByTopic = (topic: string): readonly VolumeView[] => {
		return Object.freeze(library.filterByTopic([topic]).map(toVolumeView));
	};

	// -- Generate (library-aware) ----------------------------------------------

	const generate = async (
		prompt: string,
		options?: GenerateOptions,
	): Promise<GenerateResult> => {
		const useLibrary = !options?.skipLibrary && library.size > 0;
		let libraryContext: SearchResultView[] = [];
		let enrichedPrompt = prompt;

		// 1. Search library for relevant context
		if (useLibrary) {
			try {
				const maxResults =
					options?.libraryMaxResults ?? appOptions.defaultLibraryResults;
				const threshold =
					options?.libraryThreshold ?? config.memory.similarityThreshold;
				const results = await library.search(prompt, maxResults, threshold);

				libraryContext = results.map((r) =>
					Object.freeze({ volume: toVolumeView(r.volume), score: r.score }),
				);

				if (libraryContext.length > 0) {
					const contextBlock = libraryContext
						.map(
							(r) =>
								`[${r.volume.topic}] (relevance: ${r.score.toFixed(2)}) ${r.volume.text}`,
						)
						.join('\n');

					enrichedPrompt = `Relevant context from library:\n${contextBlock}\n\nUser query: ${prompt}`;
					logger.debug(
						`Injected ${libraryContext.length} library volumes as context`,
					);
				}
			} catch (err) {
				logger.warn('Library search failed, generating without context', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		// 2. Generate via ACP
		const {
			skipLibrary: _,
			libraryMaxResults: __,
			libraryThreshold: ___,
			...acpOpts
		} = options ?? {};
		const result = await agents.client.generate(enrichedPrompt, {
			...acpOpts,
			systemPrompt:
				acpOpts.systemPrompt ?? appOptions.systemPrompt ?? undefined,
		});

		// 3. Store the Q&A pair in the library
		let storedVolumeId: string | undefined;
		if (!options?.skipLibrary) {
			try {
				const convTopic = appOptions.conversationTopic ?? 'conversation';
				storedVolumeId = await library.add(
					`Q: ${prompt}\nA: ${result.content}`,
					{
						topic: convTopic,
						source: 'generate',
					},
				);
				logger.debug('Stored generation result in library', {
					volumeId: storedVolumeId,
				});
				await autoSummarizeTopic(convTopic);
			} catch (err) {
				logger.warn('Failed to store generation result in library', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		return Object.freeze({
			content: result.content,
			agentId: result.agentId,
			serverName: result.serverName,
			libraryContext: Object.freeze(libraryContext),
			storedVolumeId,
			raw: result,
		});
	};

	// -- Stream (passthrough) -------------------------------------------------

	const generateStream = (
		prompt: string,
		options?: ACPGenerateOptions,
	): AsyncGenerator<ACPStreamChunk> => {
		return agents.client.generateStream(prompt, options);
	};

	// -- Library-aware stream -------------------------------------------------

	const generateLibraryStream = async (
		prompt: string,
		options?: GenerateOptions,
	): Promise<LibraryStreamResult> => {
		const useLibrary = !options?.skipLibrary && library.size > 0;
		let libraryContext: SearchResultView[] = [];
		let enrichedPrompt = prompt;

		// 1. Search library for relevant context
		if (useLibrary) {
			try {
				const maxResults =
					options?.libraryMaxResults ?? appOptions.defaultLibraryResults;
				const threshold =
					options?.libraryThreshold ?? config.memory.similarityThreshold;
				const results = await library.search(prompt, maxResults, threshold);

				libraryContext = results.map((r) =>
					Object.freeze({ volume: toVolumeView(r.volume), score: r.score }),
				);

				if (libraryContext.length > 0) {
					const contextBlock = libraryContext
						.map(
							(r) =>
								`[${r.volume.topic}] (relevance: ${r.score.toFixed(2)}) ${r.volume.text}`,
						)
						.join('\n');

					enrichedPrompt = `Relevant context from library:\n${contextBlock}\n\nUser query: ${prompt}`;
					logger.debug(
						`Injected ${libraryContext.length} library volumes as context`,
					);
				}
			} catch (err) {
				logger.warn('Library search failed, streaming without context', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		// 2. Start stream
		const {
			skipLibrary: _,
			libraryMaxResults: __,
			libraryThreshold: ___,
			...acpOpts
		} = options ?? {};
		const stream = agents.client.generateStream(enrichedPrompt, {
			...acpOpts,
			systemPrompt:
				acpOpts.systemPrompt ?? appOptions.systemPrompt ?? undefined,
		});

		// 3. Callback to store after stream completes
		const storeResult = async (
			fullText: string,
		): Promise<string | undefined> => {
			if (options?.skipLibrary) return undefined;
			try {
				const convTopic = appOptions.conversationTopic ?? 'conversation';
				const id = await library.add(`Q: ${prompt}\nA: ${fullText}`, {
					topic: convTopic,
					source: 'generate',
				});
				logger.debug('Stored streamed result in library', { volumeId: id });
				await autoSummarizeTopic(convTopic);
				return id;
			} catch (err) {
				logger.warn('Failed to store streamed result in library', {
					error: err instanceof Error ? err.message : String(err),
				});
				return undefined;
			}
		};

		return Object.freeze({
			libraryContext: Object.freeze(libraryContext),
			stream,
			storeResult,
		});
	};

	// -- Chain ----------------------------------------------------------------

	const runChain = async (
		template: string,
		values: Record<string, string>,
		options?: ChainOptions,
	): Promise<string> => {
		const chain = createChain({
			acpClient: agents.client,
			mcpClient: tools.mcpClient,
			library,
			logger,
		});

		const vars = [...template.matchAll(/\{([\w-]+)\}/g)].map((m) => m[1]);
		const stepName = vars[0] ?? 'step';

		chain.addStep({
			name: stepName,
			template: createPromptTemplate(template),
		});

		const results = await chain.run(values);
		const output = results.at(-1)?.output ?? '';

		// Auto-store chain output in library
		if (!options?.skipLibrary && output) {
			try {
				const chainTopic = appOptions.chainTopic ?? 'chain';
				await library.add(`Chain result: ${output}`, {
					topic: chainTopic,
					source: 'chain',
					template,
				});
				logger.debug('Stored chain result in library');
				await autoSummarizeTopic(chainTopic);
			} catch (err) {
				logger.warn('Failed to store chain result in library', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		return output;
	};

	// -- Named prompts --------------------------------------------------------

	const runNamedPrompt = async (
		name: string,
		promptConfig: PromptConfig,
		values: Record<string, string>,
		options?: ChainOptions,
	): Promise<string> => {
		const chain = createChain({
			acpClient: agents.client,
			mcpClient: tools.mcpClient,
			library,
			logger,
		});

		for (const step of promptConfig.steps) {
			chain.addStep({
				name: step.name,
				template: createPromptTemplate(step.template),
				systemPrompt:
					step.systemPrompt ?? promptConfig.systemPrompt ?? undefined,
				agentId: step.agentId ?? promptConfig.agentId ?? undefined,
				serverName: step.serverName ?? promptConfig.serverName ?? undefined,
				inputMapping: step.inputMapping ? { ...step.inputMapping } : undefined,
				storeToMemory: step.storeToMemory,
				memoryMetadata: step.memoryMetadata
					? { ...step.memoryMetadata }
					: undefined,
			});
		}

		const results = await chain.run(values);
		const output = results.at(-1)?.output ?? '';

		if (!options?.skipLibrary && output) {
			try {
				const chainTopic = appOptions.chainTopic ?? 'chain';
				await library.add(`Prompt "${name}" result: ${output}`, {
					topic: chainTopic,
					source: 'named-prompt',
					promptName: name,
				});
				logger.debug('Stored named prompt result in library', { name });
				await autoSummarizeTopic(chainTopic);
			} catch (err) {
				logger.warn('Failed to store named prompt result in library', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		return output;
	};

	// -- Learning -------------------------------------------------------------

	const getPatronProfile = (): PatronProfile | undefined => {
		return library.patronProfile;
	};

	// -- Re-embed -------------------------------------------------------------

	const reembed = async (
		onProgress?: (done: number, total: number) => void,
	): Promise<number> => {
		const allEntries = library.getAll();
		if (allEntries.length === 0) return 0;

		// Snapshot text + metadata before clearing
		const snapshot = allEntries.map((e) => ({
			text: e.text,
			metadata: { ...e.metadata },
		}));

		const total = snapshot.length;

		// Clear all entries (drops old embeddings)
		await library.clear();

		// Re-add in batches (re-embeds with the current provider)
		const batchSize = 50;
		for (let i = 0; i < total; i += batchSize) {
			const batch = snapshot.slice(i, i + batchSize);
			await library.addBatch(batch);
			onProgress?.(Math.min(i + batchSize, total), total);
		}

		return total;
	};

	// -- Public API -----------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		addVolume,
		deleteVolume,
		getVolume,
		getAllVolumes,
		search,
		recommend,
		getTopics,
		getVolumesByTopic,
		generate,
		generateStream,
		generateLibraryStream,
		runChain,
		runNamedPrompt,
		getPatronProfile,
		reembed,
		agents,
		tools,
		library,
		tasks,
		get volumeCount() {
			return library.size;
		},
	});
}
