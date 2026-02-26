// ---------------------------------------------------------------------------
// Pure TypeScript Configuration Validation
// ---------------------------------------------------------------------------
//
// All validators accept typed interfaces — TypeScript handles structural
// validation at compile time. These validators only check semantic
// constraints: URL format, numeric ranges, cross-references, conditional
// requirements, duplicate names, regex patterns, and semver format.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Validation primitives
// ---------------------------------------------------------------------------

export interface ValidationIssue {
	readonly path: string;
	readonly message: string;
}

const issue = (path: string, message: string): readonly ValidationIssue[] =>
	Object.freeze([Object.freeze({ path, message })]);

const ok: readonly ValidationIssue[] = Object.freeze([]);

const combine = (
	...results: ReadonlyArray<readonly ValidationIssue[]>
): readonly ValidationIssue[] => Object.freeze(results.flat());

// ---------------------------------------------------------------------------
// Reusable semantic validators
// ---------------------------------------------------------------------------

const validateNonEmpty = (
	value: string,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	if (value.length === 0) return issue(path, `${label} cannot be empty`);
	return ok;
};

const validateUrl = (
	value: string,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	try {
		new URL(value);
		return ok;
	} catch {
		return issue(path, `${label} must be a valid URL`);
	}
};

const validateRange = (
	value: number,
	path: string,
	label: string,
	constraints: {
		readonly min?: number;
		readonly max?: number;
		readonly integer?: boolean;
	},
): readonly ValidationIssue[] => {
	if (Number.isNaN(value)) return issue(path, `${label} must be a number`);
	if (constraints.integer && !Number.isInteger(value))
		return issue(path, `${label} must be an integer`);
	if (constraints.min !== undefined && value < constraints.min)
		return issue(path, `${label} must be at least ${constraints.min}`);
	if (constraints.max !== undefined && value > constraints.max)
		return issue(path, `${label} must be at most ${constraints.max}`);
	return ok;
};

// ---------------------------------------------------------------------------
// ACP Server Entry
// ---------------------------------------------------------------------------

export interface ACPServerEntryInput {
	readonly name: string;
	/** Command to spawn the ACP server (required — ACP uses stdio). */
	readonly command: string;
	readonly args?: readonly string[];
	readonly cwd?: string;
	readonly env?: Readonly<Record<string, string>>;
	readonly defaultAgent?: string;
	readonly timeoutMs?: number;
	readonly permissionPolicy?: 'auto-approve' | 'prompt' | 'deny';
}

export const validateACPServerEntry = (
	value: ACPServerEntryInput,
	path: string,
): readonly ValidationIssue[] =>
	combine(
		validateNonEmpty(value.name, `${path}.name`, 'ACP server name'),
		validateNonEmpty(value.command, `${path}.command`, 'ACP server command'),
		value.defaultAgent !== undefined
			? validateNonEmpty(
					value.defaultAgent,
					`${path}.defaultAgent`,
					'Default agent ID',
				)
			: ok,
		value.timeoutMs !== undefined
			? validateRange(value.timeoutMs, `${path}.timeoutMs`, 'timeoutMs', {
					min: 1000,
					max: 600_000,
					integer: true,
				})
			: ok,
	);

// ---------------------------------------------------------------------------
// ACP Config
// ---------------------------------------------------------------------------

export interface ACPConfigInput {
	readonly servers: readonly ACPServerEntryInput[];
	readonly defaultServer?: string;
	readonly defaultAgent?: string;
	/** MCP server configs to pass to ACP agents during session creation. */
	readonly mcpServers?: readonly import('../ai/acp/types.js').ACPMCPServerConfig[];
}

