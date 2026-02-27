// ---------------------------------------------------------------------------
// Configuration — pure interfaces + functional validation
// ---------------------------------------------------------------------------
//
// The config layer is entirely interface-driven.  No classes, no Zod.
// `defineConfig` is a pure function that validates a typed config object
// and returns a frozen, fully-resolved `AppConfig`.
// ---------------------------------------------------------------------------

import type { ACPConfig, ACPServerEntry } from '../ai/acp/types.js';
import type {
	MCPClientConfig,
	MCPServerConfig,
	MCPServerConnection,
} from '../ai/mcp/types.js';
import type { LibraryConfig } from '../ai/library/types.js';
import { createConfigValidationError } from '../errors/index.js';
import {
	type ACPConfigInput,
	type ACPServerEntryInput,
	type AppConfigInput,
	type ChainDefinitionInput,
	type ChainStepDefinitionInput,
	type MCPConfigInput,
	type MemoryConfigInput,
	type ParallelConfigInput,
	type ParallelSubStepDefinitionInput,
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
	ParallelConfigInput,
	ParallelSubStepDefinitionInput,
	ValidationIssue,
};

// ---------------------------------------------------------------------------
// Resolved config interfaces (output — all defaults applied)
// ---------------------------------------------------------------------------

/** Resolved parallel sub-step definition. */
export interface ParallelSubStepDefinition {
	readonly name: string;
	readonly template: string;
	readonly provider?: 'acp' | 'mcp' | 'memory';
	readonly agentId?: string;
	readonly serverName?: string;
	readonly agentConfig?: Readonly<Record<string, unknown>>;
	readonly systemPrompt?: string;
	readonly mcpServerName?: string;
	readonly mcpToolName?: string;
	readonly mcpArguments?: Readonly<Record<string, string>>;
}

/** Resolved parallel config definition. */
export interface ParallelConfigDefinition {
	readonly subSteps: readonly ParallelSubStepDefinition[];
	readonly mergeStrategy?: 'concat' | 'keyed';
	readonly failTolerant?: boolean;
	readonly concatSeparator?: string;
}

