// ---------------------------------------------------------------------------
// Agent Executor — createAgentExecutor factory
// ---------------------------------------------------------------------------

import { createChainError, createMCPToolError } from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { ACPGenerateResult } from '../acp/types.js';
import { formatSearchResults } from '../chain/format.js';
import type { Provider } from '../chain/types.js';
import type { Library } from '../library/library.js';
import type { MCPClient } from '../mcp/mcp-client.js';
import type { AgentResult, AgentStepConfig } from './types.js';

// ---------------------------------------------------------------------------
// Options + Interface
// ---------------------------------------------------------------------------

export interface AgentExecutorOptions {
	readonly acpClient: ACPClient;
	readonly mcpClient?: MCPClient;
	readonly library?: Library;
	readonly logger?: Logger;
	readonly name?: string;
}

export interface AgentExecutor {
	readonly execute: (
		step: AgentStepConfig,
		provider: Provider,
		prompt: string,
		currentValues: Record<string, string>,
		chainName?: string,
	) => Promise<AgentResult>;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createAgentExecutor(
	options: AgentExecutorOptions,
): AgentExecutor {
	const { acpClient, mcpClient, library } = options;
	const logger = (options.logger ?? getDefaultLogger()).child('agent-executor');

	async function executeACPStep(
		step: AgentStepConfig,
		prompt: string,
	): Promise<AgentResult> {
		const res: ACPGenerateResult = await acpClient.generate(prompt, {
			agentId: step.agentId,
			serverName: step.serverName,
			systemPrompt: step.systemPrompt,
			config: step.agentConfig,
		});
		return {
			output: res.content,
			model: `acp:${res.agentId}`,
			usage: res.usage,
		};
	}

	async function executeMCPStep(
		step: AgentStepConfig,
		prompt: string,
		currentValues: Record<string, string>,
		chainName?: string,
	): Promise<AgentResult> {
		if (!mcpClient) {
			throw createChainError(
				`MCP client is not configured but step "${step.name}" requires it`,
				{ code: 'CHAIN_MCP_NOT_CONFIGURED', chainName },
			);
		}

		if (!step.mcpServerName || !step.mcpToolName) {
			throw createChainError(
				`MCP step "${step.name}" requires both mcpServerName and mcpToolName`,
				{ code: 'CHAIN_INVALID_STEP', chainName },
			);
		}

		const toolArgs: Record<string, unknown> = {};
		if (step.mcpArguments) {
			for (const [argName, sourceKey] of Object.entries(step.mcpArguments)) {
				toolArgs[argName] = currentValues[sourceKey];
			}
		} else {
			toolArgs.prompt = prompt;
		}

		const result = await mcpClient.callTool(
			step.mcpServerName,
			step.mcpToolName,
			toolArgs,
		);

		if (result.isError) {
			throw createMCPToolError(
				step.mcpServerName,
				step.mcpToolName,
				`Tool returned an error: ${result.content}`,
			);
		}

		return {
			output: result.content,
			model: `mcp:${step.mcpServerName}/${step.mcpToolName}`,
			toolMetrics: result.metrics,
		};
	}

	async function executeMemoryStep(
		step: AgentStepConfig,
		prompt: string,
		chainName?: string,
	): Promise<AgentResult> {
		if (!library) {
			throw createChainError(
				`Library is not configured but step "${step.name}" requires it`,
				{ code: 'CHAIN_MEMORY_NOT_CONFIGURED', chainName },
			);
		}

		const results = await library.search(prompt);
		return {
			output: formatSearchResults(results),
			model: 'library:search',
		};
	}

	const execute = async (
		step: AgentStepConfig,
		provider: Provider,
		prompt: string,
		currentValues: Record<string, string>,
		chainName?: string,
	): Promise<AgentResult> => {
		logger.debug(`Executing ${provider} step "${step.name}"`);

		switch (provider) {
			case 'acp':
				return executeACPStep(step, prompt);
			case 'mcp':
				return executeMCPStep(step, prompt, currentValues, chainName);
			case 'memory':
				return executeMemoryStep(step, prompt, chainName);
			default: {
				const _exhaustive: never = provider;
				throw createChainError(`Unknown provider: ${_exhaustive}`, {
					code: 'CHAIN_UNKNOWN_PROVIDER',
					chainName,
				});
			}
		}
	};

	return Object.freeze({ execute });
}
