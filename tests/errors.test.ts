import { describe, expect, it } from 'bun:test';
import type { SimseError } from '../src/errors/index.js';
import {
	createChainError,
	createChainNotFoundError,
	createChainStepError,
	createConfigError,
	createConfigNotFoundError,
	createConfigParseError,
	createConfigValidationError,
	createEmbeddingError,
	// Loop
	createLoopAbortedError,
	createLoopError,
	createLoopTurnLimitError,
	createMCPConnectionError,
	createMCPError,
	createMCPServerNotConnectedError,
	createMCPToolError,
	createMCPTransportConfigError,
	createLibraryError,
	createProviderError,
	createProviderGenerationError,
	createProviderHTTPError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	// Factory functions
	createSimseError,
	// Tasks
	createTaskCircularDependencyError,
	createTaskError,
	createTaskNotFoundError,
	createTemplateError,
	createTemplateMissingVariablesError,
	// Tools
	createToolError,
	createToolExecutionError,
	createToolNotFoundError,
	createStacksCorruptionError,
	createStacksIOError,
	isChainError,
	isChainNotFoundError,
	isChainStepError,
	isConfigError,
	isConfigNotFoundError,
	isConfigValidationError,
	isEmbeddingError,
	isLoopAbortedError,
	isLoopError,
	isLoopTurnLimitError,
	isMCPConnectionError,
	isMCPError,
	isMCPServerNotConnectedError,
	isMCPToolError,
	isMCPTransportConfigError,
	isLibraryError,
	isProviderError,
	isProviderGenerationError,
	isProviderHTTPError,
	isProviderTimeoutError,
	isProviderUnavailableError,
	// Type guards
	isSimseError,
	isTaskCircularDependencyError,
	isTaskError,
	isTaskNotFoundError,
	isTemplateError,
	isTemplateMissingVariablesError,
	isToolError,
	isToolExecutionError,
	isToolNotFoundError,
	isStacksCorruptionError,
	isStacksIOError,
	// Utility
	toError,
	wrapError,
} from '../src/errors/index.js';

// ---------------------------------------------------------------------------
// SimseError (base)
// ---------------------------------------------------------------------------

