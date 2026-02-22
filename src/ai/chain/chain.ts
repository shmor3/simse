// ---------------------------------------------------------------------------
// Chain — createChain factory, createChainFromDefinition, runNamedChain
// ---------------------------------------------------------------------------

import type { AppConfig, ChainDefinition } from '../../config/settings.js';
import {
	createChainError,
	createChainNotFoundError,
	createChainStepError,
	createMCPToolError,
	isChainError,
	isChainStepError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { ACPGenerateResult } from '../acp/types.js';
import type { MCPClient } from '../mcp/mcp-client.js';
import type { MemoryManager } from '../memory/memory.js';
import { formatSearchResults } from './format.js';
import { createPromptTemplate } from './prompt-template.js';
import type {
	ChainCallbacks,
	ChainStepConfig,
	Provider,
	StepResult,
} from './types.js';

// ---------------------------------------------------------------------------
// Chain interface
// ---------------------------------------------------------------------------

export interface ChainOptions {
	acpClient: ACPClient;
	mcpClient?: MCPClient;
	memoryManager?: MemoryManager;
	logger?: Logger;
	callbacks?: ChainCallbacks;
	chainName?: string;
}

export interface Chain {
	/** Append a step to the chain. Returns the chain for fluent chaining. */
	readonly addStep: (step: ChainStepConfig) => Chain;
	/** Set chain-level callbacks. Returns the chain for fluent chaining. */
	readonly setCallbacks: (callbacks: ChainCallbacks) => Chain;
	/** Clear all steps from the chain. */
	readonly clear: () => void;
	/** Execute all steps sequentially. */
	readonly run: (
		initialValues: Record<string, string>,
	) => Promise<StepResult[]>;
	/**
	 * Convenience: build and run a single-step chain.
	 */
	readonly runSingle: (
		templateStr: string,
		values: Record<string, string>,
		options?: {
			provider?: Provider;
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
		},
	) => Promise<StepResult>;
	/** Return the number of steps in the chain. */
	readonly length: number;
	/** Return the step configs (read-only copy). */
	readonly stepConfigs: readonly ChainStepConfig[];
}

// ---------------------------------------------------------------------------
// createChain factory
// ---------------------------------------------------------------------------

export function createChain(options: ChainOptions): Chain {
	const {
		acpClient,
		mcpClient,
		memoryManager,
		callbacks: initialCallbacks,
		chainName,
	} = options;

	const logger = (options.logger ?? getDefaultLogger()).child('chain');
	const defaultProvider: Provider = 'acp';
	let steps: ChainStepConfig[] = [];
	let callbacks: ChainCallbacks | undefined = initialCallbacks;

	// -----------------------------------------------------------------------
	// Internal step execution
	// -----------------------------------------------------------------------

	async function executeACPStep(
		step: ChainStepConfig,
		prompt: string,
	): Promise<{ output: string; model: string }> {
		const res: ACPGenerateResult = await acpClient.generate(prompt, {
			agentId: step.agentId,
			serverName: step.serverName,
			systemPrompt: step.systemPrompt,
			config: step.agentConfig,
		});
		return { output: res.content, model: `acp:${res.agentId}` };
	}

	async function executeMCPStep(
		step: ChainStepConfig,
		prompt: string,
		currentValues: Record<string, string>,
	): Promise<{ output: string; model: string }> {
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

		// Build tool arguments from mcpArguments mapping or fallback to prompt
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
		};
	}

	async function executeMemoryStep(
		step: ChainStepConfig,
		prompt: string,
	): Promise<{ output: string; model: string }> {
		if (!memoryManager) {
			throw createChainError(
				`Memory manager is not configured but step "${step.name}" requires it`,
				{ code: 'CHAIN_MEMORY_NOT_CONFIGURED', chainName },
			);
		}

		const results = await memoryManager.search(prompt);
		return {
			output: formatSearchResults(results),
			model: 'memory:vector-search',
		};
	}

	async function executeStep(
		step: ChainStepConfig,
		provider: Provider,
		prompt: string,
		currentValues: Record<string, string>,
	): Promise<{ output: string; model: string }> {
		switch (provider) {
			case 'acp':
				return executeACPStep(step, prompt);
			case 'mcp':
				return executeMCPStep(step, prompt, currentValues);
			case 'memory':
				return executeMemoryStep(step, prompt);
			default: {
				const _exhaustive: never = provider;
				throw createChainError(`Unknown provider: ${_exhaustive}`, {
					code: 'CHAIN_UNKNOWN_PROVIDER',
					chainName,
				});
			}
		}
	}

	// -----------------------------------------------------------------------
	// Chain object
	// -----------------------------------------------------------------------

	const chain: Chain = {
		addStep(step: ChainStepConfig): Chain {
			// Validate MCP steps eagerly
			if (step.provider === 'mcp') {
				if (!step.mcpServerName || !step.mcpToolName) {
					throw createChainError(
						`MCP step "${step.name}" requires both mcpServerName and mcpToolName`,
						{
							code: 'CHAIN_INVALID_STEP',
							chainName,
							metadata: {
								stepName: step.name,
								mcpServerName: step.mcpServerName,
								mcpToolName: step.mcpToolName,
							},
						},
					);
				}
			}

			steps.push(step);
			return chain;
		},

		setCallbacks(newCallbacks: ChainCallbacks): Chain {
			callbacks = newCallbacks;
			return chain;
		},

		clear(): void {
			steps = [];
		},

		async run(initialValues: Record<string, string>): Promise<StepResult[]> {
			if (steps.length === 0) {
				throw createChainError('Cannot run an empty chain — add steps first', {
					code: 'CHAIN_EMPTY',
					chainName,
				});
			}

			const runSteps = [...steps];

			logger.info(
				`Running chain${chainName ? ` "${chainName}"` : ''} with ${runSteps.length} step(s)`,
				{ initialValueKeys: Object.keys(initialValues) },
			);

			const results: StepResult[] = [];
			const currentValues = { ...initialValues };

			try {
				for (let stepIndex = 0; stepIndex < runSteps.length; stepIndex++) {
					const step = runSteps[stepIndex];
					const start = Date.now();
					const provider = step.provider ?? defaultProvider;

					// Apply input mappings
					if (step.inputMapping) {
						for (const [templateVar, sourceKey] of Object.entries(
							step.inputMapping,
						)) {
							if (sourceKey in currentValues) {
								currentValues[templateVar] = currentValues[sourceKey];
							}
						}
					}

					// Resolve the prompt
					let prompt: string;
					try {
						prompt = step.template.format(currentValues);
					} catch (error) {
						const stepError = createChainStepError(
							step.name,
							stepIndex,
							`Template resolution failed: ${toError(error).message}`,
							{ chainName, cause: error },
						);

						try {
							await callbacks?.onStepError?.({
								stepName: step.name,
								stepIndex,
								error: stepError,
							});
						} catch (cbError) {
							logger.warn('onStepError callback threw', {
								error: toError(cbError).message,
							});
						}

						throw stepError;
					}

					// Fire onStepStart callback
					try {
						await callbacks?.onStepStart?.({
							stepName: step.name,
							stepIndex,
							totalSteps: runSteps.length,
							provider,
							prompt,
						});
					} catch (cbError) {
						logger.warn('onStepStart callback threw', {
							error: toError(cbError).message,
						});
					}

					logger.debug(
						`Step ${stepIndex + 1}/${runSteps.length}: "${step.name}" [${provider}]`,
						{ promptLength: prompt.length },
					);

					// Execute the step
					let rawOutput: string;
					let model: string;

					try {
						const result = await executeStep(
							step,
							provider,
							prompt,
							currentValues,
						);
						rawOutput = result.output;
						model = result.model;
					} catch (error) {
						const stepError = isChainStepError(error)
							? error
							: createChainStepError(
									step.name,
									stepIndex,
									`Provider "${provider}" failed: ${toError(error).message}`,
									{
										chainName,
										cause: error,
										metadata: { provider },
									},
								);

						try {
							await callbacks?.onStepError?.({
								stepName: step.name,
								stepIndex,
								error: stepError,
							});
						} catch (cbError) {
							logger.warn('onStepError callback threw', {
								error: toError(cbError).message,
							});
						}

						throw stepError;
					}

					// Apply output transform
					let output: string;
					try {
						output = step.outputTransform
							? step.outputTransform(rawOutput)
							: rawOutput;
					} catch (error) {
						const stepError = createChainStepError(
							step.name,
							stepIndex,
							`Output transform failed: ${toError(error).message}`,
							{ chainName, cause: error },
						);

						try {
							await callbacks?.onStepError?.({
								stepName: step.name,
								stepIndex,
								error: stepError,
							});
						} catch (cbError) {
							logger.warn('onStepError callback threw', {
								error: toError(cbError).message,
							});
						}

						throw stepError;
					}

					// Optionally store output to memory
					if (step.storeToMemory && memoryManager) {
						try {
							await memoryManager.add(output, step.memoryMetadata ?? {});
							logger.debug(`Stored step "${step.name}" output to memory`);
						} catch (error) {
							// Non-fatal: log but don't fail the chain
							logger.warn(
								`Failed to store step "${step.name}" output to memory`,
								{ error: toError(error).message },
							);
						}
					}

					const durationMs = Date.now() - start;

					const stepResult: StepResult = {
						stepName: step.name,
						provider,
						model,
						input: prompt,
						output,
						durationMs,
						stepIndex,
					};

					results.push(stepResult);

					// Fire onStepComplete callback
					try {
						await callbacks?.onStepComplete?.(stepResult);
					} catch (cbError) {
						logger.warn('onStepComplete callback threw', {
							error: toError(cbError).message,
						});
					}

					logger.info(`Step "${step.name}" completed in ${durationMs}ms`, {
						provider,
						model,
						outputLength: output.length,
					});

					// Make the output available to subsequent steps
					currentValues[step.name] = output;
					currentValues.previous_output = output;
				}

				// Fire onChainComplete callback
				try {
					await callbacks?.onChainComplete?.(results);
				} catch (cbError) {
					logger.warn('onChainComplete callback threw', {
						error: toError(cbError).message,
					});
				}

				logger.info(
					`Chain${chainName ? ` "${chainName}"` : ''} completed successfully`,
					{
						totalSteps: results.length,
						totalDurationMs: results.reduce((sum, r) => sum + r.durationMs, 0),
					},
				);

				return results;
			} catch (error) {
				// Fire onChainError callback
				try {
					await callbacks?.onChainError?.({
						error: toError(error),
						completedSteps: results,
					});
				} catch (cbError) {
					logger.warn('onChainError callback threw', {
						error: toError(cbError).message,
					});
				}

				// Re-throw ChainStepErrors directly; wrap anything else
				if (isChainStepError(error) || isChainError(error)) {
					throw error;
				}

				throw createChainError(
					`Chain execution failed: ${toError(error).message}`,
					{
						chainName,
						cause: error,
						metadata: { completedSteps: results.length },
					},
				);
			}
		},

		async runSingle(
			templateStr: string,
			values: Record<string, string>,
			singleOptions?: {
				provider?: Provider;
				agentId?: string;
				serverName?: string;
				systemPrompt?: string;
			},
		): Promise<StepResult> {
			const singleChain = createChain({
				acpClient,
				mcpClient,
				memoryManager,
				logger,
				callbacks,
			});

			singleChain.addStep({
				name: 'single',
				template: createPromptTemplate(templateStr),
				provider: singleOptions?.provider,
				agentId: singleOptions?.agentId,
				serverName: singleOptions?.serverName,
				systemPrompt: singleOptions?.systemPrompt,
			});

			const results = await singleChain.run(values);
			return results[0];
		},

		get length(): number {
			return steps.length;
		},

		get stepConfigs(): readonly ChainStepConfig[] {
			return [...steps];
		},
	};

	return Object.freeze(chain);
}

