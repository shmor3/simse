/**
 * SimSE — Basic Usage Example
 *
 * Demonstrates the core features:
 * - Configuration with defineConfig()
 * - ACP client for text generation and streaming
 * - Multi-step chains with prompt templates
 * - Named chains from config
 * - Vector memory store (search, topics, deduplication, recommendations)
 * - Memory manager with summarization
 * - Retry with exponential backoff
 * - Logging
 *
 * Prerequisites:
 *   An ACP-compatible server running at http://localhost:8000
 *   with a "default" agent configured (only needed for ACP/chain demos).
 *
 * Run:
 *   bun run example/basic.ts
 */

import {
	createACPClient,
	createChain,
	createConsoleTransport,
	createLogger,
	createMemoryManager,
	createPromptTemplate,
	createVectorStore,
	defineConfig,
	type EmbeddingProvider,
	isSimseError,
	isTransientError,
	retry,
	runNamedChain,
	type TextGenerationProvider,
} from '../src/lib.js';

// ---------------------------------------------------------------------------
// 1. Configuration
// ---------------------------------------------------------------------------

const config = defineConfig({
	acp: {
		servers: [
			{
				name: 'local',
				url: 'http://localhost:8000',
				defaultAgent: 'default',
			},
		],
	},
	memory: {
		embeddingAgent: 'default',
		storePath: '.simse-example/memory',
	},
	chains: {
		summarize: {
			description: 'Summarize a piece of text',
			steps: [
				{
					name: 'summarize',
					template: 'Summarize the following in 2-3 sentences:\n\n{text}',
				},
			],
		},
	},
});

// ---------------------------------------------------------------------------
// 2. Logger
// ---------------------------------------------------------------------------

const logger = createLogger({
	context: 'example',
	level: 'debug',
	transports: [createConsoleTransport()],
});

logger.info('Configuration loaded', { servers: config.acp.servers.length });

// ---------------------------------------------------------------------------
// 3. ACP Client — generate & stream
// ---------------------------------------------------------------------------

async function demonstrateACPClient() {
	logger.info('--- ACP Client Demo ---');

	const client = createACPClient(config.acp, { logger });

	// Check server availability
	const available = await client.isAvailable('local');
	logger.info('Server available', { available });

	if (!available) {
		logger.warn('ACP server not available, skipping ACP demos');
		return client;
	}

	// List agents
	const agents = await client.listAgents('local');
	logger.info('Available agents', { count: agents.length });

	// Single generation
	const result = await client.generate('What is TypeScript in one sentence?', {
		systemPrompt: 'You are concise and precise.',
	});
	logger.info('Generated', { content: result.content });

	// Streaming
	logger.info('Streaming response...');
	process.stdout.write('  ');
	for await (const event of client.generateStream(
		'Count from 1 to 5, one number per line.',
	)) {
		process.stdout.write(event.delta);
	}
	process.stdout.write('\n');

	return client;
}

// ---------------------------------------------------------------------------
// 4. Chains — multi-step pipelines
// ---------------------------------------------------------------------------

async function demonstrateChains(client: ReturnType<typeof createACPClient>) {
	logger.info('--- Chain Demo ---');

	// Build a chain programmatically
	const chain = createChain({ acpClient: client, logger });

	chain.addStep({
		name: 'brainstorm',
		template: createPromptTemplate('List 3 interesting facts about {topic}.'),
		systemPrompt: 'You are a knowledgeable researcher.',
	});

	chain.addStep({
		name: 'article',
		template: createPromptTemplate(
			'Write a short paragraph using these facts:\n\n{brainstorm}',
		),
		inputMapping: { brainstorm: 'brainstorm' },
		systemPrompt: 'You are a professional writer.',
	});

	chain.setCallbacks({
		onStepStart: ({ stepName, stepIndex, totalSteps }) => {
			logger.info(`Step ${stepIndex + 1}/${totalSteps}: ${stepName}`);
		},
		onStepComplete: (result) => {
			logger.info(`Completed "${result.stepName}" in ${result.durationMs}ms`);
		},
		onChainComplete: (results) => {
			const totalMs = results.reduce((sum, r) => sum + r.durationMs, 0);
			logger.info(`Chain completed in ${totalMs}ms`);
		},
	});

	const results = await chain.run({ topic: 'Bun runtime' });
	for (const step of results) {
		console.log(`\n[${step.stepName}]\n${step.output}`);
	}

	// Run a named chain from config
	logger.info('Running named chain: summarize');
	const summaryResults = await runNamedChain('summarize', config, {
		acpClient: client,
		logger,
		overrideValues: {
			text: 'TypeScript is a strongly typed programming language that builds on JavaScript. It adds optional static typing and class-based OOP. It transpiles to JavaScript and runs anywhere JavaScript does.',
		},
	});
	console.log('\n[summarize]', summaryResults.at(-1)?.output);
}

