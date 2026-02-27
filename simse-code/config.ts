/**
 * SimSE CLI — Configuration
 *
 * Builds the application config from:
 *
 * Global config (in dataDir, default ~/.simse):
 *   config.json  — General user preferences
 *   acp.json     — ACP server entries
 *   mcp.json     — MCP server entries
 *   embed.json   — Embedding provider config (agent, server, model)
 *   memory.json  — Library, stacks & storage config
 *
 * Project config (in .simse/ relative to cwd):
 *   settings.json — Project-specific overrides (agent, log level, system prompt)
 *   prompts.json  — Named prompt templates and chain definitions
 *
 * Precedence: CLI flags > project settings > global config.
 */

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
	type AppConfig,
	createConsoleTransport,
	createLogger,
	defineConfig,
	type Logger,
} from 'simse';
import type { SkillConfig } from './skills.js';

// ---------------------------------------------------------------------------
// Config file types
// ---------------------------------------------------------------------------

export interface ACPServerConfig {
	readonly name: string;
	readonly command: string;
	readonly args?: readonly string[];
	readonly cwd?: string;
	readonly env?: Readonly<Record<string, string>>;
	readonly defaultAgent?: string;
	readonly timeoutMs?: number;
}

export interface ACPFileConfig {
	readonly servers: readonly ACPServerConfig[];
	readonly defaultServer?: string;
	readonly defaultAgent?: string;
}

export interface MCPServerConfig {
	readonly name: string;
	readonly transport: 'stdio';
	readonly command: string;
	readonly args?: readonly string[];
	readonly env?: Readonly<Record<string, string>>;
	/** Env var names this server requires. Server is skipped if any are missing or empty. */
	readonly requiredEnv?: readonly string[];
}

export interface MCPFileConfig {
	readonly servers: readonly MCPServerConfig[];
}

export interface EmbedFileConfig {
	/** Hugging Face model ID for in-process embeddings. */
	readonly embeddingModel?: string;
	/** ONNX quantization dtype (fp32, fp16, q8, q4). */
	readonly dtype?: 'fp32' | 'fp16' | 'q8' | 'q4';
	/** TEI server URL — when set, uses TEI HTTP bridge instead of local embedder. */
	readonly teiUrl?: string;
	/** @deprecated ACP agent ID — use embeddingModel instead. */
	readonly embeddingAgent?: string;
	/** @deprecated ACP server name — use embeddingModel instead. */
	readonly embeddingServer?: string;
}

export interface LibraryFileConfig {
	/** Whether the library is enabled. */
	readonly enabled?: boolean;
	/** Similarity threshold for library search (0–1). */
	readonly similarityThreshold?: number;
	/** Maximum library search results. */
	readonly maxResults?: number;
	/** Storage filename within data directory. */
	readonly storageFilename?: string;
	/** Whether vector store auto-saves on every mutation. */
	readonly autoSave?: boolean;
	/** Cosine similarity threshold for duplicate detection (0–1, 0 = disabled). */
	readonly duplicateThreshold?: number;
	/** Duplicate detection behavior: skip, warn, or error. */
	readonly duplicateBehavior?: 'skip' | 'warn' | 'error';
	/** Auto-flush interval in ms (0 = disabled, only used when autoSave is false). */
	readonly flushIntervalMs?: number;
	/** Gzip compression level for storage (1–9). */
	readonly compressionLevel?: number;
	/** Whether to use atomic writes for storage. */
	readonly atomicWrite?: boolean;
	/** Max notes per topic before auto-summarizing oldest entries (0 = disabled). */
	readonly autoSummarizeThreshold?: number;
}

export interface SummarizeFileConfig {
	/** ACP server name to use for summarization. */
	readonly server: string;
	/** Command to start the summarization ACP server. */
	readonly command: string;
	/** Args for the summarization ACP server command. */
	readonly args?: readonly string[];
	/** Agent ID for the summarization ACP server. */
	readonly agent?: string;
	/** Environment variables for the summarization ACP server. */
	readonly env?: Readonly<Record<string, string>>;
}

export interface UserConfig {
	/** Default agent ID for generation. */
	readonly defaultAgent?: string;
	/** Log level. */
	readonly logLevel?: 'debug' | 'info' | 'warn' | 'error' | 'none';
	/** Perplexity API key. */
	readonly perplexityApiKey?: string;
	/** GitHub token. */
	readonly githubToken?: string;
}

