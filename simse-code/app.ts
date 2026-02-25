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
	createMemoryManager,
	createPromptTemplate,
	type EmbeddingProvider,
	type LearningProfile,
	type Logger,
	type MemoryManager,
	type RetryOptions,
	type StorageBackend,
	type TextGenerationProvider,
} from 'simse';
import { type AgentService, createAgentService } from './agents.js';
import type { PromptConfig } from './config.js';
import { createToolService, type ToolService } from './tools.js';

// ---------------------------------------------------------------------------
// View types — UI-friendly projections of internal data
// ---------------------------------------------------------------------------

export interface NoteView {
	readonly id: string;
	readonly text: string;
	readonly topic: string;
	readonly metadata: Readonly<Record<string, string>>;
	readonly timestamp: number;
}

export interface SearchResultView {
	readonly note: NoteView;
	readonly score: number;
}

export interface TopicView {
	readonly topic: string;
	readonly noteCount: number;
}

// ---------------------------------------------------------------------------
// Generate options
// ---------------------------------------------------------------------------

export interface GenerateOptions extends ACPGenerateOptions {
	/** Skip memory context injection and result storage. Default: false. */
	readonly skipMemory?: boolean;
	/** Maximum memory results to inject as context. Default: 5. */
	readonly memoryMaxResults?: number;
	/** Minimum similarity score for memory results. Default: config threshold. */
	readonly memoryThreshold?: number;
}

export interface GenerateResult {
	/** The generated text content. */
	readonly content: string;
	/** The agent that produced the response. */
	readonly agentId: string;
	/** The server that handled the request. */
	readonly serverName: string;
	/** Memory entries injected as context (empty if skipMemory). */
	readonly memoryContext: readonly SearchResultView[];
	/** ID of the stored memory entry (undefined if skipMemory or storage failed). */
	readonly storedNoteId?: string;
	/** The full ACP result for advanced consumers. */
	readonly raw: ACPGenerateResult;
}

// ---------------------------------------------------------------------------
// Stream result
// ---------------------------------------------------------------------------

export interface MemoryStreamResult {
	/** Memory entries injected as context (empty if skipMemory). */
	readonly memoryContext: readonly SearchResultView[];
	/** The async generator of stream chunks. */
	readonly stream: AsyncGenerator<ACPStreamChunk>;
	/** Call after stream completes to store the full response in memory. */
	readonly storeResult: (fullText: string) => Promise<string | undefined>;
}

// ---------------------------------------------------------------------------
// Chain options
// ---------------------------------------------------------------------------

export interface ChainOptions {
	/** Skip memory storage of the chain output. Default: false. */
	readonly skipMemory?: boolean;
}

// ---------------------------------------------------------------------------
// App options
// ---------------------------------------------------------------------------

export interface AppOptions {
	/** Full application config (from defineConfig). */
	readonly config: AppConfig;
	/** Logger instance. */
	readonly logger: Logger;
	/** Storage backend for vector memory persistence. */
	readonly storage: StorageBackend;
	/** Embedding provider — required for all memory operations. */
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
	/** Default max memory results injected into generate() context. */
	readonly defaultMemoryResults?: number;
	/** Retry options for ACP client. */
	readonly retryOptions?: RetryOptions;
	/** Topic name for conversation Q&A pairs stored in memory. */
	readonly conversationTopic?: string;
	/** Topic name for chain results stored in memory. */
	readonly chainTopic?: string;
	/** System prompt prepended to all generate() calls. */
	readonly systemPrompt?: string;
	/** Max notes per topic before auto-summarizing oldest entries (0 = disabled). */
	readonly autoSummarizeThreshold?: number;
}

// ---------------------------------------------------------------------------
// App interface
// ---------------------------------------------------------------------------

export interface KnowledgeBaseApp {
	// Lifecycle
	readonly initialize: () => Promise<void>;
	readonly dispose: () => Promise<void>;

