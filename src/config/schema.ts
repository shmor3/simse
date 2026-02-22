// ---------------------------------------------------------------------------
// Pure TypeScript Configuration Validation (no Zod)
// ---------------------------------------------------------------------------
//
// All validation is done via type-guard functions and a lightweight
// `ValidationIssue` accumulator.  Every validator is a pure function that
// returns either a list of issues (invalid) or an empty array (valid).
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
// Type guards
// ---------------------------------------------------------------------------

const isString = (v: unknown): v is string => typeof v === 'string';
const isNumber = (v: unknown): v is number =>
	typeof v === 'number' && !Number.isNaN(v);
const isBoolean = (v: unknown): v is boolean => typeof v === 'boolean';
const isObject = (v: unknown): v is Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v);
const isArray = (v: unknown): v is readonly unknown[] => Array.isArray(v);

// ---------------------------------------------------------------------------
// Reusable field validators
// ---------------------------------------------------------------------------

const validateNonEmptyString = (
	value: unknown,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	if (!isString(value)) return issue(path, `${label} must be a string`);
	if (value.length === 0) return issue(path, `${label} cannot be empty`);
	return ok;
};

const validateUrl = (
	value: unknown,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	if (!isString(value)) return issue(path, `${label} must be a string`);
	try {
		new URL(value);
		return ok;
	} catch {
		return issue(path, `${label} must be a valid URL`);
	}
};

const validateOptionalString = (
	value: unknown,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	if (value === undefined || value === null) return ok;
	return validateNonEmptyString(value, path, label);
};

const validateOptionalNumber = (
	value: unknown,
	path: string,
	label: string,
	constraints?: {
		readonly min?: number;
		readonly max?: number;
		readonly integer?: boolean;
	},
): readonly ValidationIssue[] => {
	if (value === undefined || value === null) return ok;
	if (!isNumber(value)) return issue(path, `${label} must be a number`);
	if (constraints?.integer && !Number.isInteger(value))
		return issue(path, `${label} must be an integer`);
	if (constraints?.min !== undefined && value < constraints.min)
		return issue(path, `${label} must be at least ${constraints.min}`);
	if (constraints?.max !== undefined && value > constraints.max)
		return issue(path, `${label} must be at most ${constraints.max}`);
	return ok;
};

const validateOptionalBoolean = (
	value: unknown,
	path: string,
	label: string,
): readonly ValidationIssue[] => {
	if (value === undefined || value === null) return ok;
	if (!isBoolean(value)) return issue(path, `${label} must be a boolean`);
	return ok;
};

// ---------------------------------------------------------------------------
// ACP Server Entry
// ---------------------------------------------------------------------------

export interface ACPServerEntryInput {
	readonly name: string;
	readonly url: string;
	readonly defaultAgent?: string;
	readonly apiKey?: string;
	readonly timeoutMs?: number;
}

export const validateACPServerEntry = (
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value))
		return issue(path, 'ACP server entry must be an object');

	return combine(
		validateNonEmptyString(value.name, `${path}.name`, 'ACP server name'),
		validateUrl(value.url, `${path}.url`, 'ACP server URL'),
		validateOptionalString(
			value.defaultAgent,
			`${path}.defaultAgent`,
			'Default agent ID',
		),
		validateOptionalString(value.apiKey, `${path}.apiKey`, 'API key'),
		validateOptionalNumber(value.timeoutMs, `${path}.timeoutMs`, 'timeoutMs', {
			min: 1000,
			max: 600_000,
			integer: true,
		}),
	);
};

// ---------------------------------------------------------------------------
// ACP Config
// ---------------------------------------------------------------------------

export interface ACPConfigInput {
	readonly servers: readonly ACPServerEntryInput[];
	readonly defaultServer?: string;
	readonly defaultAgent?: string;
}