// ---------------------------------------------------------------------------
// Project config file types (.simse/ in cwd)
// ---------------------------------------------------------------------------

/** A single step in a named prompt chain. */
export interface PromptStepConfig {
	/** Step name (used as the output key for subsequent steps). */
	readonly name: string;
	/** Prompt template with {variable} placeholders. */
	readonly template: string;
	/** System prompt prepended to the request. */
	readonly systemPrompt?: string;
	/** ACP agent ID override for this step. */
	readonly agentId?: string;
	/** ACP server name override for this step. */
	readonly serverName?: string;
	/** Map previous step outputs to this step's template variables. */
	readonly inputMapping?: Readonly<Record<string, string>>;
	/** Store this step's output in the library. */
	readonly storeToMemory?: boolean;
	/** Metadata to attach when storing to the library. */
	readonly memoryMetadata?: Readonly<Record<string, string>>;
}

/** A named prompt — a reusable single or multi-step chain. */
export interface PromptConfig {
	/** Human-readable description. */
	readonly description?: string;
	/** ACP agent ID for all steps (overridable per step). */
	readonly agentId?: string;
	/** ACP server name for all steps (overridable per step). */
	readonly serverName?: string;
	/** System prompt applied to all steps. */
	readonly systemPrompt?: string;
	/** Ordered list of chain steps. */
	readonly steps: readonly PromptStepConfig[];
}

/** .simse/prompts.json — named prompt templates for the project. */
export interface PromptsFileConfig {
	readonly [name: string]: PromptConfig;
}

/** .simse/agents/*.md — custom agent persona loaded from markdown with frontmatter. */
export interface AgentConfig {
	/** Unique name (derived from filename if not in frontmatter). */
	readonly name: string;
	/** Human-readable description. */
	readonly description?: string;
	/** Model hint (e.g. 'default', a specific model name). */
	readonly model?: string;
	/** ACP server name override for this agent. */
	readonly serverName?: string;
	/** ACP agent ID override for this agent. */
	readonly agentId?: string;
	/** The markdown body — used as the agent's system prompt. */
	readonly systemPrompt: string;
}

