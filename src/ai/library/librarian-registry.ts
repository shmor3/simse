// ---------------------------------------------------------------------------
// Librarian Registry — manages multiple librarians with bidding, arbitration,
// and specialist spawning.
// ---------------------------------------------------------------------------

import { unlink } from 'node:fs/promises';
import { join } from 'node:path';
import { toError } from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { ACPConnection } from '../acp/acp-connection.js';
import {
	loadAllDefinitions,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from './librarian-definition.js';
import { createLibrarian } from './librarian.js';
import type {
	ArbitrationResult,
	Librarian,
	LibrarianBid,
	LibrarianDefinition,
	LibrarianLibraryAccess,
	TextGenerationProvider,
	Volume,
} from './types.js';

// ---------------------------------------------------------------------------
// Exported interfaces
// ---------------------------------------------------------------------------

export interface ManagedLibrarian {
	readonly definition: LibrarianDefinition;
	readonly librarian: Librarian;
	readonly provider: TextGenerationProvider;
	readonly connection?: ACPConnection;
}

export interface LibrarianRegistry {
	readonly initialize: () => Promise<void>;
	readonly dispose: () => Promise<void>;
	readonly register: (definition: LibrarianDefinition) => Promise<ManagedLibrarian>;
	readonly unregister: (name: string) => Promise<void>;
	readonly get: (name: string) => ManagedLibrarian | undefined;
	readonly list: () => readonly ManagedLibrarian[];
	readonly defaultLibrarian: ManagedLibrarian;
	readonly resolveLibrarian: (topic: string, content: string) => Promise<ArbitrationResult>;
	readonly spawnSpecialist: (topic: string, volumes: readonly Volume[]) => Promise<ManagedLibrarian>;
}

export interface LibrarianRegistryOptions {
	readonly librariansDir: string;
	readonly library: LibrarianLibraryAccess;
	readonly defaultProvider: TextGenerationProvider;
	readonly logger?: Logger;
	readonly selfResolutionGap?: number;
	readonly createConnection?: (def: LibrarianDefinition) => Promise<{
		connection: ACPConnection;
		provider: TextGenerationProvider;
	}>;
}

// ---------------------------------------------------------------------------
// Default librarian definition (created programmatically, never on disk)
// ---------------------------------------------------------------------------

const DEFAULT_DEFINITION: LibrarianDefinition = Object.freeze({
	name: 'default',
	description: 'General-purpose head librarian that manages all topics.',
	purpose: 'General-purpose head librarian for routing, arbitration, and fallback.',
	topics: ['*'],
	permissions: { add: true, delete: true, reorganize: true },
	thresholds: { topicComplexity: 100, escalateAt: 500 },
});

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createLibrarianRegistry(
	options: LibrarianRegistryOptions,
): LibrarianRegistry {
	const {
		librariansDir,
		library,
		defaultProvider,
		createConnection,
	} = options;
	const selfResolutionGap = options.selfResolutionGap ?? 0.3;
	const logger = (options.logger ?? getDefaultLogger()).child('librarian-registry');

	const librarians = new Map<string, ManagedLibrarian>();
	let initialized = false;
	let initPromise: Promise<void> | null = null;

	// -------------------------------------------------------------------
	// Helpers
	// -------------------------------------------------------------------

	const buildManaged = async (
		definition: LibrarianDefinition,
	): Promise<ManagedLibrarian> => {
		if (definition.acp && createConnection) {
			const { connection, provider } = await createConnection(definition);
			const librarian = createLibrarian(provider, {
				name: definition.name,
				purpose: definition.purpose,
			});
			return Object.freeze({ definition, librarian, provider, connection });
		}

		const librarian = createLibrarian(defaultProvider, {
			name: definition.name,
			purpose: definition.purpose,
		});
		return Object.freeze({ definition, librarian, provider: defaultProvider });
	};

	// -------------------------------------------------------------------
	// initialize
	// -------------------------------------------------------------------

	const initialize = (): Promise<void> => {
		if (initialized) return Promise.resolve();
		if (initPromise) return initPromise;

		initPromise = (async () => {
			logger.info('Initializing librarian registry', { librariansDir });

			// Always create the default librarian first
			const defaultManaged = await buildManaged(DEFAULT_DEFINITION);
			librarians.set('default', defaultManaged);

			// Load definitions from disk
			const definitions = await loadAllDefinitions(librariansDir);
			for (const def of definitions) {
				if (def.name === 'default') continue; // skip if someone wrote a default.json
				try {
					const managed = await buildManaged(def);
					librarians.set(def.name, managed);
					logger.debug('Loaded librarian', { name: def.name });
				} catch (err) {
					logger.warn(`Failed to load librarian "${def.name}"`, {
						error: toError(err).message,
					});
				}
			}

			initialized = true;
			logger.info(`Librarian registry initialized (${librarians.size} librarians)`);
		})().finally(() => {
			initPromise = null;
		});

		return initPromise;
	};

	// -------------------------------------------------------------------
	// register
	// -------------------------------------------------------------------

	const register = async (
		definition: LibrarianDefinition,
	): Promise<ManagedLibrarian> => {
		const result = validateDefinition(definition);
		if (!result.valid) {
			throw new Error(
				`Invalid librarian definition: ${result.errors.join(', ')}`,
			);
		}

		if (librarians.has(definition.name)) {
			throw new Error(
				`Librarian "${definition.name}" is already registered`,
			);
		}

		const managed = await buildManaged(definition);
		librarians.set(definition.name, managed);

		// Persist to disk
		await saveDefinition(librariansDir, definition);
		logger.info('Registered librarian', { name: definition.name });

		return managed;
	};

	// -------------------------------------------------------------------
	// unregister
	// -------------------------------------------------------------------

	const unregister = async (name: string): Promise<void> => {
		if (name === 'default') {
			throw new Error('Cannot unregister the default librarian');
		}

		const managed = librarians.get(name);
		if (managed?.connection) {
			await managed.connection.close();
		}

		librarians.delete(name);

		// Delete file from disk (ignore if missing)
		try {
			await unlink(join(librariansDir, `${name}.json`));
		} catch {
			// File may not exist — that's fine
		}

		logger.info('Unregistered librarian', { name });
	};

	// -------------------------------------------------------------------
	// resolveLibrarian
	// -------------------------------------------------------------------

	const resolveLibrarian = async (
		topic: string,
		content: string,
	): Promise<ArbitrationResult> => {
		// Find candidates whose topics match
		const candidates: ManagedLibrarian[] = [];
		for (const managed of librarians.values()) {
			if (matchesTopic(managed.definition.topics, topic)) {
				candidates.push(managed);
			}
		}

		// 0 matches → default
		if (candidates.length === 0) {
			return Object.freeze({
				winner: 'default',
				reason: 'No specialist matched the topic; using default librarian.',
				bids: [],
			});
		}

		// 1 match → return directly without bidding
		if (candidates.length === 1) {
			return Object.freeze({
				winner: candidates[0].definition.name,
				reason: 'Only one librarian matched the topic.',
				bids: [],
			});
		}

		// Multiple matches → collect bids
		const bids: LibrarianBid[] = await Promise.all(
			candidates.map((c) => c.librarian.bid(content, topic, library)),
		);

		// Sort by confidence descending
		const sorted = [...bids].sort((a, b) => b.confidence - a.confidence);

		// Self-resolution: if gap between top two is large enough
		if (sorted.length >= 2) {
			const gap = sorted[0].confidence - sorted[1].confidence;
			if (gap > selfResolutionGap) {
				return Object.freeze({
					winner: sorted[0].librarianName,
					reason: `Self-resolved: confidence gap ${gap.toFixed(2)} exceeds threshold ${selfResolutionGap}.`,
					bids: sorted,
				});
			}
		}

		// Arbitration by default librarian's provider
		try {
			const candidateNames = sorted.map((b) => b.librarianName);
			const bidsDescription = sorted
				.map(
					(b) =>
						`- ${b.librarianName} (confidence: ${b.confidence.toFixed(2)}): ${b.argument}`,
				)
				.join('\n');

			const prompt = `You are arbitrating between librarians to decide who should manage new content.

Topic: ${topic}
Content preview: ${content.slice(0, 500)}

Bids:
${bidsDescription}

Choose the best librarian. Return ONLY valid JSON:
{"winner": "librarian-name", "reason": "brief explanation"}`;

			const response = await defaultProvider.generate(prompt);
			const parsed = JSON.parse(response);

			if (
				typeof parsed.winner === 'string' &&
				candidateNames.includes(parsed.winner)
			) {
				return Object.freeze({
					winner: parsed.winner,
					reason: typeof parsed.reason === 'string'
						? parsed.reason
						: 'Arbitration by default librarian.',
					bids: sorted,
				});
			}

			// Winner not in candidates — fall back to highest bidder
			logger.warn('Arbitration returned unknown winner; falling back to highest bidder', {
				parsedWinner: parsed.winner,
			});
		} catch (err) {
			logger.warn('Arbitration failed; falling back to highest bidder', {
				error: toError(err).message,
			});
		}

		// Fallback: highest bidder wins
		return Object.freeze({
			winner: sorted[0].librarianName,
			reason: 'Highest bidder wins (arbitration fallback).',
			bids: sorted,
		});
	};

	// -------------------------------------------------------------------
	// spawnSpecialist
	// -------------------------------------------------------------------

	const spawnSpecialist = async (
		topic: string,
		volumes: readonly Volume[],
	): Promise<ManagedLibrarian> => {
		// Step 1: Ask provider if specialist is needed
		const assessPrompt = `Should a specialist librarian be created for the topic "${topic}"?
There are currently ${volumes.length} volumes in this topic.

Volume samples:
${volumes.slice(0, 5).map((v) => `- ${v.text.slice(0, 100)}`).join('\n')}

Return ONLY valid JSON: {"shouldSpawn": true/false, "reason": "brief explanation"}`;

		const assessResponse = await defaultProvider.generate(assessPrompt);
		const assessParsed = JSON.parse(assessResponse);

		if (!assessParsed.shouldSpawn) {
			throw new Error(
				`Specialist not needed for topic "${topic}": ${assessParsed.reason ?? 'provider declined'}`,
			);
		}

		// Step 2: Generate a LibrarianDefinition
		const generatePrompt = `Generate a librarian definition JSON for a specialist that will manage the topic "${topic}".

The librarian should:
- Have a descriptive kebab-case name related to the topic
- Cover the topic and its subtopics
- Have appropriate permissions

Return ONLY valid JSON matching this schema:
{
  "name": "kebab-case-name",
  "description": "what this librarian does",
  "purpose": "detailed purpose statement",
  "topics": ["${topic}", "${topic}/**"],
  "permissions": { "add": true, "delete": true, "reorganize": true },
  "thresholds": { "topicComplexity": 50, "escalateAt": 100 }
}`;

		const genResponse = await defaultProvider.generate(generatePrompt);
		const genParsed = JSON.parse(genResponse) as LibrarianDefinition;

		// Validate the generated definition
		const validation = validateDefinition(genParsed);
		if (!validation.valid) {
			throw new Error(
				`Generated definition is invalid: ${validation.errors.join(', ')}`,
			);
		}

		// Step 3: Register
		const managed = await register(genParsed);
		logger.info('Spawned specialist librarian', {
			name: genParsed.name,
			topic,
		});

		return managed;
	};

	// -------------------------------------------------------------------
	// dispose
	// -------------------------------------------------------------------

	const dispose = async (): Promise<void> => {
		for (const managed of librarians.values()) {
			if (managed.connection) {
				try {
					await managed.connection.close();
				} catch {
					// Swallow close errors during teardown
				}
			}
		}
		librarians.clear();
		initialized = false;
		logger.info('Librarian registry disposed');
	};

	// -------------------------------------------------------------------
	// Return frozen interface
	// -------------------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		register,
		unregister,
		get: (name: string) => librarians.get(name),
		list: () => [...librarians.values()],
		get defaultLibrarian() {
			const d = librarians.get('default');
			if (!d) {
				throw new Error('Registry not initialized: default librarian not available');
			}
			return d;
		},
		resolveLibrarian,
		spawnSpecialist,
	});
}