export const validateACPConfig = (
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value)) return issue(path, 'ACP config must be an object');

	const issues: ValidationIssue[] = [];

	if (!isArray(value.servers))
		return issue(`${path}.servers`, 'ACP servers must be an array');
	if (value.servers.length === 0)
		return issue(
			`${path}.servers`,
			'At least one ACP server must be configured',
		);

	for (let i = 0; i < value.servers.length; i++) {
		issues.push(
			...validateACPServerEntry(value.servers[i], `${path}.servers[${i}]`),
		);
	}

	// Detect duplicate server names
	if (issues.length === 0) {
		const names = (value.servers as readonly { name: unknown }[])
			.map((s) => s.name)
			.filter((n): n is string => typeof n === 'string');
		const seen = new Set<string>();
		for (const name of names) {
			if (seen.has(name)) {
				issues.push(
					...issue(`${path}.servers`, `Duplicate ACP server name: "${name}"`),
				);
			}
			seen.add(name);
		}
	}

	issues.push(
		...validateOptionalString(
			value.defaultServer,
			`${path}.defaultServer`,
			'Default server name',
		),
		...validateOptionalString(
			value.defaultAgent,
			`${path}.defaultAgent`,
			'Default agent ID',
		),
	);

	// Cross-validate: defaultServer must reference a configured server name
	if (
		typeof value.defaultServer === 'string' &&
		value.defaultServer.length > 0
	) {
		const serverNames = new Set(
			(value.servers as readonly { name: unknown }[])
				.map((s) => s.name)
				.filter((n): n is string => typeof n === 'string'),
		);
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
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value))
		return issue(path, 'MCP server connection must be an object');

	const issues: ValidationIssue[] = [];

	issues.push(
		...validateNonEmptyString(value.name, `${path}.name`, 'MCP server name'),
	);

	if (value.transport !== 'stdio' && value.transport !== 'http') {
		return combine(
			Object.freeze(issues),
			issue(`${path}.transport`, 'MCP transport must be "stdio" or "http"'),
		);
	}

	if (value.transport === 'stdio') {
		issues.push(
			...validateNonEmptyString(
				value.command,
				`${path}.command`,
				'stdio command',
			),
		);
		if (value.args !== undefined) {
			if (!isArray(value.args)) {
				issues.push(
					...issue(`${path}.args`, 'stdio args must be an array of strings'),
				);
			} else {
				for (let j = 0; j < value.args.length; j++) {
					if (!isString(value.args[j])) {
						issues.push(
							...issue(
								`${path}.args[${j}]`,
								`stdio arg at index ${j} must be a string`,
							),
						);
					}
				}
			}
		}
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
}

export const validateMCPClientConfig = (
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value))
		return issue(path, 'MCP client config must be an object');

	if (value.servers === undefined) return ok;
	if (!isArray(value.servers))
		return issue(`${path}.servers`, 'MCP client servers must be an array');

	const issues: ValidationIssue[] = [];
	for (let i = 0; i < value.servers.length; i++) {
		issues.push(
			...validateMCPServerConnection(value.servers[i], `${path}.servers[${i}]`),
		);
	}

	// Detect duplicate server names
	if (issues.length === 0) {
		const names = (value.servers as readonly { name: unknown }[])
			.map((s) => s.name)
			.filter((n): n is string => typeof n === 'string');
		const seen = new Set<string>();
		for (const name of names) {
			if (seen.has(name)) {
				issues.push(
					...issue(`${path}.servers`, `Duplicate MCP server name: "${name}"`),
				);
			}
			seen.add(name);
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
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value))
		return issue(path, 'MCP server config must be an object');

	const issues: ValidationIssue[] = [];

	issues.push(
		...validateOptionalBoolean(
			value.enabled,
			`${path}.enabled`,
			'MCP server enabled',
		),
	);

	if (value.name !== undefined) {
		issues.push(
			...validateNonEmptyString(value.name, `${path}.name`, 'MCP server name'),
		);
	}

	if (value.version !== undefined) {
		if (!isString(value.version) || !SEMVER_RE.test(value.version)) {
			issues.push(
				...issue(
					`${path}.version`,
					'MCP server version must be semver (e.g. 1.0.0)',
				),
			);
		}
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
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value)) return issue(path, 'MCP config must be an object');

	return combine(
		value.client !== undefined
			? validateMCPClientConfig(value.client, `${path}.client`)
			: ok,
		value.server !== undefined
			? validateMCPServerConfig(value.server, `${path}.server`)
			: ok,
	);
};

// ---------------------------------------------------------------------------
// Memory Config
// ---------------------------------------------------------------------------

export interface MemoryConfigInput {
	readonly enabled?: boolean;
	readonly embeddingAgent?: string;
	readonly storePath?: string;
	readonly similarityThreshold?: number;
	readonly maxResults?: number;
}

export const validateMemoryConfig = (
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value)) return issue(path, 'Memory config must be an object');

	const enabled = value.enabled !== false;

	return combine(
		validateOptionalBoolean(value.enabled, `${path}.enabled`, 'Memory enabled'),
		enabled
			? validateNonEmptyString(
					value.embeddingAgent,
					`${path}.embeddingAgent`,
					'Embedding agent ID (required when memory is enabled)',
				)
			: validateOptionalString(
					value.embeddingAgent,
					`${path}.embeddingAgent`,
					'Embedding agent ID',
				),
		value.storePath !== undefined
			? validateNonEmptyString(
					value.storePath,
					`${path}.storePath`,
					'Store path',
				)
			: ok,
		validateOptionalNumber(
			value.similarityThreshold,
			`${path}.similarityThreshold`,
			'Similarity threshold',
			{
				min: 0,
				max: 1,
			},
		),
		validateOptionalNumber(
			value.maxResults,
			`${path}.maxResults`,
			'maxResults',
			{
				min: 1,
				max: 100,
				integer: true,
			},
		),
	);
};