/** .simse/settings.json — workspace-level overrides loaded from cwd. */
export interface WorkspaceSettings {
	/** Default agent ID. */
	readonly defaultAgent?: string;
	/** Log level. */
	readonly logLevel?: 'debug' | 'info' | 'warn' | 'error' | 'none';
	/** System prompt applied to all generate() calls. */
	readonly systemPrompt?: string;
	/** ACP server name override. */
	readonly defaultServer?: string;
	/** Topic name used when storing generate() results in the library. */
	readonly conversationTopic?: string;
	/** Topic name used when storing chain results in the library. */
	readonly chainTopic?: string;
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface CLIConfigOptions {
	/** Global data directory (default: ~/.simse). */
	readonly dataDir?: string;
	/** Working directory to scan for SIMSE.md, .simse/agents/, etc. (default: cwd). */
	readonly workDir?: string;
	/** Default agent ID for generation (overrides all config). */
	readonly defaultAgent?: string;
	/** Log level (overrides all config). */
	readonly logLevel?: 'debug' | 'info' | 'warn' | 'error' | 'none';
	/** MCP client name. */
	readonly clientName?: string;
	/** MCP client version. */
	readonly clientVersion?: string;
	/** MCP server name. */
	readonly serverName?: string;
	/** MCP server version. */
	readonly serverVersion?: string;
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

function readJsonFile<T>(path: string): T | undefined {
	if (!existsSync(path)) return undefined;
	try {
		return JSON.parse(readFileSync(path, 'utf-8')) as T;
	} catch {
		return undefined;
	}
}

function readTextFile(path: string): string | undefined {
	if (!existsSync(path)) return undefined;
	try {
		const content = readFileSync(path, 'utf-8').trim();
		return content || undefined;
	} catch {
		return undefined;
	}
}

/**
 * Parse markdown with YAML frontmatter.
 * Returns the frontmatter fields and the body (everything after the closing `---`).
 */
function parseFrontmatter(content: string): {
	meta: Record<string, string>;
	body: string;
} {
	const meta: Record<string, string> = {};

	if (!content.startsWith('---')) {
		return { meta, body: content.trim() };
	}

	const endIdx = content.indexOf('\n---', 3);
	if (endIdx === -1) {
		return { meta, body: content.trim() };
	}

	const frontmatter = content.slice(4, endIdx).trim();
	const body = content.slice(endIdx + 4).trim();

	// Simple YAML key: value parser (no nested objects)
	for (const line of frontmatter.split('\n')) {
		const colonIdx = line.indexOf(':');
		if (colonIdx === -1) continue;
		const key = line.slice(0, colonIdx).trim();
		const value = line.slice(colonIdx + 1).trim();
		if (key && value) {
			meta[key] = value;
		}
	}

	return { meta, body };
}

function loadAgents(agentsDir: string): readonly AgentConfig[] {
	if (!existsSync(agentsDir)) return [];

	try {
		const files = readdirSync(agentsDir).filter((f) => f.endsWith('.md'));
		const agents: AgentConfig[] = [];

		for (const file of files) {
			const content = readFileSync(join(agentsDir, file), 'utf-8');
			const { meta, body } = parseFrontmatter(content);

			if (!body) continue;

			const name = meta.name || file.replace(/\.md$/, '');
			agents.push(
				Object.freeze({
					name,
					description: meta.description,
					model: meta.model,
					serverName: meta.serverName,
					agentId: meta.agentId,
					systemPrompt: body,
				}),
			);
		}

		return Object.freeze(agents);
	} catch {
		return [];
	}
}

function loadSkills(skillsDir: string): readonly SkillConfig[] {
	if (!existsSync(skillsDir)) return [];

	try {
		const dirs = readdirSync(skillsDir, { withFileTypes: true }).filter((d) =>
			d.isDirectory(),
		);
		const skills: SkillConfig[] = [];

		for (const dir of dirs) {
			const skillPath = join(skillsDir, dir.name, 'SKILL.md');
			if (!existsSync(skillPath)) continue;

			const content = readFileSync(skillPath, 'utf-8');
			const { meta, body } = parseFrontmatter(content);

			if (!body) continue;

			const name = meta.name || dir.name;
			const allowedTools = meta['allowed-tools']
				? meta['allowed-tools']
						.split(',')
						.map((t) => t.trim())
						.filter(Boolean)
				: [];

			skills.push(
				Object.freeze({
					name,
					description: meta.description ?? '',
					allowedTools: Object.freeze(allowedTools),
					argumentHint: meta['argument-hint'] ?? '',
					model: meta.model || undefined,
					serverName: meta['server-name'] || undefined,
					body,
					filePath: skillPath,
				}),
			);
		}

		return Object.freeze(skills);
	} catch {
		return [];
	}
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export interface CLIConfigResult {
	readonly config: AppConfig;
	readonly logger: Logger;
	/** Embedding provider config (from embed.json). */
	readonly embedConfig: EmbedFileConfig;
	/** Resolved library file config (from memory.json). */
	readonly libraryConfig: LibraryFileConfig;
	/** Summarization ACP config (from summarize.json, undefined if not configured). */
	readonly summarizeConfig: SummarizeFileConfig | undefined;
	/** MCP servers that were skipped due to missing API keys / env vars. */
	readonly skippedServers: readonly SkippedServer[];
	/** Workspace settings from .simse/settings.json in cwd. */
	readonly workspaceSettings: WorkspaceSettings;
	/** Named prompts from .simse/prompts.json in cwd. */
	readonly prompts: PromptsFileConfig;
	/** Custom agents from .simse/agents/*.md in cwd. */
	readonly agents: readonly AgentConfig[];
	/** Skills from .simse/skills/{name}/SKILL.md in cwd. */
	readonly skills: readonly SkillConfig[];
	/** SIMSE.md contents from cwd (undefined if not found). */
	readonly workspacePrompt: string | undefined;
}

export interface SkippedServer {
	readonly name: string;
	readonly missingEnv: readonly string[];
}

export function createCLIConfig(options?: CLIConfigOptions): CLIConfigResult {
	const dataDir = options?.dataDir ?? join(process.cwd(), '.simse');

	const configPath = join(dataDir, 'config.json');
	const acpPath = join(dataDir, 'acp.json');
	const mcpPath = join(dataDir, 'mcp.json');
	const embedPath = join(dataDir, 'embed.json');
	const memoryPath = join(dataDir, 'memory.json');

	// -- Load workspace config (cwd .simse/ + SIMSE.md) -----------------------

	const workDir = options?.workDir ?? process.cwd();
	const workSimseDir = join(workDir, '.simse');

	const workspaceSettings =
		readJsonFile<WorkspaceSettings>(join(workSimseDir, 'settings.json')) ?? {};
	const prompts =
		readJsonFile<PromptsFileConfig>(join(workSimseDir, 'prompts.json')) ?? {};

	const simseMdPath = join(workDir, 'SIMSE.md');
	const workspacePrompt = readTextFile(simseMdPath);

	const agents = loadAgents(join(workSimseDir, 'agents'));
	const skills = loadSkills(join(workSimseDir, 'skills'));

	// -- Load config.json (optional) ------------------------------------------

	const userConfig = readJsonFile<UserConfig>(configPath) ?? {};

	// Precedence: CLI flags > workspace settings > global config
	const logLevel =
		options?.logLevel ??
		workspaceSettings.logLevel ??
		userConfig.logLevel ??
		'warn';
	const defaultAgent =
		options?.defaultAgent ??
		workspaceSettings.defaultAgent ??
		userConfig.defaultAgent;

	// -- Load acp.json (required) ---------------------------------------------

	const acpFileConfig = readJsonFile<ACPFileConfig>(acpPath);
	if (!acpFileConfig) {
		throw new Error(
			`No ACP config found at "${acpPath}". Create it with your server definitions or run 'simse init'.`,
		);
	}

	// -- Load mcp.json (optional) ---------------------------------------------

	const mcpFileConfig = readJsonFile<MCPFileConfig>(mcpPath);
	const mcpServers = mcpFileConfig?.servers ?? [];

	// -- Load embed.json (optional) -------------------------------------------

	const embedConfig = readJsonFile<EmbedFileConfig>(embedPath) ?? {};

	// -- Load memory.json (library config, optional) -------------------------

	const libraryConfig = readJsonFile<LibraryFileConfig>(memoryPath) ?? {};

	// -- Load summarize.json (optional) ---------------------------------------

	const summarizePath = join(dataDir, 'summarize.json');
	const summarizeConfig = readJsonFile<SummarizeFileConfig>(summarizePath);

	// -- Detect missing API keys for MCP servers that require them ------------

	const skippedServers: SkippedServer[] = [];
	const validMcpServers: MCPServerConfig[] = [];

	for (const server of mcpServers) {
		if (!server.requiredEnv || server.requiredEnv.length === 0) {
			validMcpServers.push(server);
			continue;
		}

		const missingEnv = server.requiredEnv.filter((key) => {
			const val = server.env?.[key] ?? process.env[key];
			return !val;
		});

		if (missingEnv.length > 0) {
			skippedServers.push({ name: server.name, missingEnv });
		} else {
			validMcpServers.push(server);
		}
	}

	// -- Build final config ---------------------------------------------------

	const config = defineConfig({
		acp: {
			servers: acpFileConfig.servers.map((s) => ({
				...s,
				args: s.args ? [...s.args] : undefined,
			})),
			defaultServer: acpFileConfig.defaultServer,
			defaultAgent: acpFileConfig.defaultAgent ?? defaultAgent,
			// Pass MCP server configs to ACP so agents can discover tools
			mcpServers: validMcpServers.map((s) => ({
				name: s.name,
				command: s.command,
				args: s.args ? [...s.args] : undefined,
				env: s.env,
			})),
		},
		mcp: {
			client: {
				servers: validMcpServers.map((s) => ({
					...s,
					args: s.args ? [...s.args] : undefined,
				})),
				clientName: options?.clientName,
				clientVersion: options?.clientVersion,
			},
			server: {
				name: options?.serverName,
				version: options?.serverVersion,
			},
		},
		memory: {
			enabled: libraryConfig.enabled ?? true,
			embeddingAgent:
				embedConfig.embeddingModel ?? 'nomic-ai/nomic-embed-text-v1.5',
			similarityThreshold: libraryConfig.similarityThreshold ?? 0.7,
			maxResults: libraryConfig.maxResults ?? 10,
		},
		chains: {},
	});

	const logger = createLogger({
		context: 'simse-code',
		level: logLevel,
		transports: [createConsoleTransport()],
	});

	return Object.freeze({
		config,
		logger,
		embedConfig,
		libraryConfig,
		summarizeConfig,
		skippedServers,
		workspaceSettings,
		prompts,
		agents,
		skills,
		workspacePrompt,
	});
}