// ---------------------------------------------------------------------------
// 5. Vector Store — search, topics, deduplication, recommendations
// ---------------------------------------------------------------------------

async function demonstrateVectorStore() {
	logger.info('--- Vector Store Demo ---');

	const store = createVectorStore('.simse-example/vectors', {
		autoSave: true,
		duplicateThreshold: 0.95,
		logger,
	});
	await store.load();

	// Add entries with pre-computed embeddings and topic metadata
	await store.add(
		'TypeScript is a typed superset of JavaScript that compiles to plain JS',
		[0.9, 0.1, 0.0, 0.05],
		{ topic: 'typescript', category: 'language' },
	);
	await store.add(
		'Bun is a fast all-in-one JavaScript runtime with built-in bundler',
		[0.1, 0.9, 0.0, 0.1],
		{ topic: 'bun', category: 'runtime' },
	);
	await store.add(
		'React is a declarative UI library for building user interfaces',
		[0.0, 0.1, 0.9, 0.0],
		{ topic: 'react', category: 'framework' },
	);
	await store.add(
		'Node.js is a JavaScript runtime built on Chrome V8 engine',
		[0.1, 0.85, 0.0, 0.15],
		{ topic: 'nodejs', category: 'runtime' },
	);

	logger.info('Store size', { size: store.size });

	// --- Vector similarity search ---
	const searchResults = store.search([0.85, 0.15, 0.0, 0.05], 2, 0.1);
	logger.info('Vector search results');
	for (const r of searchResults) {
		console.log(`  [${r.score.toFixed(3)}] ${r.entry.text}`);
	}

	// --- Text search (fuzzy) ---
	const textResults = store.textSearch({
		query: 'javascript',
		mode: 'fuzzy',
		maxResults: 3,
	});
	logger.info('Text search results');
	for (const r of textResults) {
		console.log(`  [${r.score.toFixed(3)}] ${r.entry.text}`);
	}

	// --- Metadata filtering ---
	const runtimes = store.filterByMetadata([
		{ key: 'category', value: 'runtime' },
	]);
	logger.info('Entries with category=runtime', { count: runtimes.length });
	for (const entry of runtimes) {
		console.log(`  - ${entry.text}`);
	}

	// --- Topic listing and filtering ---
	const topics = store.getTopics();
	logger.info('Topics in store');
	for (const t of topics) {
		console.log(`  ${t.topic} (${t.entryCount} entries)`);
	}

	const reactEntries = store.filterByTopic(['react']);
	logger.info('Entries matching topic "react"', { count: reactEntries.length });

	// --- Advanced search (vector + text combined) ---
	const advancedResults = store.advancedSearch({
		queryEmbedding: [0.1, 0.9, 0.0, 0.1],
		text: { query: 'runtime', mode: 'substring' },
		maxResults: 3,
		rankBy: 'average',
	});
	logger.info('Advanced search results');
	for (const r of advancedResults) {
		console.log(
			`  [combined=${r.score.toFixed(3)} vec=${r.scores.vector?.toFixed(3)} txt=${r.scores.text?.toFixed(3)}] ${r.entry.text}`,
		);
	}

	// --- Deduplication ---
	await store.add(
		'TypeScript is a typed superset of JavaScript',
		[0.89, 0.11, 0.0, 0.04],
		{ topic: 'typescript' },
	);

	const dupeGroups = store.findDuplicates(0.9);
	logger.info('Duplicate groups found', { count: dupeGroups.length });
	for (const group of dupeGroups) {
		console.log(`  Representative: "${group.representative.text}"`);
		console.log(
			`  Duplicates: ${group.duplicates.length} (avg similarity: ${group.averageSimilarity.toFixed(3)})`,
		);
	}

	// --- Recommendations (vector + recency + frequency) ---
	const recommendations = store.recommend({
		queryEmbedding: [0.1, 0.85, 0.0, 0.1],
		weights: { vector: 0.6, recency: 0.2, frequency: 0.2 },
		maxResults: 3,
	});
	logger.info('Recommendations');
	for (const rec of recommendations) {
		console.log(
			`  [score=${rec.score.toFixed(3)} vec=${rec.scores.vector?.toFixed(3)} recency=${rec.scores.recency?.toFixed(3)}] ${rec.entry.text}`,
		);
	}

	await store.dispose();
}

// ---------------------------------------------------------------------------
// 6. Memory Manager — high-level API with auto-embedding & summarization
// ---------------------------------------------------------------------------