// ---------------------------------------------------------------------------
// Chain Step Definition
// ---------------------------------------------------------------------------

const STEP_NAME_RE = /^[a-zA-Z_][\w-]*$/;

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
}

export const validateChainStepDefinition = (
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value)) return issue(path, 'Chain step must be an object');

	const issues: ValidationIssue[] = [];

	// name
	const nameIssues = validateNonEmptyString(
		value.name,
		`${path}.name`,
		'Chain step name',
	);
	issues.push(...nameIssues);
	if (
		nameIssues.length === 0 &&
		isString(value.name) &&
		!STEP_NAME_RE.test(value.name)
	) {
		issues.push(
			...issue(
				`${path}.name`,
				'Step name must start with a letter or underscore and contain only word characters or hyphens',
			),
		);
	}

	// template
	issues.push(
		...validateNonEmptyString(
			value.template,
			`${path}.template`,
			'Chain step template',
		),
	);

	// provider
	if (value.provider !== undefined) {
		if (
			value.provider !== 'acp' &&
			value.provider !== 'mcp' &&
			value.provider !== 'memory'
		) {
			issues.push(
				...issue(
					`${path}.provider`,
					'Provider must be "acp", "mcp", or "memory"',
				),
			);
		}
	}

	// optional strings
	issues.push(
		...validateOptionalString(value.agentId, `${path}.agentId`, 'Agent ID'),
		...validateOptionalString(
			value.serverName,
			`${path}.serverName`,
			'Server name',
		),
		...validateOptionalString(
			value.mcpServerName,
			`${path}.mcpServerName`,
			'MCP server name',
		),
		...validateOptionalString(
			value.mcpToolName,
			`${path}.mcpToolName`,
			'MCP tool name',
		),
	);

	// optional fields
	issues.push(
		...validateOptionalString(
			value.systemPrompt,
			`${path}.systemPrompt`,
			'System prompt',
		),
	);
	if (value.storeToMemory !== undefined && !isBoolean(value.storeToMemory)) {
		issues.push(
			...issue(`${path}.storeToMemory`, 'storeToMemory must be a boolean'),
		);
	}

	// MCP provider requires mcpServerName and mcpToolName
	if (value.provider === 'mcp') {
		if (!isString(value.mcpServerName) || value.mcpServerName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpServerName`,
					'MCP step requires "mcpServerName" to be set',
				),
			);
		}
		if (!isString(value.mcpToolName) || value.mcpToolName.length === 0) {
			issues.push(
				...issue(
					`${path}.mcpToolName`,
					'MCP step requires "mcpToolName" to be set',
				),
			);
		}
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
	value: unknown,
	path: string,
): readonly ValidationIssue[] => {
	if (!isObject(value))
		return issue(path, 'Chain definition must be an object');

	const issues: ValidationIssue[] = [];

	if (!isArray(value.steps))
		return issue(`${path}.steps`, 'Chain steps must be an array');
	if (value.steps.length === 0)
		return issue(`${path}.steps`, 'A chain must have at least one step');

	issues.push(
		...validateOptionalString(
			value.agentId,
			`${path}.agentId`,
			'Chain agent ID',
		),
		...validateOptionalString(
			value.serverName,
			`${path}.serverName`,
			'Chain server name',
		),
	);

	for (let i = 0; i < value.steps.length; i++) {
		issues.push(
			...validateChainStepDefinition(value.steps[i], `${path}.steps[${i}]`),
		);
	}

	// Detect duplicate step names
	if (issues.length === 0) {
		const stepNames = new Set<string>();
		for (const step of value.steps as readonly Record<string, unknown>[]) {
			const name = step?.name;
			if (typeof name === 'string' && name.length > 0) {
				if (stepNames.has(name)) {
					issues.push(
						...issue(`${path}.steps`, `Duplicate step name: "${name}"`),
					);
				}
				stepNames.add(name);
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
	value: unknown,
	path = '(root)',
): readonly ValidationIssue[] => {
	if (!isObject(value)) return issue(path, 'Config must be an object');

	const issues: ValidationIssue[] = [];

	// acp (required)
	if (value.acp === undefined) {
		issues.push(...issue('acp', 'ACP configuration is required'));
	} else {
		issues.push(...validateACPConfig(value.acp, 'acp'));
	}

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
		if (!isObject(value.chains)) {
			issues.push(...issue('chains', 'Chains must be an object'));
		} else {
			for (const [chainName, chainDef] of Object.entries(value.chains)) {
				issues.push(
					...validateChainDefinition(chainDef, `chains.${chainName}`),
				);
			}
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
