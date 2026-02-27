// ---------------------------------------------------------------------------
// Cross-ACP Delegation Tool Registration
//
// Registers a delegation tool per non-primary ACP server so one ACP's model
// can invoke another. Follows the subagent-tools.ts pattern exactly.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { ToolDefinition, ToolHandler, ToolRegistry } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface DelegationCallbacks {
	readonly onDelegationStart?: (info: DelegationInfo) => void;
	readonly onDelegationComplete?: (
		id: string,
		result: DelegationResult,
	) => void;
	readonly onDelegationError?: (id: string, error: Error) => void;
}

export interface DelegationInfo {
	readonly id: string;
	readonly serverName: string;
	readonly task: string;
}

export interface DelegationResult {
	readonly text: string;
	readonly serverName: string;
	readonly durationMs: number;
}

export interface DelegationToolsOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly primaryServer?: string;
	readonly delegationMaxTurns?: number;
	readonly callbacks?: DelegationCallbacks;
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

let delegationCounter = 0;

const nextDelegationId = (): string => `del_${++delegationCounter}`;

/** Reset counter — exposed for tests only. */
export const _resetDelegationCounter = (): void => {
	delegationCounter = 0;
};

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/**
 * Register a delegation tool for each non-primary ACP server.
 * This lets one ACP server's model invoke another via single-shot generation.
 */
export function registerDelegationTools(
	registry: ToolRegistry,
	options: DelegationToolsOptions,
): void {
	const { acpClient, primaryServer, callbacks } = options;

	for (const serverName of acpClient.serverNames) {
		if (serverName === primaryServer) continue;

		// Sanitize server name to valid tool name (alphanumeric + underscore)
		const safeName = serverName.replace(/[^a-zA-Z0-9]/g, '_');

		registerTool(
			registry,
			{
				name: `delegate_${safeName}`,
				description: `Delegate a task to the "${serverName}" ACP server for a single-shot response. Use this to get a response from a different AI model/server.`,
				parameters: {
					task: {
						type: 'string',
						description: 'The task/prompt to send to the server',
						required: true,
					},
					systemPrompt: {
						type: 'string',
						description: 'Optional system prompt for the delegation',
					},
				},
				category: 'subagent',
			},
			async (args) => {
				const id = nextDelegationId();
				const task = String(args.task ?? '');
				const systemPrompt =
					typeof args.systemPrompt === 'string' ? args.systemPrompt : undefined;

				const info: DelegationInfo = Object.freeze({
					id,
					serverName,
					task,
				});

				callbacks?.onDelegationStart?.(info);

				try {
					const start = Date.now();
					const result = await acpClient.generate(task, {
						serverName,
						systemPrompt,
					});

					const delegationResult: DelegationResult = Object.freeze({
						text: result.content,
						serverName,
						durationMs: Date.now() - start,
					});

					callbacks?.onDelegationComplete?.(id, delegationResult);
					return result.content;
				} catch (err) {
					const error = toError(err);
					callbacks?.onDelegationError?.(id, error);
					throw error;
				}
			},
		);
	}
}
