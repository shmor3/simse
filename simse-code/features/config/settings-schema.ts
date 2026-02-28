/**
 * SimSE Code -- Settings Schema
 *
 * Defines the schema for each config file: field names, types, descriptions,
 * defaults, and allowed values. Used by the settings explorer to know what
 * fields exist and how to edit them.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type FieldType = 'string' | 'number' | 'boolean' | 'enum';

export interface FieldSchema {
	readonly key: string;
	readonly type: FieldType;
	readonly description: string;
	readonly default?: unknown;
	readonly options?: readonly string[]; // for enum type
}

export interface ConfigFileSchema {
	readonly filename: string;
	readonly description: string;
	readonly fields: readonly FieldSchema[];
}

// ---------------------------------------------------------------------------
// Schema definitions
// ---------------------------------------------------------------------------

const configJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'config.json',
	description: 'General user preferences',
	fields: Object.freeze([
		Object.freeze({
			key: 'logLevel',
			type: 'enum' as FieldType,
			description: 'Log level for the application',
			default: 'warn',
			options: Object.freeze(['debug', 'info', 'warn', 'error', 'none']),
		}),
		Object.freeze({
			key: 'defaultAgent',
			type: 'string' as FieldType,
			description: 'Default agent ID for generation',
		}),
		Object.freeze({
			key: 'perplexityApiKey',
			type: 'string' as FieldType,
			description: 'Perplexity API key for web search',
		}),
		Object.freeze({
			key: 'githubToken',
			type: 'string' as FieldType,
			description: 'GitHub personal access token',
		}),
	]),
});

const acpJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'acp.json',
	description: 'ACP server configuration',
	fields: Object.freeze([
		Object.freeze({
			key: 'defaultServer',
			type: 'string' as FieldType,
			description: 'Default ACP server name',
		}),
	]),
});

const embedJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'embed.json',
	description: 'Embedding provider configuration',
	fields: Object.freeze([
		Object.freeze({
			key: 'embeddingModel',
			type: 'string' as FieldType,
			description: 'Hugging Face model ID for in-process embeddings',
			default: 'nomic-ai/nomic-embed-text-v1.5',
		}),
		Object.freeze({
			key: 'dtype',
			type: 'enum' as FieldType,
			description: 'ONNX quantization dtype',
			options: Object.freeze(['fp32', 'fp16', 'q8', 'q4']),
		}),
		Object.freeze({
			key: 'teiUrl',
			type: 'string' as FieldType,
			description:
				'TEI server URL (when set, uses TEI HTTP bridge instead of local embedder)',
		}),
	]),
});

const memoryJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'memory.json',
	description: 'Library, stacks, and storage configuration',
	fields: Object.freeze([
		Object.freeze({
			key: 'enabled',
			type: 'boolean' as FieldType,
			description: 'Whether the library is enabled',
			default: true,
		}),
		Object.freeze({
			key: 'similarityThreshold',
			type: 'number' as FieldType,
			description: 'Similarity threshold for library search (0-1)',
			default: 0.7,
		}),
		Object.freeze({
			key: 'maxResults',
			type: 'number' as FieldType,
			description: 'Maximum library search results',
			default: 10,
		}),
		Object.freeze({
			key: 'autoSummarizeThreshold',
			type: 'number' as FieldType,
			description:
				'Max notes per topic before auto-summarizing oldest entries (0 = disabled)',
			default: 20,
		}),
		Object.freeze({
			key: 'duplicateThreshold',
			type: 'number' as FieldType,
			description:
				'Cosine similarity threshold for duplicate detection (0-1, 0 = disabled)',
			default: 0,
		}),
		Object.freeze({
			key: 'duplicateBehavior',
			type: 'enum' as FieldType,
			description: 'Duplicate detection behavior',
			default: 'skip',
			options: Object.freeze(['skip', 'warn', 'error']),
		}),
	]),
});

const summarizeJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'summarize.json',
	description: 'Summarization ACP server configuration',
	fields: Object.freeze([
		Object.freeze({
			key: 'server',
			type: 'string' as FieldType,
			description: 'ACP server name to use for summarization',
		}),
		Object.freeze({
			key: 'command',
			type: 'string' as FieldType,
			description: 'Command to start the summarization ACP server',
		}),
		Object.freeze({
			key: 'agent',
			type: 'string' as FieldType,
			description: 'Agent ID for the summarization ACP server',
		}),
	]),
});

const settingsJsonSchema: ConfigFileSchema = Object.freeze({
	filename: 'settings.json',
	description: 'Workspace-level overrides (.simse/settings.json)',
	fields: Object.freeze([
		Object.freeze({
			key: 'defaultAgent',
			type: 'string' as FieldType,
			description: 'Default agent ID',
		}),
		Object.freeze({
			key: 'logLevel',
			type: 'enum' as FieldType,
			description: 'Log level',
			options: Object.freeze(['debug', 'info', 'warn', 'error', 'none']),
		}),
		Object.freeze({
			key: 'systemPrompt',
			type: 'string' as FieldType,
			description: 'System prompt applied to all generate() calls',
		}),
		Object.freeze({
			key: 'defaultServer',
			type: 'string' as FieldType,
			description: 'ACP server name override',
		}),
		Object.freeze({
			key: 'conversationTopic',
			type: 'string' as FieldType,
			description:
				'Topic name used when storing generate() results in the library',
		}),
		Object.freeze({
			key: 'chainTopic',
			type: 'string' as FieldType,
			description:
				'Topic name used when storing chain results in the library',
		}),
	]),
});

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

const allSchemas: readonly ConfigFileSchema[] = Object.freeze([
	configJsonSchema,
	acpJsonSchema,
	embedJsonSchema,
	memoryJsonSchema,
	summarizeJsonSchema,
	settingsJsonSchema,
]);

const schemaByFilename = new Map<string, ConfigFileSchema>(
	allSchemas.map((s) => [s.filename, s]),
);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Returns the schema for a known config file, or undefined for unknown files.
 */
export function getConfigSchema(
	filename: string,
): ConfigFileSchema | undefined {
	return schemaByFilename.get(filename);
}

/**
 * Returns all config file schemas.
 */
export function getAllConfigSchemas(): readonly ConfigFileSchema[] {
	return allSchemas;
}

/**
 * Reads a JSON config file and returns its contents.
 * Returns empty object if the file doesn't exist or parse fails.
 */
export function loadConfigFile(
	dataDir: string,
	filename: string,
): Record<string, unknown> {
	const filePath = join(dataDir, filename);
	try {
		if (!existsSync(filePath)) return {};
		const raw = readFileSync(filePath, 'utf-8');
		const parsed = JSON.parse(raw);
		if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
			return parsed as Record<string, unknown>;
		}
		return {};
	} catch {
		return {};
	}
}

/**
 * Reads the config file, updates the field, writes it back.
 * Creates the file if it doesn't exist.
 */
export function saveConfigField(
	dataDir: string,
	filename: string,
	key: string,
	value: unknown,
): void {
	const filePath = join(dataDir, filename);
	const existing = loadConfigFile(dataDir, filename);
	existing[key] = value;
	mkdirSync(dirname(filePath), { recursive: true });
	writeFileSync(filePath, JSON.stringify(existing, null, '\t'), 'utf-8');
}
