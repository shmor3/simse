// ---------------------------------------------------------------------------
// Configuration — pure interfaces + functional validation
// ---------------------------------------------------------------------------
//
// The config layer is entirely interface-driven.  No classes, no Zod.
// `defineConfig` is a pure function that validates a plain JSON object
// and returns a frozen, fully-resolved `AppConfig`.
// ---------------------------------------------------------------------------

import type { ACPConfig, ACPServerEntry } from '../ai/acp/types.js';
import type {
	MCPClientConfig,
	MCPServerConfig,
	MCPServerConnection,
} from '../ai/mcp/types.js';
import type { MemoryConfig } from '../ai/memory/types.js';
import { createConfigValidationError } from '../errors/index.js';
import {
	type ACPConfigInput,
	type ACPServerEntryInput,
	type AppConfigInput,
	type ChainDefinitionInput,
	type ChainStepDefinitionInput,
	type MCPConfigInput,
	type MemoryConfigInput,
	type ValidationIssue,
	validateAppConfig,
} from './schema.js';

// ---------------------------------------------------------------------------
// Re-export input interfaces for convenience
// ---------------------------------------------------------------------------

export type {
	ACPConfigInput,
	ACPServerEntryInput as ACPServerInput,
	AppConfigInput,
	ChainDefinitionInput,
	ChainStepDefinitionInput,
	MCPConfigInput,
	MemoryConfigInput,
	ValidationIssue,
};

// ---------------------------------------------------------------------------
// Resolved config interfaces (output — all defaults applied)
// ---------------------------------------------------------------------------

/** JSON-serialisable step definition (no class instances or functions). */
export interface ChainStepDefinition {
	readonly name: string;
	readonly template: string;
	readonly provider?: 'acp' | 'mcp' | 'memory';
	readonly agentId?: string;
	readonly serverName?: string;
	readonly agentConfig?: Readonly<Record<string, unknown>>;
	readonly systemPrompt?: string;
	readonly inputMapping?: Readonly<Record<string, string>>;
	readonly mcpServerName?: string;
	readonly mcpToolName?: string;
	readonly mcpArguments?: Readonly<Record<string, string>>;
	readonly storeToMemory?: boolean;
	readonly memoryMetadata?: Readonly<Record<string, string>>;
}

export interface ChainDefinition {
	readonly description?: string;
	readonly agentId?: string;
	readonly serverName?: string;
	readonly initialValues: Readonly<Record<string, string>>;
	readonly steps: readonly ChainStepDefinition[];
}

export type { MCPClientConfig, MCPServerConfig, ACPConfig };

export interface AppConfig {
	readonly acp: Readonly<ACPConfig>;
	readonly mcp: {
		readonly client: Readonly<MCPClientConfig>;
		readonly server: Readonly<MCPServerConfig>;
	};
	readonly memory: Readonly<MemoryConfig>;
	readonly chains: Readonly<Record<string, ChainDefinition>>;
}

// ---------------------------------------------------------------------------
// Top-level input interface — what the user passes to defineConfig()
// ---------------------------------------------------------------------------

/**
 * Top-level configuration input interface.
 *
 * The `acp` field (with at least one server) is required.
 * All other sections are optional and will receive sensible defaults.
 *
 * @example
 * ```ts
 * import { defineConfig } from "simse";
 *
 * const config = defineConfig({
 *   acp: {
 *     servers: [{ name: "local", url: "http://localhost:8000" }],
 *     defaultServer: "local",
 *     defaultAgent: "default",
 *   },
 *   memory: {
 *     storePath: ".my-app/memory",
 *   },
 *   chains: {
 *     summarize: {
 *       description: "Summarize a document",
 *       steps: [
 *         { name: "summarize", template: "Summarize:\n\n{text}" },
 *       ],
 *     },
 *   },
 * });
 * ```
 */
export interface SimseConfig extends AppConfigInput {}

// ---------------------------------------------------------------------------
// Options for defineConfig
// ---------------------------------------------------------------------------