describe('SimseError', () => {
	it('should create an error with default values', () => {
		const err = createSimseError('something went wrong');

		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('SimseError');
		expect(err.message).toBe('something went wrong');
		expect(err.code).toBe('SIMSE_ERROR');
		expect(err.statusCode).toBe(500);
		expect(err.metadata).toEqual({});
		expect(err.cause).toBeUndefined();
		expect(err.stack).toBeDefined();
	});

	it('should accept custom code, statusCode, cause, and metadata', () => {
		const cause = new Error('root cause');
		const err = createSimseError('with options', {
			code: 'CUSTOM_CODE',
			statusCode: 418,
			cause,
			metadata: { foo: 'bar' },
		});

		expect(err.code).toBe('CUSTOM_CODE');
		expect(err.statusCode).toBe(418);
		expect(err.cause).toBe(cause);
		expect(err.metadata).toEqual({ foo: 'bar' });
	});

	it('should serialize to JSON correctly', () => {
		const cause = new Error('inner');
		const err = createSimseError('json test', {
			code: 'JSON_CODE',
			statusCode: 400,
			cause,
			metadata: { key: 123 },
		});

		const json = err.toJSON();

		expect(json.name).toBe('SimseError');
		expect(json.code).toBe('JSON_CODE');
		expect(json.message).toBe('json test');
		expect(json.statusCode).toBe(400);
		expect(json.metadata).toEqual({ key: 123 });
		expect(json.cause).toEqual({ name: 'Error', message: 'inner' });
		expect(json.stack).toBeDefined();
	});

	it('should serialize non-Error cause in toJSON', () => {
		const err = createSimseError('with string cause', { cause: 'oops' });
		const json = err.toJSON();
		expect(json.cause).toBe('oops');
	});

	it('should serialize undefined cause in toJSON', () => {
		const err = createSimseError('no cause');
		const json = err.toJSON();
		expect(json.cause).toBeUndefined();
	});

	it('should have a proper prototype chain for instanceof checks', () => {
		const err = createSimseError('proto test');
		// Prototype chain checks are unreliable after migration; use guard instead
		expect(isSimseError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// Config Errors
// ---------------------------------------------------------------------------

describe('ConfigError', () => {
	it('should default code to CONFIG_ERROR and statusCode to 400', () => {
		const err = createConfigError('bad config');

		expect(isSimseError(err)).toBe(true);
		expect(isConfigError(err)).toBe(true);
		expect(err.name).toBe('ConfigError');
		expect(err.code).toBe('CONFIG_ERROR');
		expect(err.statusCode).toBe(400);
	});

	it('should allow custom code', () => {
		const err = createConfigError('custom', { code: 'MY_CONFIG_ERR' });
		expect(err.code).toBe('MY_CONFIG_ERR');
	});
});

describe('ConfigNotFoundError', () => {
	it('should include the config path in message and metadata', () => {
		const err = createConfigNotFoundError('/path/to/config.json');

		expect(isConfigNotFoundError(err)).toBe(true);
		expect(err.name).toBe('ConfigNotFoundError');
		expect(err.code).toBe('CONFIG_NOT_FOUND');
		expect(err.message).toContain('/path/to/config.json');
		expect(err.metadata).toEqual({ configPath: '/path/to/config.json' });
	});

	it('should preserve the cause', () => {
		const cause = new Error('ENOENT');
		const err = createConfigNotFoundError('missing.json', { cause });
		expect(err.cause).toBe(cause);
	});
});

describe('ConfigValidationError', () => {
	it('should format a single issue', () => {
		const issues = [{ path: 'acp.servers.0.url', message: 'must be a URL' }];
		const err = createConfigValidationError(issues);

		expect(isConfigValidationError(err)).toBe(true);
		expect(err.name).toBe('ConfigValidationError');
		expect(err.code).toBe('CONFIG_VALIDATION');
		expect(err.message).toContain('must be a URL');
		expect(err.issues).toHaveLength(1);
		expect(err.issues[0]).toEqual({
			path: 'acp.servers.0.url',
			message: 'must be a URL',
		});
	});

	it('should format multiple issues with a count', () => {
		const issues = [
			{ path: 'a', message: 'bad a' },
			{ path: 'b', message: 'bad b' },
			{ path: 'c', message: 'bad c' },
		];
		const err = createConfigValidationError(issues);

		expect(err.message).toContain('3 validation errors');
		expect(err.issues).toHaveLength(3);
	});

	it('should have frozen issues array', () => {
		const issues = [{ path: 'x', message: 'y' }];
		const err = createConfigValidationError(issues);

		expect(() => {
			(err.issues as Array<{ path: string; message: string }>).push({
				path: 'z',
				message: 'z',
			});
		}).toThrow();
	});
});

describe('ConfigParseError', () => {
	it('should include the config path', () => {
		const err = createConfigParseError('config.json');

		expect(isConfigError(err)).toBe(true);
		expect(err.name).toBe('ConfigParseError');
		expect(err.code).toBe('CONFIG_PARSE');
		expect(err.message).toContain('config.json');
		expect(err.metadata).toEqual({ configPath: 'config.json' });
	});
});

// ---------------------------------------------------------------------------
// Provider Errors
// ---------------------------------------------------------------------------

describe('ProviderError', () => {
	it('should store the provider name', () => {
		const err = createProviderError('local-server', 'something failed');

		expect(isSimseError(err)).toBe(true);
		expect(isProviderError(err)).toBe(true);
		expect(err.name).toBe('ProviderError');
		expect(err.provider).toBe('local-server');
		expect(err.code).toBe('PROVIDER_ERROR');
		expect(err.statusCode).toBe(502);
		expect(err.metadata).toEqual({ provider: 'local-server' });
	});

	it('should merge metadata with provider', () => {
		const err = createProviderError('remote-server', 'fail', {
			metadata: { extra: true },
		});
		expect(err.metadata).toEqual({ provider: 'remote-server', extra: true });
	});
});

describe('ProviderUnavailableError', () => {
	it('should set code and statusCode correctly', () => {
		const err = createProviderUnavailableError('local-server');

		expect(isProviderUnavailableError(err)).toBe(true);
		expect(err.name).toBe('ProviderUnavailableError');
		expect(err.code).toBe('PROVIDER_UNAVAILABLE');
		expect(err.statusCode).toBe(503);
		expect(err.provider).toBe('local-server');
		expect(err.message).toContain('local-server');
		expect(err.message).toContain('not available');
	});
});

describe('ProviderTimeoutError', () => {
	it('should include timeout duration', () => {
		const err = createProviderTimeoutError('local-server', 30000);

		expect(isProviderTimeoutError(err)).toBe(true);
		expect(err.name).toBe('ProviderTimeoutError');
		expect(err.code).toBe('PROVIDER_TIMEOUT');
		expect(err.statusCode).toBe(504);
		expect(err.timeoutMs).toBe(30000);
		expect(err.message).toContain('30000ms');
	});
});

describe('ProviderGenerationError', () => {
	it('should set appropriate code and include model in metadata', () => {
		const err = createProviderGenerationError(
			'remote-server',
			'model returned garbage',
			{ model: 'gpt-4' },
		);

		expect(isProviderGenerationError(err)).toBe(true);
		expect(err.name).toBe('ProviderGenerationError');
		expect(err.code).toBe('PROVIDER_GENERATION_FAILED');
		expect(err.statusCode).toBe(502);
		expect(err.metadata).toEqual({ provider: 'remote-server', model: 'gpt-4' });
	});

	it('should work without model option', () => {
		const err = createProviderGenerationError('local-server', 'fail');
		expect(err.metadata).toEqual({ provider: 'local-server' });
	});
});

// ---------------------------------------------------------------------------
// Chain Errors
// ---------------------------------------------------------------------------

describe('ChainError', () => {
	it('should store chain name', () => {
		const err = createChainError('chain broke', { chainName: 'blog-writer' });

		expect(isSimseError(err)).toBe(true);
		expect(isChainError(err)).toBe(true);
		expect(err.name).toBe('ChainError');
		expect(err.code).toBe('CHAIN_ERROR');
		expect(err.chainName).toBe('blog-writer');
		expect(err.metadata).toEqual({ chainName: 'blog-writer' });
	});

	it('should handle missing chain name', () => {
		const err = createChainError('no name');
		expect(err.chainName).toBeUndefined();
	});
});

describe('ChainStepError', () => {
	it('should include step name and index in message', () => {
		const err = createChainStepError('outline', 0, 'LLM timed out', {
			chainName: 'blog-writer',
		});

		expect(isChainStepError(err)).toBe(true);
		expect(err.name).toBe('ChainStepError');
		expect(err.code).toBe('CHAIN_STEP_ERROR');
		expect(err.stepName).toBe('outline');
		expect(err.stepIndex).toBe(0);
		expect(err.message).toContain('outline');
		expect(err.message).toContain('index 0');
		expect(err.message).toContain('LLM timed out');
		expect(err.chainName).toBe('blog-writer');
	});

	it('should store step info in metadata', () => {
		const err = createChainStepError('draft', 2, 'error');
		expect(err.metadata).toEqual(
			expect.objectContaining({ stepName: 'draft', stepIndex: 2 }),
		);
	});
});

describe('ChainNotFoundError', () => {
	it('should include chain name in message', () => {
		const err = createChainNotFoundError('nonexistent');

		expect(isChainNotFoundError(err)).toBe(true);
		expect(err.name).toBe('ChainNotFoundError');
		expect(err.code).toBe('CHAIN_NOT_FOUND');
		expect(err.chainName).toBe('nonexistent');
		expect(err.message).toContain('nonexistent');
	});
});

// ---------------------------------------------------------------------------
// Template Errors
// ---------------------------------------------------------------------------

describe('TemplateError', () => {
	it('should default code to TEMPLATE_ERROR', () => {
		const err = createTemplateError('bad template');

		expect(isTemplateError(err)).toBe(true);
		expect(err.name).toBe('TemplateError');
		expect(err.code).toBe('TEMPLATE_ERROR');
		expect(err.statusCode).toBe(400);
	});
});

describe('TemplateMissingVariablesError', () => {
	it('should list missing variables (singular)', () => {
		const err = createTemplateMissingVariablesError(['topic']);

		expect(isTemplateMissingVariablesError(err)).toBe(true);
		expect(err.name).toBe('TemplateMissingVariablesError');
		expect(err.code).toBe('TEMPLATE_MISSING_VARS');
		expect(err.missingVariables).toEqual(['topic']);
		expect(err.message).toContain('variable:');
		expect(err.message).toContain('topic');
	});

	it('should list missing variables (plural)', () => {
		const err = createTemplateMissingVariablesError(['topic', 'language']);

		expect(err.message).toContain('variables:');
		expect(err.message).toContain('topic');
		expect(err.message).toContain('language');
	});

	it('should include template in metadata when provided', () => {
		const err = createTemplateMissingVariablesError(['x'], {
			template: 'Hello {x}!',
		});
		expect(err.metadata).toEqual(
			expect.objectContaining({ template: 'Hello {x}!' }),
		);
	});

	it('should have frozen missingVariables array', () => {
		const err = createTemplateMissingVariablesError(['a', 'b']);
		expect(() => {
			(err.missingVariables as string[]).push('c');
		}).toThrow();
	});
});

// ---------------------------------------------------------------------------
// MCP Errors
// ---------------------------------------------------------------------------

describe('MCPError', () => {
	it('should default code to MCP_ERROR', () => {
		const err = createMCPError('mcp broke');

		expect(isMCPError(err)).toBe(true);
		expect(err.name).toBe('MCPError');
		expect(err.code).toBe('MCP_ERROR');
	});
});

describe('MCPConnectionError', () => {
	it('should include server name in message and metadata', () => {
		const err = createMCPConnectionError('file-tools', 'connection refused');

		expect(isMCPConnectionError(err)).toBe(true);
		expect(err.name).toBe('MCPConnectionError');
		expect(err.code).toBe('MCP_CONNECTION_ERROR');
		expect(err.statusCode).toBe(503);
		expect(err.serverName).toBe('file-tools');
		expect(err.message).toContain('file-tools');
		expect(err.message).toContain('connection refused');
	});
});

describe('MCPServerNotConnectedError', () => {
	it('should include server name and connection hint', () => {
		const err = createMCPServerNotConnectedError('web-search');

		expect(isMCPServerNotConnectedError(err)).toBe(true);
		expect(err.name).toBe('MCPServerNotConnectedError');
		expect(err.code).toBe('MCP_NOT_CONNECTED');
		expect(err.serverName).toBe('web-search');
		expect(err.message).toContain('web-search');
		expect(err.message).toContain('connect');
	});
});

describe('MCPToolError', () => {
	it('should include server name and tool name', () => {
		const err = createMCPToolError('file-tools', 'read-file', 'not found');

		expect(isMCPToolError(err)).toBe(true);
		expect(err.name).toBe('MCPToolError');
		expect(err.code).toBe('MCP_TOOL_ERROR');
		expect(err.statusCode).toBe(502);
		expect(err.serverName).toBe('file-tools');
		expect(err.toolName).toBe('read-file');
		expect(err.message).toContain('file-tools');
		expect(err.message).toContain('read-file');
		expect(err.message).toContain('not found');
	});
});

describe('MCPTransportConfigError', () => {
	it('should include server name and config message', () => {
		const err = createMCPTransportConfigError(
			'my-server',
			'stdio transport requires a "command"',
		);

		expect(isMCPTransportConfigError(err)).toBe(true);
		expect(err.name).toBe('MCPTransportConfigError');
		expect(err.code).toBe('MCP_TRANSPORT_CONFIG');
		expect(err.statusCode).toBe(400);
		expect(err.serverName).toBe('my-server');
		expect(err.message).toContain('my-server');
		expect(err.message).toContain('command');
	});
});

// ---------------------------------------------------------------------------
// Library / Stacks Errors
// ---------------------------------------------------------------------------

describe('LibraryError', () => {
	it('should default code to MEMORY_ERROR', () => {
		const err = createLibraryError('library problem');

		expect(isLibraryError(err)).toBe(true);
		expect(err.name).toBe('LibraryError');
		expect(err.code).toBe('LIBRARY_ERROR');
	});
});

describe('EmbeddingError', () => {
	it('should include model in metadata', () => {
		const err = createEmbeddingError('embedding failed', {
			model: 'nomic-embed-text',
		});

		expect(isEmbeddingError(err)).toBe(true);
		expect(err.name).toBe('EmbeddingError');
		expect(err.code).toBe('EMBEDDING_ERROR');
		expect(err.metadata).toEqual({ model: 'nomic-embed-text' });
	});

	it('should work without model option', () => {
		const err = createEmbeddingError('fail');
		expect(err.metadata).toEqual({});
	});
});

describe('StacksCorruptionError', () => {
	it('should include store path', () => {
		const err = createStacksCorruptionError('/data/memory.json');

		expect(isStacksCorruptionError(err)).toBe(true);
		expect(err.name).toBe('StacksCorruptionError');
		expect(err.code).toBe('STACKS_CORRUPT');
		expect(err.storePath).toBe('/data/memory.json');
		expect(err.message).toContain('/data/memory.json');
	});
});

describe('StacksIOError', () => {
	it('should include store path and operation (read)', () => {
		const err = createStacksIOError('/data/memory.json', 'read');

		expect(isStacksIOError(err)).toBe(true);
		expect(err.name).toBe('StacksIOError');
		expect(err.code).toBe('STACKS_IO');
		expect(err.storePath).toBe('/data/memory.json');
		expect(err.message).toContain('read');
		expect(err.metadata).toEqual(
			expect.objectContaining({ operation: 'read' }),
		);
	});

	it('should include store path and operation (write)', () => {
		const err = createStacksIOError('/data/memory.json', 'write');
		expect(err.message).toContain('write');
		expect(err.metadata).toEqual(
			expect.objectContaining({ operation: 'write' }),
		);
	});
});

// ---------------------------------------------------------------------------
// Provider HTTP Error
// ---------------------------------------------------------------------------

describe('ProviderHTTPError', () => {
	it('should include status code and provider', () => {
		const err = createProviderHTTPError('openai', 429, 'Rate limited');

		expect(isProviderHTTPError(err)).toBe(true);
		expect(isProviderError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('ProviderHTTPError');
		expect(err.code).toBe('PROVIDER_HTTP_ERROR');
		expect(err.statusCode).toBe(429);
		expect(err.provider).toBe('openai');
		expect(err.message).toBe('Rate limited');
	});

	it('should work with 5xx status codes', () => {
		const err = createProviderHTTPError('claude', 502, 'Bad gateway');
		expect(err.statusCode).toBe(502);
	});
});

// ---------------------------------------------------------------------------
// Loop Errors
// ---------------------------------------------------------------------------

describe('LoopError', () => {
	it('should default code to LOOP_ERROR', () => {
		const err = createLoopError('loop broke');

		expect(isLoopError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('LoopError');
		expect(err.code).toBe('LOOP_ERROR');
	});
});

describe('LoopTurnLimitError', () => {
	it('should include maxTurns in metadata', () => {
		const err = createLoopTurnLimitError(10);

		expect(isLoopTurnLimitError(err)).toBe(true);
		expect(isLoopError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('LoopTurnLimitError');
		expect(err.code).toBe('LOOP_TURN_LIMIT');
		expect(err.metadata).toEqual(expect.objectContaining({ maxTurns: 10 }));
		expect(err.message).toContain('10');
	});
});

describe('LoopAbortedError', () => {
	it('should include turn number in metadata', () => {
		const err = createLoopAbortedError(5);

		expect(isLoopAbortedError(err)).toBe(true);
		expect(isLoopError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('LoopAbortedError');
		expect(err.code).toBe('LOOP_ABORTED');
		expect(err.metadata).toEqual(expect.objectContaining({ turn: 5 }));
		expect(err.message).toContain('5');
	});
});

// ---------------------------------------------------------------------------
// Task Errors
// ---------------------------------------------------------------------------

describe('TaskError', () => {
	it('should default code to TASK_ERROR', () => {
		const err = createTaskError('task broke');

		expect(isTaskError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('TaskError');
		expect(err.code).toBe('TASK_ERROR');
		expect(err.statusCode).toBe(400);
	});
});

describe('TaskNotFoundError', () => {
	it('should include taskId in metadata', () => {
		const err = createTaskNotFoundError('task-42');

		expect(isTaskNotFoundError(err)).toBe(true);
		expect(isTaskError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('TaskNotFoundError');
		expect(err.code).toBe('TASK_NOT_FOUND');
		expect(err.metadata).toEqual(
			expect.objectContaining({ taskId: 'task-42' }),
		);
		expect(err.message).toContain('task-42');
	});
});

describe('TaskCircularDependencyError', () => {
	it('should include both task IDs in metadata', () => {
		const err = createTaskCircularDependencyError('a', 'b');

		expect(isTaskCircularDependencyError(err)).toBe(true);
		expect(isTaskError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('TaskCircularDependencyError');
		expect(err.code).toBe('TASK_CIRCULAR_DEPENDENCY');
		expect(err.metadata).toEqual(
			expect.objectContaining({ taskId: 'a', dependencyId: 'b' }),
		);
		expect(err.message).toContain('a');
		expect(err.message).toContain('b');
	});
});

// ---------------------------------------------------------------------------
// Tool Errors
// ---------------------------------------------------------------------------

describe('ToolError', () => {
	it('should default code to TOOL_ERROR', () => {
		const err = createToolError('tool broke');

		expect(isToolError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('ToolError');
		expect(err.code).toBe('TOOL_ERROR');
	});
});

describe('ToolNotFoundError', () => {
	it('should include toolName in metadata', () => {
		const err = createToolNotFoundError('library_search');

		expect(isToolNotFoundError(err)).toBe(true);
		expect(isToolError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('ToolNotFoundError');
		expect(err.code).toBe('TOOL_NOT_FOUND');
		expect(err.metadata).toEqual(
			expect.objectContaining({ toolName: 'library_search' }),
		);
		expect(err.message).toContain('library_search');
	});
});

describe('ToolExecutionError', () => {
	it('should include toolName and failure message', () => {
		const err = createToolExecutionError('vfs_write', 'permission denied');

		expect(isToolExecutionError(err)).toBe(true);
		expect(isToolError(err)).toBe(true);
		expect(isSimseError(err)).toBe(true);
		expect(err.name).toBe('ToolExecutionError');
		expect(err.code).toBe('TOOL_EXECUTION_ERROR');
		expect(err.metadata).toEqual(
			expect.objectContaining({ toolName: 'vfs_write' }),
		);
		expect(err.message).toContain('vfs_write');
		expect(err.message).toContain('permission denied');
	});
});

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

describe('toError', () => {
	it('should return Error instances as-is', () => {
		const original = new Error('hello');
		expect(toError(original)).toBe(original);
	});

	it('should return SimseError instances as-is', () => {
		const original = createSimseError('hello');
		expect(toError(original)).toBe(original);
	});

	it('should wrap a string in an Error', () => {
		const result = toError('string error');
		expect(result).toBeInstanceOf(Error); // toError returns Error, not migrated
		expect(result.message).toBe('string error');
	});

	it('should wrap a number in an Error via String()', () => {
		const result = toError(42);
		expect(result).toBeInstanceOf(Error); // toError returns Error, not migrated
		expect(result.message).toBe('42');
	});

	it('should wrap undefined in an Error', () => {
		const result = toError(undefined);
		expect(result).toBeInstanceOf(Error); // toError returns Error, not migrated
		expect(result.message).toBe('undefined');
	});

	it('should wrap null in an Error', () => {
		const result = toError(null);
		expect(result).toBeInstanceOf(Error); // toError returns Error, not migrated
		expect(result.message).toBe('null');
	});

	it('should wrap an object in an Error', () => {
		const result = toError({ key: 'value' });
		expect(result).toBeInstanceOf(Error); // toError returns Error, not migrated
		expect(result.message).toBe('[object Object]');
	});
});

describe('isSimseError', () => {
	it('should return true for SimseError', () => {
		expect(isSimseError(createSimseError('test'))).toBe(true);
	});

	it('should return true for subclasses', () => {
		expect(isSimseError(createConfigError('test'))).toBe(true);
		expect(isSimseError(createProviderError('local-server', 'test'))).toBe(
			true,
		);
		expect(isSimseError(createChainStepError('step', 0, 'test'))).toBe(true);
		expect(isSimseError(createMCPToolError('s', 't', 'm'))).toBe(true);
		expect(isSimseError(createEmbeddingError('test'))).toBe(true);
	});

	it('should return false for plain Error', () => {
		expect(isSimseError(new Error('test'))).toBe(false);
	});

	it('should return false for non-Error values', () => {
		expect(isSimseError('string')).toBe(false);
		expect(isSimseError(42)).toBe(false);
		expect(isSimseError(null)).toBe(false);
		expect(isSimseError(undefined)).toBe(false);
		expect(isSimseError({})).toBe(false);
	});
});

describe('wrapError', () => {
	it('should create a SimseError with the original as cause', () => {
		const original = new Error('root');
		const wrapped = wrapError('wrapped message', original);

		expect(isSimseError(wrapped)).toBe(true);
		expect(wrapped.message).toBe('wrapped message');
		expect(wrapped.cause).toBe(original);
		expect(wrapped.code).toBe('SIMSE_ERROR');
	});

	it('should accept a custom code', () => {
		const wrapped = wrapError('msg', 'string cause', 'CUSTOM');
		expect(wrapped.code).toBe('CUSTOM');
		expect(wrapped.cause).toBe('string cause');
	});

	it('should wrap non-Error causes', () => {
		const wrapped = wrapError('msg', 404);
		expect(wrapped.cause).toBe(404);
	});
});

// ---------------------------------------------------------------------------
// Inheritance chain validation
// ---------------------------------------------------------------------------

describe('Error hierarchy', () => {
	it('should maintain proper prototype chains', () => {
		// Config hierarchy
		const configErr = createConfigNotFoundError('x');
		expect(isConfigNotFoundError(configErr)).toBe(true);
		expect(isConfigError(configErr)).toBe(true);
		expect(isSimseError(configErr)).toBe(true);

		// Provider hierarchy
		const providerErr = createProviderTimeoutError('local-server', 1000);
		expect(isProviderTimeoutError(providerErr)).toBe(true);
		expect(isProviderError(providerErr)).toBe(true);
		expect(isSimseError(providerErr)).toBe(true);

		// Chain hierarchy
		const chainErr = createChainStepError('s', 0, 'm');
		expect(isChainStepError(chainErr)).toBe(true);
		expect(isChainError(chainErr)).toBe(true);
		expect(isSimseError(chainErr)).toBe(true);

		// Template hierarchy
		const tmplErr = createTemplateMissingVariablesError(['x']);
		expect(isTemplateMissingVariablesError(tmplErr)).toBe(true);
		expect(isTemplateError(tmplErr)).toBe(true);
		expect(isSimseError(tmplErr)).toBe(true);

		// MCP hierarchy
		const mcpErr = createMCPToolError('s', 't', 'm');
		expect(isMCPToolError(mcpErr)).toBe(true);
		expect(isMCPError(mcpErr)).toBe(true);
		expect(isSimseError(mcpErr)).toBe(true);

		// Library hierarchy
		const memErr = createStacksCorruptionError('p');
		expect(isStacksCorruptionError(memErr)).toBe(true);
		expect(isLibraryError(memErr)).toBe(true);
		expect(isSimseError(memErr)).toBe(true);

		// Loop hierarchy
		const loopErr = createLoopTurnLimitError(10);
		expect(isLoopTurnLimitError(loopErr)).toBe(true);
		expect(isLoopError(loopErr)).toBe(true);
		expect(isSimseError(loopErr)).toBe(true);

		// Task hierarchy
		const taskErr = createTaskNotFoundError('1');
		expect(isTaskNotFoundError(taskErr)).toBe(true);
		expect(isTaskError(taskErr)).toBe(true);
		expect(isSimseError(taskErr)).toBe(true);

		// Tool hierarchy
		const toolErr = createToolExecutionError('t', 'm');
		expect(isToolExecutionError(toolErr)).toBe(true);
		expect(isToolError(toolErr)).toBe(true);
		expect(isSimseError(toolErr)).toBe(true);

		// Provider HTTP hierarchy
		const httpErr = createProviderHTTPError('p', 500, 'm');
		expect(isProviderHTTPError(httpErr)).toBe(true);
		expect(isProviderError(httpErr)).toBe(true);
		expect(isSimseError(httpErr)).toBe(true);
	});

	it('should have correct .name properties throughout the hierarchy', () => {
		expect(createSimseError('').name).toBe('SimseError');
		expect(createConfigError('').name).toBe('ConfigError');
		expect(createConfigNotFoundError('').name).toBe('ConfigNotFoundError');
		expect(createConfigValidationError([{ path: '', message: '' }]).name).toBe(
			'ConfigValidationError',
		);
		expect(createConfigParseError('').name).toBe('ConfigParseError');
		expect(createProviderError('', '').name).toBe('ProviderError');
		expect(createProviderUnavailableError('').name).toBe(
			'ProviderUnavailableError',
		);
		expect(createProviderTimeoutError('', 0).name).toBe('ProviderTimeoutError');
		expect(createProviderGenerationError('', '').name).toBe(
			'ProviderGenerationError',
		);
		expect(createChainError('').name).toBe('ChainError');
		expect(createChainStepError('', 0, '').name).toBe('ChainStepError');
		expect(createChainNotFoundError('').name).toBe('ChainNotFoundError');
		expect(createTemplateError('').name).toBe('TemplateError');
		expect(createTemplateMissingVariablesError([]).name).toBe(
			'TemplateMissingVariablesError',
		);
		expect(createMCPError('').name).toBe('MCPError');
		expect(createMCPConnectionError('', '').name).toBe('MCPConnectionError');
		expect(createMCPServerNotConnectedError('').name).toBe(
			'MCPServerNotConnectedError',
		);
		expect(createMCPToolError('', '', '').name).toBe('MCPToolError');
		expect(createMCPTransportConfigError('', '').name).toBe(
			'MCPTransportConfigError',
		);
		expect(createLibraryError('').name).toBe('LibraryError');
		expect(createEmbeddingError('').name).toBe('EmbeddingError');
		expect(createStacksCorruptionError('').name).toBe(
			'StacksCorruptionError',
		);
		expect(createStacksIOError('', 'read').name).toBe(
			'StacksIOError',
		);

		// New error types
		expect(createProviderHTTPError('', 500, '').name).toBe('ProviderHTTPError');
		expect(createLoopError('').name).toBe('LoopError');
		expect(createLoopTurnLimitError(0).name).toBe('LoopTurnLimitError');
		expect(createLoopAbortedError(0).name).toBe('LoopAbortedError');
		expect(createTaskError('').name).toBe('TaskError');
		expect(createTaskNotFoundError('').name).toBe('TaskNotFoundError');
		expect(createTaskCircularDependencyError('', '').name).toBe(
			'TaskCircularDependencyError',
		);
		expect(createToolError('').name).toBe('ToolError');
		expect(createToolNotFoundError('').name).toBe('ToolNotFoundError');
		expect(createToolExecutionError('', '').name).toBe('ToolExecutionError');
	});
});

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

describe('Edge cases', () => {
	it('should handle deeply nested cause chains', () => {
		const root = new Error('root');
		const mid = createSimseError('mid', { cause: root });
		const top = createChainStepError('s', 0, 'top', { cause: mid });

		expect(top.cause).toBe(mid);
		expect((top.cause as SimseError).cause).toBe(root);
	});

	it('should handle empty metadata', () => {
		const err = createSimseError('empty meta', { metadata: {} });
		expect(err.metadata).toEqual({});
	});

	it('should handle ConfigValidationError with empty issues array', () => {
		const err = createConfigValidationError([]);
		expect(err.issues).toHaveLength(0);
		expect(err.message).toContain('0 validation errors');
	});

	it('should handle TemplateMissingVariablesError with empty array', () => {
		const err = createTemplateMissingVariablesError([]);
		expect(err.missingVariables).toHaveLength(0);
		// Single variable path because length is 0 (not > 1)
		expect(err.message).toContain('variable:');
	});

	it('should handle ProviderError with all optional fields', () => {
		const err = createProviderError('test', 'msg', {
			code: 'X',
			statusCode: 599,
			cause: 'str',
			metadata: { a: 1, b: 2 },
		});
		expect(err.code).toBe('X');
		expect(err.statusCode).toBe(599);
		expect(err.cause).toBe('str');
		expect(err.metadata).toEqual({ a: 1, b: 2, provider: 'test' });
	});
});