export const validateACPConfig = (
	value: ACPConfigInput,
	path: string,
): readonly ValidationIssue[] => {
	if (value.servers.length === 0)
		return issue(
			`${path}.servers`,
			'At least one ACP server must be configured',
		);

	const issues: ValidationIssue[] = [];

	for (let i = 0; i < value.servers.length; i++) {
		issues.push(
			...validateACPServerEntry(value.servers[i], `${path}.servers[${i}]`),
		);
	}

	// Detect duplicate server names
	if (issues.length === 0) {
		const seen = new Set<string>();
		for (const server of value.servers) {
			if (seen.has(server.name)) {
				issues.push(
					...issue(
						`${path}.servers`,
						`Duplicate ACP server name: "${server.name}"`,
					),
				);
			}
			seen.add(server.name);
		}
	}

	if (value.defaultServer !== undefined) {
		issues.push(
			...validateNonEmpty(
				value.defaultServer,
				`${path}.defaultServer`,
				'Default server name',
			),
		);
	}

	if (value.defaultAgent !== undefined) {
		issues.push(
			...validateNonEmpty(
				value.defaultAgent,
				`${path}.defaultAgent`,
				'Default agent ID',
			),
		);
	}

	// Cross-validate: defaultServer must reference a configured server name
	if (value.defaultServer !== undefined && value.defaultServer.length > 0) {
		const serverNames = new Set(value.servers.map((s) => s.name));
		if (!serverNames.has(value.defaultServer)) {
			issues.push(
				...issue(
					`${path}.defaultServer`,
					`Default server "${value.defaultServer}" is not defined in servers (available: ${[...serverNames].join(', ')})`,
				),
			);
		}
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// MCP Server Connection
// ---------------------------------------------------------------------------

export interface MCPStdioConnectionInput {
	readonly name: string;
	readonly transport: 'stdio';
	readonly command: string;
	readonly args?: readonly string[];
	readonly env?: Readonly<Record<string, string>>;
	readonly url?: string;
}

export interface MCPHttpConnectionInput {
	readonly name: string;
	readonly transport: 'http';
	readonly url: string;
	readonly command?: string;
	readonly args?: readonly string[];
}

export type MCPServerConnectionInput =
	| MCPStdioConnectionInput
	| MCPHttpConnectionInput;

export const validateMCPServerConnection = (
	value: MCPServerConnectionInput,
	path: string,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];

	issues.push(
		...validateNonEmpty(value.name, `${path}.name`, 'MCP server name'),
	);

	if (value.transport === 'stdio') {
		issues.push(
			...validateNonEmpty(value.command, `${path}.command`, 'stdio command'),
		);
	}

	if (value.transport === 'http') {
		issues.push(...validateUrl(value.url, `${path}.url`, 'http URL'));
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// MCP Client Config
// ---------------------------------------------------------------------------

export interface MCPClientConfigInput {
	readonly servers?: readonly MCPServerConnectionInput[];
	/** Client name advertised during MCP handshake. */
	readonly clientName?: string;
	/** Client version advertised during MCP handshake. */
	readonly clientVersion?: string;
}

export const validateMCPClientConfig = (
	value: MCPClientConfigInput,
	path: string,
): readonly ValidationIssue[] => {
	if (value.servers === undefined || value.servers.length === 0) return ok;

	const issues: ValidationIssue[] = [];
	for (let i = 0; i < value.servers.length; i++) {
		issues.push(
			...validateMCPServerConnection(value.servers[i], `${path}.servers[${i}]`),
		);
	}

	// Detect duplicate server names
	if (issues.length === 0) {
		const seen = new Set<string>();
		for (const server of value.servers) {
			if (seen.has(server.name)) {
				issues.push(
					...issue(
						`${path}.servers`,
						`Duplicate MCP server name: "${server.name}"`,
					),
				);
			}
			seen.add(server.name);
		}
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// MCP Server Config (built-in server mode)
// ---------------------------------------------------------------------------

export interface MCPServerConfigInput {
	readonly enabled?: boolean;
	readonly transport?: 'stdio';
	readonly name?: string;
	readonly version?: string;
}

const SEMVER_RE = /^\d+\.\d+\.\d+$/;

export const validateMCPServerConfig = (
	value: MCPServerConfigInput,
	path: string,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];
	const enabled = value.enabled === true;

	if (enabled && value.name === undefined) {
		issues.push(
			...issue(`${path}.name`, 'MCP server name is required when enabled'),
		);
	}

	if (value.name !== undefined) {
		issues.push(
			...validateNonEmpty(value.name, `${path}.name`, 'MCP server name'),
		);
	}

	if (enabled && value.version === undefined) {
		issues.push(
			...issue(
				`${path}.version`,
				'MCP server version is required when enabled',
			),
		);
	}

	if (value.version !== undefined && !SEMVER_RE.test(value.version)) {
		issues.push(
			...issue(
				`${path}.version`,
				'MCP server version must be semver (e.g. 1.0.0)',
			),
		);
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// MCP Combined Config
// ---------------------------------------------------------------------------

export interface MCPConfigInput {
	readonly client?: MCPClientConfigInput;
	readonly server?: MCPServerConfigInput;
}

export const validateMCPConfig = (
	value: MCPConfigInput,
	path: string,
): readonly ValidationIssue[] =>
	combine(
		value.client !== undefined
			? validateMCPClientConfig(value.client, `${path}.client`)
			: ok,
		value.server !== undefined
			? validateMCPServerConfig(value.server, `${path}.server`)
			: ok,
	);

// ---------------------------------------------------------------------------
// Memory Config
// ---------------------------------------------------------------------------

export interface MemoryConfigInput {
	readonly enabled?: boolean;
	readonly embeddingAgent?: string;
	readonly similarityThreshold?: number;
	readonly maxResults?: number;
}

export const validateMemoryConfig = (
	value: MemoryConfigInput,
	path: string,
): readonly ValidationIssue[] => {
	const enabled = value.enabled !== false;

	return combine(
		enabled && value.embeddingAgent === undefined
			? issue(
					`${path}.embeddingAgent`,
					'Embedding agent ID is required when memory is enabled',
				)
			: ok,
		value.embeddingAgent !== undefined
			? validateNonEmpty(
					value.embeddingAgent,
					`${path}.embeddingAgent`,
					'Embedding agent ID',
				)
			: ok,
		enabled && value.similarityThreshold === undefined
			? issue(
					`${path}.similarityThreshold`,
					'Similarity threshold is required when memory is enabled',
				)
			: ok,
		value.similarityThreshold !== undefined
			? validateRange(
					value.similarityThreshold,
					`${path}.similarityThreshold`,
					'Similarity threshold',
					{ min: 0, max: 1 },
				)
			: ok,
		enabled && value.maxResults === undefined
			? issue(
					`${path}.maxResults`,
					'Max results is required when memory is enabled',
				)
			: ok,
		value.maxResults !== undefined
			? validateRange(value.maxResults, `${path}.maxResults`, 'maxResults', {
					min: 1,
					max: 100,
					integer: true,
				})
			: ok,
	);
};

// ---------------------------------------------------------------------------
// Chain Step Definition
// ---------------------------------------------------------------------------

const STEP_NAME_RE = /^[a-zA-Z_][\w-]*$/;

export interface ParallelSubStepDefinitionInput {
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

export interface ParallelConfigInput {
	readonly subSteps: readonly ParallelSubStepDefinitionInput[];
	readonly mergeStrategy?: 'concat' | 'keyed';
	readonly failTolerant?: boolean;
	readonly concatSeparator?: string;
}

export interface ChainStepDefinitionInput {
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
	readonly parallel?: ParallelConfigInput;
}

const validateParallelSubStep = (
	value: ParallelSubStepDefinitionInput,
	path: string,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];

	const nameIssues = validateNonEmpty(
		value.name,
		`${path}.name`,
		'Sub-step name',
	);
	issues.push(...nameIssues);
	if (nameIssues.length === 0 && !STEP_NAME_RE.test(value.name)) {
		issues.push(
			...issue(
				`${path}.name`,
				'Sub-step name must start with a letter or underscore and contain only word characters or hyphens',
			),
		);
	}

	issues.push(
		...validateNonEmpty(
			value.template,
			`${path}.template`,
			'Sub-step template',
		),
	);

	if (value.provider === 'mcp') {
		if (value.mcpServerName === undefined || value.mcpServerName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpServerName`,
					'MCP sub-step requires "mcpServerName" to be set',
				),
			);
		}
		if (value.mcpToolName === undefined || value.mcpToolName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpToolName`,
					'MCP sub-step requires "mcpToolName" to be set',
				),
			);
		}
	}

	return Object.freeze(issues);
};

const validateParallelConfig = (
	value: ParallelConfigInput,
	path: string,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];

	if (value.subSteps.length < 2) {
		issues.push(
			...issue(
				`${path}.subSteps`,
				'Parallel config must have at least 2 sub-steps',
			),
		);
	}

	for (let i = 0; i < value.subSteps.length; i++) {
		issues.push(
			...validateParallelSubStep(value.subSteps[i], `${path}.subSteps[${i}]`),
		);
	}

	// Detect duplicate sub-step names
	if (issues.length === 0) {
		const seen = new Set<string>();
		for (const sub of value.subSteps) {
			if (sub.name.length > 0) {
				if (seen.has(sub.name)) {
					issues.push(
						...issue(
							`${path}.subSteps`,
							`Duplicate sub-step name: "${sub.name}"`,
						),
					);
				}
				seen.add(sub.name);
			}
		}
	}

	return Object.freeze(issues);
};

export const validateChainStepDefinition = (
	value: ChainStepDefinitionInput,
	path: string,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];

	// name
	const nameIssues = validateNonEmpty(
		value.name,
		`${path}.name`,
		'Chain step name',
	);
	issues.push(...nameIssues);
	if (nameIssues.length === 0 && !STEP_NAME_RE.test(value.name)) {
		issues.push(
			...issue(
				`${path}.name`,
				'Step name must start with a letter or underscore and contain only word characters or hyphens',
			),
		);
	}

	// template
	issues.push(
		...validateNonEmpty(
			value.template,
			`${path}.template`,
			'Chain step template',
		),
	);

	// MCP provider requires mcpServerName and mcpToolName
	if (value.provider === 'mcp') {
		if (value.mcpServerName === undefined || value.mcpServerName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpServerName`,
					'MCP step requires "mcpServerName" to be set',
				),
			);
		}
		if (value.mcpToolName === undefined || value.mcpToolName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpToolName`,
					'MCP step requires "mcpToolName" to be set',
				),
			);
		}
	}

	// Parallel config validation
	if (value.parallel !== undefined) {
		issues.push(...validateParallelConfig(value.parallel, `${path}.parallel`));
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// Chain Definition
// ---------------------------------------------------------------------------

export interface ChainDefinitionInput {
	readonly description?: string;
	readonly agentId?: string;
	readonly serverName?: string;
	readonly initialValues?: Readonly<Record<string, string>>;
	readonly steps: readonly ChainStepDefinitionInput[];
}

export const validateChainDefinition = (
	value: ChainDefinitionInput,
	path: string,
): readonly ValidationIssue[] => {
	if (value.steps.length === 0)
		return issue(`${path}.steps`, 'A chain must have at least one step');

	const issues: ValidationIssue[] = [];

	for (let i = 0; i < value.steps.length; i++) {
		issues.push(
			...validateChainStepDefinition(value.steps[i], `${path}.steps[${i}]`),
		);
	}

	// Detect duplicate step names
	if (issues.length === 0) {
		const stepNames = new Set<string>();
		for (const step of value.steps) {
			if (step.name.length > 0) {
				if (stepNames.has(step.name)) {
					issues.push(
						...issue(`${path}.steps`, `Duplicate step name: "${step.name}"`),
					);
				}
				stepNames.add(step.name);
			}
		}
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// Top-Level App Config Input
// ---------------------------------------------------------------------------

export interface AppConfigInput {
	readonly acp: ACPConfigInput;
	readonly mcp?: MCPConfigInput;
	readonly memory?: MemoryConfigInput;
	readonly chains?: Readonly<Record<string, ChainDefinitionInput>>;
}

export const validateAppConfig = (
	value: AppConfigInput,
): readonly ValidationIssue[] => {
	const issues: ValidationIssue[] = [];

	// acp (required)
	issues.push(...validateACPConfig(value.acp, 'acp'));

	// mcp (optional)
	if (value.mcp !== undefined) {
		issues.push(...validateMCPConfig(value.mcp, 'mcp'));
	}

	// memory (optional)
	if (value.memory !== undefined) {
		issues.push(...validateMemoryConfig(value.memory, 'memory'));
	}

	// chains (optional)
	if (value.chains !== undefined) {
		for (const [chainName, chainDef] of Object.entries(value.chains)) {
			issues.push(...validateChainDefinition(chainDef, `chains.${chainName}`));
		}
	}

	return Object.freeze(issues);
};

// ---------------------------------------------------------------------------
// Formatting helper
// ---------------------------------------------------------------------------

/**
 * Format validation issues into human-readable `{ path, message }` tuples
 * suitable for `ConfigValidationError`.
 */
export const formatValidationIssues = (
	issues: readonly ValidationIssue[],
): ReadonlyArray<{ readonly path: string; readonly message: string }> =>
	Object.freeze(
		issues.map((i) => Object.freeze({ path: i.path, message: i.message })),
	);