export interface DefineConfigOptions {
	/**
	 * If `true`, validation errors are logged as warnings and defaults are used
	 * for the invalid fields instead of throwing. Defaults to `false`.
	 */
	readonly lenient?: boolean;
	/**
	 * Optional warning handler called in lenient mode.
	 * Receives the list of validation issues.
	 * If not provided, warnings are silently ignored.
	 */
	readonly onWarn?: (
		issues: ReadonlyArray<{ readonly path: string; readonly message: string }>,
	) => void;
}

// ---------------------------------------------------------------------------
// Pure helper functions
// ---------------------------------------------------------------------------

/**
 * Resolve API keys from environment variables for ACP servers.
 *
 * Looks for `ACP_API_KEY_<UPPER_NAME>` where UPPER_NAME is the server name
 * uppercased with hyphens replaced by underscores.
 */
const resolveACPApiKeys = (
	servers: readonly ACPServerEntryInput[],
): readonly ACPServerEntryInput[] =>
	servers.map((server): ACPServerEntryInput => {
		if (server.apiKey) return server;

		if (server.name) {
			const envKey = `ACP_API_KEY_${server.name.toUpperCase().replace(/-/g, '_')}`;
			const envValue = process.env[envKey];
			if (envValue) {
				return { ...server, apiKey: envValue };
			}
		}

		return server;
	});

/**
 * Convert an input step definition to a resolved step definition.
 */
const resolveStepDefinition = (
	step: ChainStepDefinitionInput,
	chainAgentId?: string,
	chainServerName?: string,
): ChainStepDefinition =>
	Object.freeze({
		name: step.name,
		template: step.template,
		provider: step.provider,
		agentId: step.agentId ?? chainAgentId,
		serverName: step.serverName ?? chainServerName,
		agentConfig: step.agentConfig,
		systemPrompt: step.systemPrompt,
		inputMapping: step.inputMapping,
		mcpServerName: step.mcpServerName,
		mcpToolName: step.mcpToolName,
		mcpArguments: step.mcpArguments,
		storeToMemory: step.storeToMemory,
		memoryMetadata: step.memoryMetadata,
	});

/**
 * Convert an input chain definition to a resolved chain definition.
 */
const resolveChainDefinition = (chain: ChainDefinitionInput): ChainDefinition =>
	Object.freeze({
		description: chain.description,
		agentId: chain.agentId,
		serverName: chain.serverName,
		initialValues: Object.freeze({ ...(chain.initialValues ?? {}) }),
		steps: Object.freeze(
			chain.steps.map((step) =>
				resolveStepDefinition(step, chain.agentId, chain.serverName),
			),
		),
	});

/**
 * Resolve chains separately so we can give precise errors per chain.
 */
const resolveChains = (
	rawChains: Readonly<Record<string, ChainDefinitionInput>> | undefined,
): Readonly<Record<string, ChainDefinition>> => {
	if (!rawChains || Object.keys(rawChains).length === 0)
		return Object.freeze({});

	const result: Record<string, ChainDefinition> = {};
	for (const [chainName, chainDef] of Object.entries(rawChains)) {
		result[chainName] = resolveChainDefinition(chainDef);
	}
	return Object.freeze(result);
};

/**
 * Build a resolved ACP server entry with defaults applied.
 */
const resolveACPServerEntry = (input: ACPServerEntryInput): ACPServerEntry =>
	Object.freeze({
		name: input.name,
		url: input.url,
		defaultAgent: input.defaultAgent,
		apiKey: input.apiKey,
		timeoutMs: input.timeoutMs ?? 30_000,
	});

/**
 * Build a resolved MCP server connection from input.
 */
const resolveMCPServerConnection = (
	input: MCPServerConnection,
): MCPServerConnection =>
	Object.freeze({
		name: input.name,
		transport: input.transport,
		command: input.command,
		args: input.args ? [...input.args] : undefined,
		url: input.url,
	});

// ---------------------------------------------------------------------------
// defineConfig
// ---------------------------------------------------------------------------

