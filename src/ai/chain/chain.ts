// ---------------------------------------------------------------------------
// Chain — createChain factory, createChainFromDefinition, runNamedChain
// ---------------------------------------------------------------------------

import type { Library } from '../library/library.js';
import type { AppConfig, ChainDefinition } from '../../config/settings.js';
import {
	createChainError,
	createChainNotFoundError,
	createChainStepError,
	isChainError,
	isChainStepError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { ACPClient } from '../acp/acp-client.js';
import type { ACPTokenUsage } from '../acp/types.js';
import type { AgentExecutor } from '../agent/agent-executor.js';
import { createAgentExecutor } from '../agent/agent-executor.js';
import type { ParallelSubResult } from '../agent/types.js';
import type { MCPClient } from '../mcp/mcp-client.js';
import type { MCPToolCallMetrics } from '../mcp/types.js';
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
	readonly acpClient: ACPClient;
	readonly mcpClient?: MCPClient;
	readonly library?: Library;
	readonly logger?: Logger;
	readonly callbacks?: ChainCallbacks;
	readonly chainName?: string;
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
		library,
		callbacks: initialCallbacks,
		chainName,
	} = options;

	const logger = (options.logger ?? getDefaultLogger()).child('chain');
	const defaultProvider: Provider = 'acp';
	let steps: ChainStepConfig[] = [];
	let callbacks: ChainCallbacks | undefined = initialCallbacks;

	const executor = createAgentExecutor({
		acpClient,
		mcpClient,
		library,
		logger,
		name: chainName,
	});

	// -----------------------------------------------------------------------
	// Parallel step execution
	// -----------------------------------------------------------------------

	async function runParallelStep(
		step: ChainStepConfig,
		stepIndex: number,
		totalSteps: number,
		currentValues: Record<string, string>,
		currentExecutor: AgentExecutor,
		currentCallbacks: ChainCallbacks | undefined,
	): Promise<StepResult> {
		if (!step.parallel) {
			throw createChainError(
				`Parallel step "${step.name}" has no parallel config`,
				{ code: 'CHAIN_INVALID_STEP', chainName },
			);
		}
		const {
			subSteps,
			mergeStrategy = 'concat',
			failTolerant = false,
			concatSeparator = '\n\n',
		} = step.parallel;
		const start = Date.now();

		// Fire onStepStart for the parent parallel step
		try {
			await currentCallbacks?.onStepStart?.({
				stepName: step.name,
				stepIndex,
				totalSteps,
				provider: 'acp',
				prompt: `[parallel: ${subSteps.length} sub-steps]`,
			});
		} catch (cbError) {
			logger.warn('onStepStart callback threw', {
				error: toError(cbError).message,
			});
		}

		logger.debug(
			`Step ${stepIndex + 1}/${totalSteps}: "${step.name}" [parallel: ${subSteps.length} sub-steps]`,
		);

		// Resolve all sub-step templates before fanning out
		const resolvedSubSteps: Array<{
			config: (typeof subSteps)[number];
			prompt: string;
		}> = [];
		for (const subStep of subSteps) {
			let prompt: string;
			try {
				prompt = subStep.template.format(currentValues);
			} catch (error) {
				const stepError = createChainStepError(
					`${step.name}.${subStep.name}`,
					stepIndex,
					`Template resolution failed for sub-step: ${toError(error).message}`,
					{ chainName, cause: error },
				);
				try {
					await currentCallbacks?.onStepError?.({
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
			resolvedSubSteps.push({ config: subStep, prompt });
		}

		// Execute sub-steps concurrently
		const subResultPromises = resolvedSubSteps.map(
			async ({ config: subStep, prompt }) => {
				const subStart = Date.now();
				const provider: Provider = subStep.provider ?? defaultProvider;

				// Fire onStepStart for the sub-step
				try {
					await currentCallbacks?.onStepStart?.({
						stepName: `${step.name}.${subStep.name}`,
						stepIndex,
						totalSteps,
						provider,
						prompt,
					});
				} catch (cbError) {
					logger.warn('onStepStart callback threw for sub-step', {
						error: toError(cbError).message,
					});
				}

				const agentResult = await currentExecutor.execute(
					{
						name: `${step.name}.${subStep.name}`,
						agentId: subStep.agentId,
						serverName: subStep.serverName,
						agentConfig: subStep.agentConfig,
						systemPrompt: subStep.systemPrompt,
						mcpServerName: subStep.mcpServerName,
						mcpToolName: subStep.mcpToolName,
						mcpArguments: subStep.mcpArguments,
					},
					provider,
					prompt,
					currentValues,
					chainName,
				);

				const rawOutput = agentResult.output;
				const output = subStep.outputTransform
					? subStep.outputTransform(rawOutput)
					: rawOutput;
				const durationMs = Date.now() - subStart;

				const subResult: ParallelSubResult = {
					subStepName: subStep.name,
					provider,
					model: agentResult.model,
					input: prompt,
					output,
					durationMs,
					usage: agentResult.usage,
					toolMetrics: agentResult.toolMetrics,
				};

				// Fire onStepComplete for the sub-step
				try {
					await currentCallbacks?.onStepComplete?.({
						stepName: `${step.name}.${subStep.name}`,
						provider,
						model: agentResult.model,
						input: prompt,
						output,
						durationMs,
						stepIndex,
						usage: agentResult.usage,
						toolMetrics: agentResult.toolMetrics,
					});
				} catch (cbError) {
					logger.warn('onStepComplete callback threw for sub-step', {
						error: toError(cbError).message,
					});
				}

				return subResult;
			},
		);

		// Fan out
		let settledSubResults: ParallelSubResult[];

		if (failTolerant) {
			const settled = await Promise.allSettled(subResultPromises);
			settledSubResults = settled
				.filter(
					(r): r is PromiseFulfilledResult<ParallelSubResult> =>
						r.status === 'fulfilled',
				)
				.map((r) => r.value);

			if (settledSubResults.length === 0) {
				const stepError = createChainStepError(
					step.name,
					stepIndex,
					'All parallel sub-steps failed',
					{ chainName },
				);
				try {
					await currentCallbacks?.onStepError?.({
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
		} else {
			try {
				settledSubResults = await Promise.all(subResultPromises);
			} catch (error) {
				const stepError = isChainStepError(error)
					? error
					: createChainStepError(
							step.name,
							stepIndex,
							`Parallel sub-step failed: ${toError(error).message}`,
							{ chainName, cause: error },
						);
				try {
					await currentCallbacks?.onStepError?.({
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
		}

		// Merge sub-results
		let mergedOutput: string;
		if (typeof mergeStrategy === 'function') {
			mergedOutput = mergeStrategy(settledSubResults);
		} else {
			mergedOutput = settledSubResults
				.map((r) => r.output)
				.join(concatSeparator);
		}

		const durationMs = Date.now() - start;

		const stepResult: StepResult = {
			stepName: step.name,
			provider: 'acp',
			model: `parallel:${settledSubResults.length}`,
			input: `[parallel: ${subSteps.length} sub-steps]`,
			output: mergedOutput,
			durationMs,
			stepIndex,
			subResults: settledSubResults,
		};

		// Fire onStepComplete for the parent step
		try {
			await currentCallbacks?.onStepComplete?.(stepResult);
		} catch (cbError) {
			logger.warn('onStepComplete callback threw', {
				error: toError(cbError).message,
			});
		}

		logger.info(`Parallel step "${step.name}" completed in ${durationMs}ms`, {
			subStepCount: settledSubResults.length,
			outputLength: mergedOutput.length,
		});

		// Populate keyed values for 'keyed' merge strategy
		if (mergeStrategy === 'keyed') {
			for (const sub of settledSubResults) {
				currentValues[`${step.name}.${sub.subStepName}`] = sub.output;
			}
		}

		currentValues[step.name] = mergedOutput;
		currentValues.previous_output = mergedOutput;

		return stepResult;
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

			// Validate parallel steps
			if (step.parallel) {
				if (step.parallel.subSteps.length < 2) {
					throw createChainError(
						`Parallel step "${step.name}" must have at least 2 sub-steps`,
						{ code: 'CHAIN_INVALID_STEP', chainName },
					);
				}
				for (const sub of step.parallel.subSteps) {
					if (sub.provider === 'mcp') {
						if (!sub.mcpServerName || !sub.mcpToolName) {
							throw createChainError(
								`MCP sub-step "${step.name}.${sub.name}" requires both mcpServerName and mcpToolName`,
								{ code: 'CHAIN_INVALID_STEP', chainName },
							);
						}
					}
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

					// Parallel step branch
					if (step.parallel) {
						const parallelResult = await runParallelStep(
							step,
							stepIndex,
							runSteps.length,
							currentValues,
							executor,
							callbacks,
						);
						results.push(parallelResult);
						continue;
					}

					// Sequential step branch
					const start = Date.now();
					const provider = step.provider ?? defaultProvider;

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

					// Execute the step via agent executor
					let rawOutput: string;
					let model: string;
					let stepUsage: ACPTokenUsage | undefined;
					let stepToolMetrics: MCPToolCallMetrics | undefined;

					try {
						const result = await executor.execute(
							step,
							provider,
							prompt,
							currentValues,
							chainName,
						);
						rawOutput = result.output;
						model = result.model;
						stepUsage = result.usage;
						stepToolMetrics = result.toolMetrics;
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
					if (step.storeToMemory && library) {
						try {
							await library.add(output, step.memoryMetadata ?? {});
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
						usage: stepUsage,
						toolMetrics: stepToolMetrics,
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
				library,
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
			parallel: step.parallel
				? {
						mergeStrategy: step.parallel.mergeStrategy,
						failTolerant: step.parallel.failTolerant,
						concatSeparator: step.parallel.concatSeparator,
						subSteps: step.parallel.subSteps.map((sub) => ({
							name: sub.name,
							template: createPromptTemplate(sub.template),
							provider: sub.provider,
							agentId: sub.agentId ?? step.agentId ?? definition.agentId,
							serverName:
								sub.serverName ?? step.serverName ?? definition.serverName,
							agentConfig: sub.agentConfig,
							systemPrompt: sub.systemPrompt,
							mcpServerName: sub.mcpServerName,
							mcpToolName: sub.mcpToolName,
							mcpArguments: sub.mcpArguments,
						})),
					}
				: undefined,
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