// ---------------------------------------------------------------------------
// createChainFromDefinition
// ---------------------------------------------------------------------------

/**
 * Build a chain from a JSON chain definition.
 * Each step's template string is converted to a PromptTemplate instance.
 */
export function createChainFromDefinition(
	definition: ChainDefinition,
	options: ChainOptions,
): Chain {
	const chain = createChain(options);

	if (definition.steps.length === 0) {
		throw createChainError('Chain definition has no steps', {
			code: 'CHAIN_EMPTY',
			chainName: options.chainName,
		});
	}

	for (const step of definition.steps) {
		chain.addStep({
			name: step.name,
			template: createPromptTemplate(step.template),
			provider: step.provider,
			agentId: step.agentId ?? definition.agentId,
			serverName: step.serverName ?? definition.serverName,
			agentConfig: step.agentConfig,
			systemPrompt: step.systemPrompt,
			inputMapping: step.inputMapping,
			mcpServerName: step.mcpServerName,
			mcpToolName: step.mcpToolName,
			mcpArguments: step.mcpArguments,
			storeToMemory: step.storeToMemory,
			memoryMetadata: step.memoryMetadata,
		});
	}

	return chain;
}

// ---------------------------------------------------------------------------
// runNamedChain
// ---------------------------------------------------------------------------

/**
 * Build and run a chain from a named definition in the app config.
 *
 * @throws {ChainNotFoundError} if the chain name is not defined in the config.
 */
export async function runNamedChain(
	chainName: string,
	config: AppConfig,
	options: ChainOptions & { overrideValues?: Record<string, string> },
): Promise<StepResult[]> {
	const definition = config.chains[chainName];
	if (!definition) {
		throw createChainNotFoundError(chainName);
	}

	const chain = createChainFromDefinition(definition, {
		...options,
		chainName,
	});

	const initialValues = {
		...(definition.initialValues ?? {}),
		...(options.overrideValues ?? {}),
	};

	return chain.run(initialValues);
}