/**
 * Create a validated `AppConfig` from a user-supplied configuration object.
 *
 * Applies sensible defaults for all optional fields and validates the
 * configuration against TypeScript validation functions.
 *
 * @param input  - The user-supplied configuration.
 * @param options - Optional settings controlling validation behaviour.
 * @returns A fully resolved, frozen, and validated `AppConfig`.
 *
 * @throws {ConfigValidationError} when the input fails schema validation
 *         (unless `options.lenient` is `true`).
 *
 * @example
 * ```ts
 * const config = defineConfig({
 *   acp: {
 *     servers: [{ name: "local", url: "http://localhost:8000" }],
 *   },
 * });
 * ```
 */
export const defineConfig = (
	rawInput: SimseConfig,
	options?: DefineConfigOptions,
): AppConfig => {
	const lenient = options?.lenient ?? false;

	// Validate input
	const issues = validateAppConfig(rawInput);

	// Work on a shallow copy so we never mutate the caller's object
	let input = rawInput;

	if (issues.length > 0) {
		const formatted = issues.map((i) => ({ path: i.path, message: i.message }));
		if (lenient) {
			options?.onWarn?.(formatted);

			// In lenient mode, reset invalid fields to undefined so that the
			// resolution defaults (via ??) take effect instead of using the
			// invalid values verbatim.  We shallow-clone affected sub-objects
			// to avoid mutating the caller's original input.
			const invalidPaths = new Set(issues.map((i) => i.path));
			input = { ...rawInput };
			if (rawInput.memory) {
				const mem = { ...rawInput.memory } as Record<string, unknown>;
				if (invalidPaths.has('memory.enabled')) mem.enabled = undefined;
				if (invalidPaths.has('memory.storePath')) mem.storePath = undefined;
				if (invalidPaths.has('memory.similarityThreshold'))
					mem.similarityThreshold = undefined;
				if (invalidPaths.has('memory.maxResults')) mem.maxResults = undefined;
				if (invalidPaths.has('memory.embeddingAgent'))
					mem.embeddingAgent = undefined;
				input = { ...input, memory: mem as typeof rawInput.memory };
			}
			if (rawInput.mcp?.server) {
				const srv = { ...rawInput.mcp.server } as Record<string, unknown>;
				if (invalidPaths.has('mcp.server.enabled')) srv.enabled = undefined;
				if (invalidPaths.has('mcp.server.name')) srv.name = undefined;
				if (invalidPaths.has('mcp.server.version')) srv.version = undefined;
				input = {
					...input,
					mcp: { ...rawInput.mcp, server: srv as typeof rawInput.mcp.server },
				};
			}
		} else {
			throw createConfigValidationError(formatted);
		}
	}

	// Guard against structurally invalid input (lenient mode may suppress errors)
	if (
		!input.acp ||
		!Array.isArray(input.acp.servers) ||
		input.acp.servers.length === 0
	) {
		throw createConfigValidationError([
			{
				path: 'acp.servers',
				message: 'ACP servers are required and must be a non-empty array',
			},
		]);
	}

	// Resolve ACP API keys from environment
	const acpServers = resolveACPApiKeys(input.acp.servers);

	// Build fully resolved config
	const config: AppConfig = Object.freeze({
		acp: Object.freeze({
			servers: Object.freeze(acpServers.map(resolveACPServerEntry)),
			defaultServer: input.acp.defaultServer,
			defaultAgent: input.acp.defaultAgent,
		}),
		mcp: Object.freeze({
			client: Object.freeze({
				servers: Object.freeze(
					(input.mcp?.client?.servers ?? []).map(resolveMCPServerConnection),
				),
			}),
			server: Object.freeze({
				enabled: input.mcp?.server?.enabled ?? false,
				transport: 'stdio' as const,
				name: input.mcp?.server?.name ?? 'simse',
				version: input.mcp?.server?.version ?? '1.0.0',
			}),
		}),
		memory: Object.freeze({
			enabled: input.memory?.enabled ?? false,
			embeddingAgent: input.memory?.embeddingAgent,
			storePath: input.memory?.storePath ?? '.simse/memory',
			similarityThreshold: input.memory?.similarityThreshold ?? 0.7,
			maxResults: input.memory?.maxResults ?? 5,
		}),
		chains: resolveChains(input.chains),
	});

	return config;
};