	// Notes
	readonly addNote: (
		text: string,
		topic: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly deleteNote: (id: string) => Promise<boolean>;
	readonly getNote: (id: string) => NoteView | undefined;
	readonly getAllNotes: () => readonly NoteView[];

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
	readonly getNotesByTopic: (topic: string) => readonly NoteView[];

	// Generation — memory-aware by default
	readonly generate: (
		prompt: string,
		options?: GenerateOptions,
	) => Promise<GenerateResult>;

	// Streaming — passthrough (no memory middleware)
	readonly generateStream: (
		prompt: string,
		options?: ACPGenerateOptions,
	) => AsyncGenerator<ACPStreamChunk>;

	// Streaming — memory-aware (search context, stream response, store after)
	readonly generateMemoryStream: (
		prompt: string,
		options?: GenerateOptions,
	) => Promise<MemoryStreamResult>;

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
	readonly getLearningProfile: () => LearningProfile | undefined;

	// Re-embed — clear and re-add all entries with the current embedding provider
	readonly reembed: (
		onProgress?: (done: number, total: number) => void,
	) => Promise<number>;

	// Services
	readonly agents: AgentService;
	readonly tools: ToolService;
	readonly memory: MemoryManager;

	// Stats
	readonly noteCount: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toNoteView(entry: {
	id: string;
	text: string;
	metadata: Record<string, string>;
	timestamp: number;
}): NoteView {
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

	const memory = createMemoryManager(embedder, config.memory, {
		storage: appOptions.storage,
		logger,
		textGenerator,
		vectorStoreOptions: {
			autoSave: appOptions.autoSave,
			flushIntervalMs: appOptions.flushIntervalMs,
			duplicateThreshold: appOptions.duplicateThreshold,
			duplicateBehavior: appOptions.duplicateBehavior,
		},
	});

	// -- Lifecycle ------------------------------------------------------------

	const initialize = async (): Promise<void> => {
		await memory.initialize();
		logger.info(`Knowledge base ready (${memory.size} existing notes)`);
	};

	const dispose = async (): Promise<void> => {
		await tools.disconnect();
		await memory.dispose();
	};

	// -- Notes ----------------------------------------------------------------

	const autoSummarizeThreshold = appOptions.autoSummarizeThreshold ?? 0;

	const autoSummarizeTopic = async (topic: string): Promise<void> => {
		if (autoSummarizeThreshold <= 0 || !textGenerator) return;

		const topicNotes = memory.filterByTopic([topic]);
		if (topicNotes.length <= autoSummarizeThreshold) return;

		// Summarize the oldest half, keep the newest half fresh
		const sorted = [...topicNotes].sort((a, b) => a.timestamp - b.timestamp);
		const cutoff = Math.floor(sorted.length / 2);
		const toSummarize = sorted.slice(0, cutoff);

		if (toSummarize.length < 2) return;

		try {
			await memory.summarize({
				ids: toSummarize.map((e) => e.id),
				deleteOriginals: true,
				metadata: { topic },
			});
			logger.debug(
				`Auto-summarized ${toSummarize.length} oldest notes in topic "${topic}"`,
			);
		} catch (err) {
			logger.warn('Auto-summarization failed', {
				topic,
				error: err instanceof Error ? err.message : String(err),
			});
		}
	};

	const addNote = async (
		text: string,
		topic: string,
		metadata?: Record<string, string>,
	): Promise<string> => {
		const id = await memory.add(text, { topic, ...metadata });
		await autoSummarizeTopic(topic);
		return id;
	};

	const deleteNote = (id: string): Promise<boolean> => memory.delete(id);

	const getNote = (id: string): NoteView | undefined => {
		const entry = memory.getById(id);
		return entry ? toNoteView(entry) : undefined;
	};

	const getAllNotes = (): readonly NoteView[] =>
		Object.freeze(memory.getAll().map(toNoteView));

	// -- Search & discovery ---------------------------------------------------

	const search = async (
		query: string,
		maxResults?: number,
	): Promise<readonly SearchResultView[]> => {
		const results = await memory.search(
			query,
			maxResults ?? appOptions.defaultSearchResults,
		);
		return Object.freeze(
			results.map((r) =>
				Object.freeze({ note: toNoteView(r.entry), score: r.score }),
			),
		);
	};

	const recommend = async (
		query: string,
		maxResults?: number,
	): Promise<readonly SearchResultView[]> => {
		const results = await memory.recommend(query, {
			maxResults: maxResults ?? appOptions.defaultRecommendResults,
		});
		return Object.freeze(
			results.map((r) =>
				Object.freeze({ note: toNoteView(r.entry), score: r.score }),
			),
		);
	};

	const getTopics = (): readonly TopicView[] => {
		return Object.freeze(
			memory
				.getTopics()
				.map((t) => Object.freeze({ topic: t.topic, noteCount: t.entryCount })),
		);
	};

	const getNotesByTopic = (topic: string): readonly NoteView[] => {
		return Object.freeze(memory.filterByTopic([topic]).map(toNoteView));
	};

	// -- Generate (memory-aware) ----------------------------------------------

	const generate = async (
		prompt: string,
		options?: GenerateOptions,
	): Promise<GenerateResult> => {
		const useMemory = !options?.skipMemory && memory.size > 0;
		let memoryContext: SearchResultView[] = [];
		let enrichedPrompt = prompt;

		// 1. Search memory for relevant context
		if (useMemory) {
			try {
				const maxResults =
					options?.memoryMaxResults ?? appOptions.defaultMemoryResults;
				const threshold =
					options?.memoryThreshold ?? config.memory.similarityThreshold;
				const results = await memory.search(prompt, maxResults, threshold);

				memoryContext = results.map((r) =>
					Object.freeze({ note: toNoteView(r.entry), score: r.score }),
				);

				if (memoryContext.length > 0) {
					const contextBlock = memoryContext
						.map(
							(r) =>
								`[${r.note.topic}] (relevance: ${r.score.toFixed(2)}) ${r.note.text}`,
						)
						.join('\n');

					enrichedPrompt = `Relevant context from memory:\n${contextBlock}\n\nUser query: ${prompt}`;
					logger.debug(
						`Injected ${memoryContext.length} memory entries as context`,
					);
				}
			} catch (err) {
				logger.warn('Memory search failed, generating without context', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		// 2. Generate via ACP
		const {
			skipMemory: _,
			memoryMaxResults: __,
			memoryThreshold: ___,
			...acpOpts
		} = options ?? {};
		const result = await agents.client.generate(enrichedPrompt, {
			...acpOpts,
			systemPrompt:
				acpOpts.systemPrompt ?? appOptions.systemPrompt ?? undefined,
		});

		// 3. Store the Q&A pair in memory
		let storedNoteId: string | undefined;
		if (!options?.skipMemory) {
			try {
				const convTopic = appOptions.conversationTopic ?? 'conversation';
				storedNoteId = await memory.add(`Q: ${prompt}\nA: ${result.content}`, {
					topic: convTopic,
					source: 'generate',
				});
				logger.debug('Stored generation result in memory', {
					noteId: storedNoteId,
				});
				await autoSummarizeTopic(convTopic);
			} catch (err) {
				logger.warn('Failed to store generation result in memory', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		return Object.freeze({
			content: result.content,
			agentId: result.agentId,
			serverName: result.serverName,
			memoryContext: Object.freeze(memoryContext),
			storedNoteId,
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

	// -- Memory-aware stream --------------------------------------------------

	const generateMemoryStream = async (
		prompt: string,
		options?: GenerateOptions,
	): Promise<MemoryStreamResult> => {
		const useMemory = !options?.skipMemory && memory.size > 0;
		let memoryContext: SearchResultView[] = [];
		let enrichedPrompt = prompt;

		// 1. Search memory for relevant context
		if (useMemory) {
			try {
				const maxResults =
					options?.memoryMaxResults ?? appOptions.defaultMemoryResults;
				const threshold =
					options?.memoryThreshold ?? config.memory.similarityThreshold;
				const results = await memory.search(prompt, maxResults, threshold);

				memoryContext = results.map((r) =>
					Object.freeze({ note: toNoteView(r.entry), score: r.score }),
				);

				if (memoryContext.length > 0) {
					const contextBlock = memoryContext
						.map(
							(r) =>
								`[${r.note.topic}] (relevance: ${r.score.toFixed(2)}) ${r.note.text}`,
						)
						.join('\n');

					enrichedPrompt = `Relevant context from memory:\n${contextBlock}\n\nUser query: ${prompt}`;
					logger.debug(
						`Injected ${memoryContext.length} memory entries as context`,
					);
				}
			} catch (err) {
				logger.warn('Memory search failed, streaming without context', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		// 2. Start stream
		const {
			skipMemory: _,
			memoryMaxResults: __,
			memoryThreshold: ___,
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
			if (options?.skipMemory) return undefined;
			try {
				const convTopic = appOptions.conversationTopic ?? 'conversation';
				const id = await memory.add(`Q: ${prompt}\nA: ${fullText}`, {
					topic: convTopic,
					source: 'generate',
				});
				logger.debug('Stored streamed result in memory', { noteId: id });
				await autoSummarizeTopic(convTopic);
				return id;
			} catch (err) {
				logger.warn('Failed to store streamed result in memory', {
					error: err instanceof Error ? err.message : String(err),
				});
				return undefined;
			}
		};

		return Object.freeze({
			memoryContext: Object.freeze(memoryContext),
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
			memoryManager: memory,
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

		// Auto-store chain output in memory
		if (!options?.skipMemory && output) {
			try {
				const chainTopic = appOptions.chainTopic ?? 'chain';
				await memory.add(`Chain result: ${output}`, {
					topic: chainTopic,
					source: 'chain',
					template,
				});
				logger.debug('Stored chain result in memory');
				await autoSummarizeTopic(chainTopic);
			} catch (err) {
				logger.warn('Failed to store chain result in memory', {
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
			memoryManager: memory,
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

		if (!options?.skipMemory && output) {
			try {
				const chainTopic = appOptions.chainTopic ?? 'chain';
				await memory.add(`Prompt "${name}" result: ${output}`, {
					topic: chainTopic,
					source: 'named-prompt',
					promptName: name,
				});
				logger.debug('Stored named prompt result in memory', { name });
				await autoSummarizeTopic(chainTopic);
			} catch (err) {
				logger.warn('Failed to store named prompt result in memory', {
					error: err instanceof Error ? err.message : String(err),
				});
			}
		}

		return output;
	};

	// -- Learning -------------------------------------------------------------

	const getLearningProfile = (): LearningProfile | undefined => {
		return memory.learningProfile;
	};

	// -- Re-embed -------------------------------------------------------------

	const reembed = async (
		onProgress?: (done: number, total: number) => void,
	): Promise<number> => {
		const allEntries = memory.getAll();
		if (allEntries.length === 0) return 0;

		// Snapshot text + metadata before clearing
		const snapshot = allEntries.map((e) => ({
			text: e.text,
			metadata: { ...e.metadata },
		}));

		const total = snapshot.length;

		// Clear all entries (drops old embeddings)
		await memory.clear();

		// Re-add in batches (re-embeds with the current provider)
		const batchSize = 50;
		for (let i = 0; i < total; i += batchSize) {
			const batch = snapshot.slice(i, i + batchSize);
			await memory.addBatch(batch);
			onProgress?.(Math.min(i + batchSize, total), total);
		}

		return total;
	};

	// -- Public API -----------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		addNote,
		deleteNote,
		getNote,
		getAllNotes,
		search,
		recommend,
		getTopics,
		getNotesByTopic,
		generate,
		generateStream,
		generateMemoryStream,
		runChain,
		runNamedPrompt,
		getLearningProfile,
		reembed,
		agents,
		tools,
		memory,
		get noteCount() {
			return memory.size;
		},
	});
}
