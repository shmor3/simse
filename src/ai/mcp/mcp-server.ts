import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import type { ACPClient } from '../acp/acp-client.js';
import { createChain } from '../chain/chain.js';
import { createPromptTemplate } from '../chain/prompt-template.js';
import type { MCPServerConfig } from './types.js';

// ---------------------------------------------------------------------------
// SimseMCPServer interface
// ---------------------------------------------------------------------------

export interface SimseMCPServer {
	readonly start: () => Promise<void>;
	readonly stop: () => Promise<void>;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createMCPServer(
	config: MCPServerConfig,
	acpClient: ACPClient,
): SimseMCPServer {
	const server = new McpServer({
		name: config.name,
		version: config.version,
	});

	// -----------------------------------------------------------------------
	// Tools
	// -----------------------------------------------------------------------

	const registerTools = (): void => {
		// Tool: generate — single LLM prompt via ACP
		server.registerTool(
			'generate',
			{
				title: 'Generate Text',
				description: 'Generate text from a prompt using an ACP agent',
				inputSchema: {
					prompt: z.string().describe('The prompt to send to the agent'),
					agentId: z
						.string()
						.optional()
						.describe('ACP agent ID to use (defaults to configured default)'),
					serverName: z
						.string()
						.optional()
						.describe(
							'ACP server name to use (defaults to configured default)',
						),
					systemPrompt: z.string().optional().describe('System prompt'),
				},
			},
			async ({ prompt, agentId, serverName, systemPrompt }) => {
				try {
					const res = await acpClient.generate(prompt, {
						agentId,
						serverName,
						systemPrompt,
					});

					return { content: [{ type: 'text' as const, text: res.content }] };
				} catch (error) {
					const message =
						error instanceof Error ? error.message : String(error);
					return {
						content: [{ type: 'text' as const, text: `Error: ${message}` }],
						isError: true,
					};
				}
			},
		);

		// Tool: run-chain — execute a multi-step chain
		server.registerTool(
			'run-chain',
			{
				title: 'Run Chain',
				description:
					'Execute a multi-step LangChain pipeline. Provide steps as a JSON array and initial values as a JSON object.',
				inputSchema: {
					steps: z
						.string()
						.describe(
							'JSON array of step objects: [{"name":"step1","template":"...","agentId":"my-agent"}]',
						),
					values: z
						.string()
						.describe('JSON object of initial template variable values'),
				},
			},
			async ({ steps, values }) => {
				let parsedSteps: Array<{
					name: string;
					template: string;
					agentId?: string;
					serverName?: string;
					systemPrompt?: string;
				}>;
				let parsedValues: Record<string, string>;

				try {
					parsedSteps = JSON.parse(steps);
				} catch {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "steps" is not valid JSON',
							},
						],
						isError: true,
					};
				}

				if (
					!Array.isArray(parsedSteps) ||
					!parsedSteps.every(
						(s) =>
							typeof s === 'object' &&
							s !== null &&
							typeof s.name === 'string' &&
							typeof s.template === 'string',
					)
				) {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "steps" must be a JSON array of objects with "name" and "template" string fields',
							},
						],
						isError: true,
					};
				}

				try {
					parsedValues = JSON.parse(values);
				} catch {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "values" is not valid JSON',
							},
						],
						isError: true,
					};
				}

				if (
					typeof parsedValues !== 'object' ||
					parsedValues === null ||
					Array.isArray(parsedValues)
				) {
					return {
						content: [
							{
								type: 'text' as const,
								text: 'Error: "values" must be a JSON object',
							},
						],
						isError: true,
					};
				}

				const nonStringKeys = Object.entries(parsedValues)
					.filter(([, v]) => typeof v !== 'string')
					.map(([k]) => k);
				if (nonStringKeys.length > 0) {
					return {
						content: [
							{
								type: 'text' as const,
								text: `Error: "values" entries must all be strings. Non-string keys: ${nonStringKeys.join(', ')}`,
							},
						],
						isError: true,
					};
				}

				try {
					const chain = createChain({ acpClient });

					for (const s of parsedSteps) {
						chain.addStep({
							name: s.name,
							template: createPromptTemplate(s.template),
							agentId: s.agentId,
							serverName: s.serverName,
							systemPrompt: s.systemPrompt,
						});
					}

					const results = await chain.run(parsedValues);
					const formatted = results
						.map(
							(r) =>
								`[${r.stepName}] (${r.provider}/${r.model}, ${r.durationMs}ms)\n${r.output}`,
						)
						.join('\n\n');

					return { content: [{ type: 'text' as const, text: formatted }] };
				} catch (error) {
					const message =
						error instanceof Error ? error.message : String(error);
					return {
						content: [{ type: 'text' as const, text: `Error: ${message}` }],
						isError: true,
					};
				}
			},
		);

		// Tool: list-agents — check which ACP agents are available
		server.registerTool(
			'list-agents',
			{
				title: 'List Agents',
				description: 'List available ACP agents across configured servers',
				inputSchema: {
					serverName: z
						.string()
						.optional()
						.describe(
							'Specific ACP server to query (defaults to all configured)',
						),
				},
			},
			async ({ serverName }) => {
				const lines: string[] = [];

				if (serverName) {
					const available = await acpClient.isAvailable(serverName);
					lines.push(
						`${serverName}: ${available ? 'available' : 'unavailable'}`,
					);
					if (available) {
						try {
							const agents = await acpClient.listAgents(serverName);
							for (const agent of agents) {
								lines.push(
									`  - ${agent.id}${agent.name ? ` (${agent.name})` : ''}${agent.description ? `: ${agent.description}` : ''}`,
								);
							}
							if (agents.length === 0) {
								lines.push('  (no agents found)');
							}
						} catch {
							lines.push('  (could not list agents)');
						}
					}
				} else {
					for (const name of acpClient.serverNames) {
						const available = await acpClient.isAvailable(name);
						lines.push(`${name}: ${available ? 'available' : 'unavailable'}`);
						if (available) {
							try {
								const agents = await acpClient.listAgents(name);
								for (const agent of agents) {
									lines.push(
										`  - ${agent.id}${agent.name ? ` (${agent.name})` : ''}${agent.description ? `: ${agent.description}` : ''}`,
									);
								}
								if (agents.length === 0) {
									lines.push('  (no agents found)');
								}
							} catch {
								lines.push('  (could not list agents)');
							}
						}
					}
				}

				if (lines.length === 0) {
					lines.push('No ACP servers configured.');
				}

				return {
					content: [{ type: 'text' as const, text: lines.join('\n') }],
				};
			},
		);
	};

	// -----------------------------------------------------------------------
	// Resources
	// -----------------------------------------------------------------------

	const registerResources = (): void => {
		server.registerResource(
			'acp-agents',
			'agents://acp',
			{
				description: 'List of available ACP agents across all servers',
				mimeType: 'application/json',
			},
			async (uri) => {
				const allAgents: Array<{
					server: string;
					id: string;
					name?: string;
					description?: string;
				}> = [];

				for (const serverName of acpClient.serverNames) {
					try {
						const agents = await acpClient.listAgents(serverName);
						for (const agent of agents) {
							allAgents.push({
								server: serverName,
								id: agent.id,
								name: agent.name,
								description: agent.description,
							});
						}
					} catch {
						// Server not available, skip
					}
				}

				return {
					contents: [
						{
							uri: uri.href,
							text: JSON.stringify(allAgents, null, 2),
						},
					],
				};
			},
		);
	};

	// -----------------------------------------------------------------------
	// Prompts
	// -----------------------------------------------------------------------

	const registerPrompts = (): void => {
		server.registerPrompt(
			'single-prompt',
			{
				title: 'Single Prompt',
				description: 'A reusable prompt template with {variable} substitution',
				argsSchema: {
					template: z
						.string()
						.describe(
							'Prompt template with {variable} placeholders, e.g. "Translate {text} to {language}"',
						),
					variables: z
						.string()
						.describe(
							'JSON object of variable values, e.g. {"text":"hello","language":"French"}',
						),
				},
			},
			({ template, variables }) => {
				let parsedVars: Record<string, string>;
				try {
					parsedVars = JSON.parse(variables);
				} catch {
					return {
						messages: [
							{
								role: 'user' as const,
								content: {
									type: 'text' as const,
									text: 'Error: "variables" is not valid JSON',
								},
							},
						],
					};
				}

				let formatted: string;
				try {
					const pt = createPromptTemplate(template);
					formatted = pt.format(parsedVars);
				} catch (error) {
					const message =
						error instanceof Error ? error.message : String(error);
					return {
						messages: [
							{
								role: 'user' as const,
								content: {
									type: 'text' as const,
									text: `Error: ${message}`,
								},
							},
						],
					};
				}
				return {
					messages: [
						{
							role: 'user' as const,
							content: { type: 'text' as const, text: formatted },
						},
					],
				};
			},
		);
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	let started = false;
	let startPromise: Promise<void> | null = null;

	const start = async (): Promise<void> => {
		if (started) return;
		if (startPromise) return startPromise;

		startPromise = (async () => {
			registerTools();
			registerResources();
			registerPrompts();

			const transport = new StdioServerTransport();
			await server.connect(transport);
			started = true;
		})().finally(() => {
			startPromise = null;
		});

		return startPromise;
	};

	const stop = async (): Promise<void> => {
		await server.close();
	};

	return { start, stop };
}