/** Resolved step definition (no class instances or functions). */
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
	readonly parallel?: ParallelConfigDefinition;
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
	readonly memory: Readonly<LibraryConfig>;
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
 *     servers: [{ name: "copilot", command: "copilot", args: ["--acp"] }],
 *     defaultServer: "copilot",
 *     defaultAgent: "default",
 *   },
 *   memory: {
 *     enabled: true,
 *     embeddingAgent: "default",
 *     similarityThreshold: 0.7,
 *     maxResults: 10,
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
 * Convert an input step definition to a resolved step definition.
 */
const resolveParallelSubStep = (
	sub: ParallelSubStepDefinitionInput,
	stepAgentId?: string,
	stepServerName?: string,
): ParallelSubStepDefinition =>
	Object.freeze({
		name: sub.name,
		template: sub.template,
		provider: sub.provider,
		agentId: sub.agentId ?? stepAgentId,
		serverName: sub.serverName ?? stepServerName,
		agentConfig: sub.agentConfig,
		systemPrompt: sub.systemPrompt,
		mcpServerName: sub.mcpServerName,
		mcpToolName: sub.mcpToolName,
		mcpArguments: sub.mcpArguments,
	});

const resolveParallelConfig = (
	config: ParallelConfigInput,
	stepAgentId?: string,
	stepServerName?: string,
): ParallelConfigDefinition =>
	Object.freeze({
		subSteps: Object.freeze(
			config.subSteps.map((sub) =>
				resolveParallelSubStep(sub, stepAgentId, stepServerName),
			),
		),
		mergeStrategy: config.mergeStrategy,
		failTolerant: config.failTolerant,
		concatSeparator: config.concatSeparator,
	});

const resolveStepDefinition = (
	step: ChainStepDefinitionInput,
	chainAgentId?: string,
	chainServerName?: string,
): ChainStepDefinition => {
	const agentId = step.agentId ?? chainAgentId;
	const serverName = step.serverName ?? chainServerName;

	return Object.freeze({
		name: step.name,
		template: step.template,
		provider: step.provider,
		agentId,
		serverName,
		agentConfig: step.agentConfig,
		systemPrompt: step.systemPrompt,
		inputMapping: step.inputMapping,
		mcpServerName: step.mcpServerName,
		mcpToolName: step.mcpToolName,
		mcpArguments: step.mcpArguments,
		storeToMemory: step.storeToMemory,
		memoryMetadata: step.memoryMetadata,
		parallel: step.parallel
			? resolveParallelConfig(step.parallel, agentId, serverName)
			: undefined,
	});
};

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
		command: input.command,
		args: input.args ? Object.freeze([...input.args]) : undefined,
		cwd: input.cwd,
		env: input.env ? Object.freeze({ ...input.env }) : undefined,
		defaultAgent: input.defaultAgent,
		timeoutMs: input.timeoutMs ?? 30_000,
		permissionPolicy: input.permissionPolicy,
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
		env: input.env ? { ...input.env } : undefined,
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
 *     servers: [{ name: "copilot", command: "copilot", args: ["--acp"] }],
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
				input = {
					...input,
					memory: {
						...rawInput.memory,
						...(invalidPaths.has('memory.enabled') && {
							enabled: undefined,
						}),
						...(invalidPaths.has('memory.similarityThreshold') && {
							similarityThreshold: undefined,
						}),
						...(invalidPaths.has('memory.maxResults') && {
							maxResults: undefined,
						}),
						...(invalidPaths.has('memory.embeddingAgent') && {
							embeddingAgent: undefined,
						}),
					},
				};
			}
			if (rawInput.mcp?.server) {
				input = {
					...input,
					mcp: {
						...rawInput.mcp,
						server: {
							...rawInput.mcp.server,
							...(invalidPaths.has('mcp.server.enabled') && {
								enabled: undefined,
							}),
							...(invalidPaths.has('mcp.server.name') && {
								name: undefined,
							}),
							...(invalidPaths.has('mcp.server.version') && {
								version: undefined,
							}),
						},
					},
				};
			}
		} else {
			throw createConfigValidationError(formatted);
		}
	}

	// Guard: lenient mode may suppress errors, but we still need at least one server
	if (input.acp.servers.length === 0) {
		throw createConfigValidationError([
			{
				path: 'acp.servers',
				message: 'ACP servers are required and must be a non-empty array',
			},
		]);
	}

	// Build fully resolved config
	const config: AppConfig = Object.freeze({
		acp: Object.freeze({
			servers: Object.freeze(input.acp.servers.map(resolveACPServerEntry)),
			defaultServer: input.acp.defaultServer,
			defaultAgent: input.acp.defaultAgent,
			mcpServers: input.acp.mcpServers
				? Object.freeze(
						input.acp.mcpServers.map((s) => Object.freeze({ ...s })),
					)
				: undefined,
		}),
		mcp: Object.freeze({
			client: Object.freeze({
				servers: Object.freeze(
					(input.mcp?.client?.servers ?? []).map(resolveMCPServerConnection),
				),
				clientName: input.mcp?.client?.clientName,
				clientVersion: input.mcp?.client?.clientVersion,
			}),
			server: Object.freeze({
				enabled: input.mcp?.server?.enabled ?? false,
				transport: 'stdio' as const,
				name: input.mcp?.server?.name as string,
				version: input.mcp?.server?.version as string,
			}),
		}),
		memory: Object.freeze({
			enabled: input.memory?.enabled ?? false,
			embeddingAgent: input.memory?.embeddingAgent,
			similarityThreshold: input.memory?.similarityThreshold as number,
			maxResults: input.memory?.maxResults as number,
		}),
		chains: resolveChains(input.chains),
	});

	return config;
};