async function demonstrateMemoryManager() {
	logger.info('--- Memory Manager Demo ---');

	// Mock providers for offline demo. In production, use the ACP client.
	const mockEmbedder: EmbeddingProvider = {
		embed: async (input) => {
			const texts = Array.isArray(input) ? input : [input];
			return {
				embeddings: texts.map((t) => {
					// Deterministic mock: hash text into a 4-dim vector
					const h = [...t].reduce((acc, c) => acc + c.charCodeAt(0), 0);
					return [
						Math.sin(h) * 0.5 + 0.5,
						Math.cos(h) * 0.5 + 0.5,
						Math.sin(h * 2) * 0.5 + 0.5,
						Math.cos(h * 2) * 0.5 + 0.5,
					];
				}),
			};
		},
	};

	const mockTextGenerator: TextGenerationProvider = {
		generate: async () => {
			return 'Summary: The entries discuss JavaScript runtimes and tooling.';
		},
	};

	const memory = createMemoryManager(
		mockEmbedder,
		{
			enabled: true,
			storePath: '.simse-example/managed-memory',
			embeddingAgent: 'mock',
			similarityThreshold: 0.1,
			maxResults: 10,
		},
		{ logger, textGenerator: mockTextGenerator },
	);

	await memory.initialize();

	// Add entries — embedding happens automatically
	const id1 = await memory.add('Bun is a fast JavaScript runtime', {
		category: 'runtime',
	});
	const id2 = await memory.add('Deno is a secure JavaScript runtime', {
		category: 'runtime',
	});
	await memory.add('React hooks simplify state management', {
		category: 'framework',
	});

	logger.info('Added entries', { count: memory.size });

	// Semantic search (query is embedded automatically)
	const results = await memory.search('javascript runtime');
	logger.info('Semantic search results', { count: results.length });
	for (const r of results) {
		console.log(`  [${r.score.toFixed(3)}] ${r.entry.text}`);
	}

	// Recommendations (combines vector similarity, recency, access frequency)
	const recs = await memory.recommend('fast runtime', { maxResults: 2 });
	logger.info('Recommendations', { count: recs.length });
	for (const r of recs) {
		console.log(`  [${r.score.toFixed(3)}] ${r.entry.text}`);
	}

	// Summarization (condense multiple entries into one)
	const summary = await memory.summarize({
		ids: [id1, id2],
		deleteOriginals: false,
	});
	logger.info('Summary created', {
		summaryId: summary.summaryId,
		text: summary.summaryText,
	});

	// Duplicate detection
	const dupeCheck = await memory.checkDuplicate(
		'Bun is a fast JavaScript runtime',
	);
	logger.info('Duplicate check', {
		isDuplicate: dupeCheck.isDuplicate,
		similarity: dupeCheck.similarity?.toFixed(3),
	});

	// Topic browsing
	const topics = memory.getTopics();
	logger.info('Topics', {
		topics: topics.map((t) => `${t.topic}(${t.entryCount})`),
	});

	await memory.dispose();
}

// ---------------------------------------------------------------------------
// 7. Retry with exponential backoff
// ---------------------------------------------------------------------------

async function demonstrateRetry() {
	logger.info('--- Retry Demo ---');

	let attempt = 0;
	const result = await retry(
		async () => {
			attempt++;
			if (attempt < 3) {
				throw new Error('Simulated transient failure');
			}
			return `Success on attempt ${attempt}`;
		},
		{
			maxAttempts: 5,
			baseDelayMs: 100,
			shouldRetry: (err) =>
				isTransientError(err) ||
				(err instanceof Error && err.message.includes('transient')),
			onRetry: (_err, nextAttempt, delayMs) => {
				logger.info('Retrying', { nextAttempt, delayMs });
			},
		},
	);
	logger.info('Retry result', { result });
}

// ---------------------------------------------------------------------------
// 8. Error handling
// ---------------------------------------------------------------------------

function demonstrateErrorHandling() {
	logger.info('--- Error Handling Demo ---');

	try {
		defineConfig({
			acp: { servers: [] }, // invalid — needs at least one server
		});
	} catch (err) {
		if (isSimseError(err)) {
			logger.info('Caught SimseError', {
				code: err.code,
				message: err.message,
			});
		}
	}
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
	logger.info('SimSE Example Starting');

	// These work without an ACP server
	demonstrateErrorHandling();
	await demonstrateVectorStore();
	await demonstrateMemoryManager();
	await demonstrateRetry();

	// These require an ACP server
	try {
		const client = await demonstrateACPClient();
		if (client) {
			await demonstrateChains(client);
		}
	} catch (err) {
		if (isSimseError(err)) {
			logger.warn('ACP demos skipped (server not available)', {
				code: err.code,
			});
		} else {
			throw err;
		}
	}

	// Cleanup example data
	const { rm } = await import('node:fs/promises');
	await rm('.simse-example', { recursive: true, force: true });

	logger.info('Example complete!');
}

main().catch((err) => {
	console.error('Fatal error:', err);
	process.exit(1);
});
