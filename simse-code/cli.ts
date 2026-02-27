#!/usr/bin/env bun
// ---------------------------------------------------------------------------
// SimSE CLI — Interactive command-line application
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import type { Interface as ReadlineInterface } from 'node:readline';
import { createInterface } from 'node:readline';
import {
	type ACPPermissionRequestInfo,
	createACPClient,
	createDefaultValidators,
	createLocalEmbedder,
	createVFSDisk,
	createVirtualFS,
	toError,
	type VFSWriteEvent,
	validateSnapshot,
} from 'simse';
import type { KnowledgeBaseApp } from './app.js';
import { createApp } from './app.js';
import type { AppContext, Command, SessionState } from './app-context.js';
import {
	createBackgroundManager,
	renderBackgroundTasks,
} from './background.js';
import { createCheckpointManager } from './checkpoints.js';
import type { EmbedFileConfig } from './config.js';
import { createCLIConfig } from './config.js';
import { createConversation } from './conversation.js';
import { renderChangeCount } from './diff-display.js';
import {
	formatMentionsAsContext,
	resolveFileMentions,
} from './file-mentions.js';
import { createFileTracker } from './file-tracker.js';
import { createHooksManager, renderHooksList } from './hooks-config.js';
import { detectImages, formatImageIndicator } from './image-input.js';
import { createKeybindingManager } from './keybindings.js';
import { createAgenticLoop } from './loop.js';
import { createPermissionManager } from './permission-manager.js';
import { showPicker } from './picker.js';
import { createPlanMode } from './plan-mode.js';
import { createACPGenerator } from './providers.js';
import {
	createSessionStore,
	formatSessionSummary,
} from './session-persistence.js';
import { runSetup } from './setup.js';
import { createSkillRegistry } from './skills.js';
import { createStatusLine } from './status-line.js';
import { createFileStorageBackend } from './storage.js';
import { createThemeManager } from './themes.js';
import { parseTodoCommand, renderTodoList } from './todo-ui.js';
import { createToolRegistry } from './tool-registry.js';
import {
	createColors,
	createMarkdownRenderer,
	createSpinner,
	createThinkingSpinner,
	renderAgentToolCallActive,
	renderAgentToolCallCompleted,
	renderAgentToolCallFailed,
	renderAssistantMessage,
	renderBanner,
	renderContextGrid,
	renderDetailLine,
	renderError,
	renderHelp,
	renderModeBadge,
	renderServiceStatus,
	renderSkillLoading,
	renderToolCallActive,
	renderToolCallCompleted,
	renderToolCallFailed,
	renderToolResultCollapsed,
	type TermColors,
} from './ui.js';
import { createUsageTracker, renderUsageChart } from './usage-tracker.js';
import { createVerboseState } from './verbose.js';

// ---------------------------------------------------------------------------
// Stream / tool display helpers
// ---------------------------------------------------------------------------

/** Indent continuation lines so streaming text aligns under the ● bullet. */
function writeStreamText(text: string): void {
	process.stdout.write(text.replace(/\n/g, '\n    '));
}

/** Derive a brief summary from tool output (line count, result count, etc.). */
function deriveToolSummary(name: string, output: string): string | undefined {
	const lines = output.split('\n');
	const lineCount = lines.length;
	if (
		name.startsWith('vfs_') ||
		name.startsWith('file_') ||
		name === 'glob' ||
		name === 'grep'
	) {
		return `${lineCount} line${lineCount !== 1 ? 's' : ''}`;
	}
	if (name.includes('search') || name.includes('list')) {
		return `${lineCount} result${lineCount !== 1 ? 's' : ''}`;
	}
	if (
		name.includes('write') ||
		name.includes('create') ||
		name.includes('edit')
	) {
		const bytes = Buffer.byteLength(output, 'utf-8');
		if (bytes < 1024) return `${bytes} bytes`;
		return `${(bytes / 1024).toFixed(1)} KB`;
	}
	return `${lineCount} line${lineCount !== 1 ? 's' : ''}`;
}

// ---------------------------------------------------------------------------
// Embed state — tracks which provider was used for current embeddings
// ---------------------------------------------------------------------------

const EMBED_STATE_FILE = 'embed-state.json';

function readEmbedState(dataDir: string): EmbedFileConfig | undefined {
	const filePath = join(dataDir, EMBED_STATE_FILE);
	if (!existsSync(filePath)) return undefined;
	try {
		return JSON.parse(readFileSync(filePath, 'utf-8')) as EmbedFileConfig;
	} catch {
		return undefined;
	}
}

function writeEmbedState(dataDir: string, config: EmbedFileConfig): void {
	const filePath = join(dataDir, EMBED_STATE_FILE);
	writeFileSync(filePath, `${JSON.stringify(config, null, '\t')}\n`, 'utf-8');
}

function embedConfigChanged(
	current: EmbedFileConfig,
	saved: EmbedFileConfig | undefined,
): boolean {
	if (!saved) return false; // no saved state = first run, nothing to compare
	return (
		(current.embeddingModel ?? '') !== (saved.embeddingModel ?? '') ||
		(current.dtype ?? '') !== (saved.dtype ?? '')
	);
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

interface CLIArgs {
	readonly dataDir: string;
	readonly logLevel?: 'debug' | 'info' | 'warn' | 'error' | 'none';
	readonly bypassPermissions: boolean;
}

function parseArgs(): CLIArgs {
	const args = process.argv.slice(2);
	let dataDir = join(homedir(), '.simse');
	let logLevel: CLIArgs['logLevel'];
	let bypassPermissions = false;

	for (let i = 0; i < args.length; i++) {
		const arg = args[i];
		const next = args[i + 1];

		if (arg === '--data-dir' && next) dataDir = args[++i];
		else if (arg === '--log-level' && next)
			logLevel = args[++i] as CLIArgs['logLevel'];
		else if (
			arg === '--bypass-permissions' ||
			arg === '--dangerously-skip-permissions'
		)
			bypassPermissions = true;
		else if (arg === '--help' || arg === '-h') {
			printUsage();
			process.exit(0);
		}
	}

	return {
		dataDir,
		logLevel,
		bypassPermissions,
	};
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

// Types imported from app-context.ts: AppContext, SessionState, Command, PermissionMode

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatUptime(ms: number): string {
	const secs = Math.floor(ms / 1000);
	if (secs < 60) return `${secs}s`;
	const mins = Math.floor(secs / 60);
	if (mins < 60) return `${mins}m ${secs % 60}s`;
	const hours = Math.floor(mins / 60);
	return `${hours}h ${mins % 60}m`;
}

function formatNote(
	note: { id: string; topic: string; text: string; timestamp: number },
	colors: TermColors,
): string {
	return [
		`  ${colors.bold('ID:')}      ${colors.dim(note.id)}`,
		`  ${colors.bold('Topic:')}   ${colors.magenta(note.topic)}`,
		`  ${colors.bold('Text:')}    ${note.text}`,
		`  ${colors.bold('Created:')} ${colors.dim(new Date(note.timestamp).toISOString())}`,
	].join('\n');
}

function resolveNote(
	app: KnowledgeBaseApp,
	idOrPrefix: string,
): { id: string; topic: string; text: string; timestamp: number } | undefined {
	const exact = app.getNote(idOrPrefix);
	if (exact) return exact;
	return app.getAllNotes().find((n) => n.id.startsWith(idOrPrefix));
}

// ---------------------------------------------------------------------------
// Note commands
// ---------------------------------------------------------------------------

const addCommand: Command = {
	name: 'add',
	usage: '/add <topic> <text>',
	description: 'Add a note to a topic',
	category: 'notes',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		const spaceIdx = rest.indexOf(' ');
		if (spaceIdx === -1 || !rest.slice(spaceIdx + 1).trim())
			return `${colors.yellow('Usage:')} /add <topic> <text>`;
		const topic = rest.slice(0, spaceIdx);
		const text = rest.slice(spaceIdx + 1).trim();
		const id = await ctx.app.addNote(text, topic);
		return `${colors.green('✓')} Added note ${colors.dim(id.slice(0, 8))} to topic ${colors.magenta(`"${topic}"`)}`;
	},
};

const searchCommand: Command = {
	name: 'search',
	aliases: ['s'],
	usage: '/search <query>',
	description: 'Search notes by semantic similarity',
	category: 'notes',
	handler: async (ctx, rest) => {
		const { colors, spinner } = ctx;
		if (!rest) return `${colors.yellow('Usage:')} /search <query>`;
		spinner.start('Searching memory...');
		const results = await ctx.app.search(rest);
		if (results.length === 0) {
			spinner.succeed('No results found.');
			return undefined;
		}
		spinner.succeed(`${results.length} result(s)`);
		return results
			.map(
				(r) =>
					`  ${colors.cyan(r.score.toFixed(3))}  ${colors.magenta(`[${r.note.topic}]`)} ${r.note.text} ${colors.dim(`(${r.note.id.slice(0, 8)})`)}`,
			)
			.join('\n');
	},
};

const recommendCommand: Command = {
	name: 'recommend',
	aliases: ['rec'],
	usage: '/recommend <query>',
	description: 'Get recommendations based on learning profile',
	category: 'notes',
	handler: async (ctx, rest) => {
		const { colors, spinner } = ctx;
		if (!rest) return `${colors.yellow('Usage:')} /recommend <query>`;
		spinner.start('Finding recommendations...');
		const results = await ctx.app.recommend(rest);
		if (results.length === 0) {
			spinner.succeed('No recommendations.');
			return undefined;
		}
		spinner.succeed(`${results.length} recommendation(s)`);
		return results
			.map(
				(r) =>
					`  ${colors.cyan(r.score.toFixed(3))}  ${colors.magenta(`[${r.note.topic}]`)} ${r.note.text} ${colors.dim(`(${r.note.id.slice(0, 8)})`)}`,
			)
			.join('\n');
	},
};

const topicsCommand: Command = {
	name: 'topics',
	usage: '/topics',
	description: 'List all topics',
	category: 'notes',
	handler: (ctx) => {
		const { colors } = ctx;
		const topics = ctx.app.getTopics();
		if (topics.length === 0) return colors.dim('No topics yet.');
		return topics
			.map(
				(t) => `  ${colors.magenta(t.topic)} ${colors.dim(`(${t.noteCount})`)}`,
			)
			.join('\n');
	},
};

const notesCommand: Command = {
	name: 'notes',
	aliases: ['ls'],
	usage: '/notes [topic]',
	description: 'List notes, optionally filtered by topic',
	category: 'notes',
	handler: (ctx, rest) => {
		const { colors } = ctx;
		const notes = rest ? ctx.app.getNotesByTopic(rest) : ctx.app.getAllNotes();
		if (notes.length === 0) return colors.dim('No notes.');
		return notes
			.map(
				(n) =>
					`  ${colors.magenta(`[${n.topic}]`)} ${n.text} ${colors.dim(`(${n.id.slice(0, 8)})`)}`,
			)
			.join('\n');
	},
};

const getCommand: Command = {
	name: 'get',
	usage: '/get <id>',
	description: 'Get a note by ID or prefix',
	category: 'notes',
	handler: (ctx, rest) => {
		const { colors } = ctx;
		if (!rest) return `${colors.yellow('Usage:')} /get <id>`;
		const note = resolveNote(ctx.app, rest);
		if (!note) return `${colors.red('✗')} Note "${rest}" not found.`;
		return formatNote(note, colors);
	},
};

const deleteCommand: Command = {
	name: 'delete',
	aliases: ['rm'],
	usage: '/delete <id>',
	description: 'Delete a note by ID or prefix',
	category: 'notes',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (!rest) return `${colors.yellow('Usage:')} /delete <id>`;
		const note = resolveNote(ctx.app, rest);
		if (!note) return `${colors.red('✗')} Note "${rest}" not found.`;
		await ctx.app.deleteNote(note.id);
		return `${colors.green('✓')} Deleted ${colors.dim(note.id.slice(0, 8))}`;
	},
};

// ---------------------------------------------------------------------------
// Auto-compact — triggered when conversation exceeds threshold
// ---------------------------------------------------------------------------

async function maybeAutoCompact(ctx: AppContext): Promise<void> {
	const { conversation, colors, spinner } = ctx;
	if (!conversation.needsCompaction) return;

	spinner.start('Auto-compacting conversation...');
	try {
		const conversationText = conversation.serialize();
		const prompt = `Summarize the following conversation concisely, preserving key decisions, code changes, and context needed for future turns:\n\n${conversationText}`;
		const result = await ctx.app.generate(prompt, { skipMemory: true });
		const prevCount = conversation.messageCount;
		conversation.compact(result.content);
		spinner.succeed(
			`Auto-compacted ${prevCount} messages ${colors.dim('(context was getting large)')}`,
		);
	} catch {
		spinner.fail('Auto-compact failed — conversation may be large');
	}
}

// ---------------------------------------------------------------------------
// AI — bare text handler (primary interaction mode)
// ---------------------------------------------------------------------------

async function handleBareTextInput(
	ctx: AppContext,
	input: string,
): Promise<string | undefined> {
	const { colors, spinner, session } = ctx;

	if (!input) return undefined;

	session.lastUserInput = input;

	// 0a. Resolve @-file mentions
	const mentionResult = resolveFileMentions(input);
	let processedInput = mentionResult.cleanInput || input;
	if (mentionResult.mentions.length > 0) {
		const mentionContext = formatMentionsAsContext(mentionResult.mentions);
		processedInput = `${mentionContext}\n\n${processedInput}`;
		for (const mention of mentionResult.mentions) {
			console.log(
				renderDetailLine(`@${mention.path} (${mention.size} bytes)`, colors),
			);
		}
	}

	// 0b. Detect image attachments
	const imageResult = detectImages(processedInput);
	if (imageResult.images.length > 0) {
		processedInput = imageResult.cleanInput;
		for (const img of imageResult.images) {
			console.log(formatImageIndicator(img, colors));
		}
	}

	// 0c. Auto-checkpoint before AI prompt
	if (ctx.checkpointManager) {
		ctx.checkpointManager.save('pre-prompt');
	}

	// 1. Pre-loop memory context injection
	let enrichedInput = processedInput;
	if (session.memoryEnabled && ctx.app.noteCount > 0) {
		try {
			const results = await ctx.app.search(input, 5);
			if (results.length > 0) {
				const contextBlock = results
					.map(
						(r) =>
							`[${r.note.topic}] (relevance: ${r.score.toFixed(2)}) ${r.note.text}`,
					)
					.join('\n');
				enrichedInput = `Relevant context from memory:\n${contextBlock}\n\nUser query: ${input}`;
				console.log(
					renderDetailLine(
						`${results.length} memory entries used as context`,
						colors,
					),
				);
			}
		} catch {
			// Memory search failed — proceed without context
		}
	}

	// 2. Build agent system prompt
	const systemPromptParts: string[] = [];
	if (session.agentName) {
		const agent = ctx.configResult.agents.find(
			(a) => a.name === session.agentName,
		);
		if (agent?.systemPrompt) systemPromptParts.push(agent.systemPrompt);
	}
	if (ctx.configResult.workspacePrompt) {
		systemPromptParts.push(ctx.configResult.workspacePrompt);
	}

	// 2b. Append skill catalog so the AI knows what skills are available
	const skillCatalog = ctx.skillRegistry.formatForSystemPrompt();
	if (skillCatalog) systemPromptParts.push(skillCatalog);

	// 3. Create abort controller for SIGINT cancellation
	const abortController = new AbortController();
	session.abortController = abortController;

	// 4. Determine if the ACP agent manages its own tools.
	// Agents with permissionPolicy (Claude Code, Copilot) have native tools
	// and don't need <tool_use> XML injection. Simple text generators (Ollama)
	// need tools injected via the system prompt.
	const resolvedServerName =
		session.serverName ??
		ctx.configResult.config.acp.defaultServer ??
		ctx.configResult.config.acp.servers[0]?.name;
	const serverEntry = ctx.configResult.config.acp.servers.find(
		(s) => s.name === resolvedServerName,
	);
	const agentManagesTools = !!serverEntry?.permissionPolicy;

	// 5. Create and run the agentic loop
	const loop = createAgenticLoop({
		acpClient: ctx.acpClient,
		toolRegistry: ctx.toolRegistry,
		conversation: ctx.conversation,
		maxTurns: session.maxTurns,
		serverName: session.serverName,
		systemPrompt: systemPromptParts.join('\n\n') || undefined,
		signal: abortController.signal,
		agentManagesTools,
	});

	spinner.start();
	let firstChunk = true;
	let hadError = false;
	const toolTimings = new Map<string, number>();
	const toolArgs = new Map<string, string>();

	const agentToolMeta = new Map<string, { title: string; kind: string }>();

	const result = await loop.run(enrichedInput, {
		onStreamStart: () => {
			// Spinner continues until first delta
		},
		onStreamDelta: (text) => {
			if (firstChunk) {
				spinner.stop();
				process.stdout.write(`\n  ${colors.magenta('●')} `);
				firstChunk = false;
			}
			writeStreamText(text);
		},
		onToolCallStart: (call) => {
			if (!firstChunk) {
				process.stdout.write('\n\n');
				firstChunk = true;
			}
			spinner.stop();
			const argsStr = JSON.stringify(call.arguments);
			toolTimings.set(call.id, Date.now());
			toolArgs.set(call.id, argsStr);
			console.log(renderToolCallActive(call.name, argsStr, colors));
			spinner.start();
		},
		onToolCallEnd: (toolResult) => {
			spinner.stop();
			const startTime = toolTimings.get(toolResult.id);
			const durationMs =
				startTime !== undefined ? Date.now() - startTime : undefined;
			const argsStr = toolArgs.get(toolResult.id) ?? '{}';
			if (toolResult.isError) {
				console.log(
					renderToolCallFailed(
						toolResult.name,
						argsStr,
						toolResult.output,
						colors,
					),
				);
			} else {
				const summary = deriveToolSummary(toolResult.name, toolResult.output);
				console.log(
					renderToolCallCompleted(toolResult.name, argsStr, colors, {
						durationMs,
						summary,
					}),
				);
			}
			console.log(
				renderToolResultCollapsed(
					toolResult.name,
					toolResult.output,
					toolResult.isError,
					colors,
				),
			);
			spinner.start();
		},
		onAgentToolCall: (toolCall) => {
			if (!firstChunk) {
				process.stdout.write('\n\n');
				firstChunk = true;
			}
			spinner.stop();
			toolTimings.set(toolCall.toolCallId, Date.now());
			agentToolMeta.set(toolCall.toolCallId, {
				title: toolCall.title || toolCall.toolCallId,
				kind: toolCall.kind,
			});
			console.log(
				renderAgentToolCallActive(
					toolCall.title || toolCall.toolCallId,
					toolCall.kind,
					colors,
				),
			);
			spinner.start();
		},
		onAgentToolCallUpdate: (update) => {
			if (update.status === 'completed' || update.status === 'failed') {
				spinner.stop();
				const startTime = toolTimings.get(update.toolCallId);
				const durationMs =
					startTime !== undefined ? Date.now() - startTime : undefined;
				const meta = agentToolMeta.get(update.toolCallId);
				const title = meta?.title ?? '';
				const kind = meta?.kind ?? 'other';
				const output =
					typeof update.content === 'string'
						? update.content
						: update.content
							? JSON.stringify(update.content)
							: update.status;
				if (update.status === 'failed') {
					console.log(renderAgentToolCallFailed(kind, output, colors));
				} else {
					console.log(
						renderAgentToolCallCompleted(title, kind, colors, {
							durationMs,
						}),
					);
				}
				console.log(
					renderToolResultCollapsed(
						'',
						output,
						update.status === 'failed',
						colors,
					),
				);
				spinner.start();
			}
		},
		onError: (error) => {
			spinner.stop();
			if (!firstChunk) {
				process.stdout.write('\n');
				firstChunk = true;
			}
			console.log(renderError(error.message, colors));
			hadError = true;
		},
	});

	spinner.stop();
	session.abortController = undefined;
	if (!firstChunk) process.stdout.write('\n');

	// Track turns
	session.totalTurns += result.totalTurns;

	if (result.aborted) {
		console.log(colors.dim('  Generation interrupted.'));
		return undefined;
	}

	if (result.hitTurnLimit) {
		console.log(
			colors.yellow(`  Reached turn limit (${result.totalTurns} turns)`),
		);
	}

	// 5. Store in memory — but only meaningful responses
	const isError =
		hadError ||
		!result.finalText ||
		result.finalText.startsWith('Error communicating') ||
		result.finalText.startsWith('No response received');

	if (session.memoryEnabled && result.finalText && !isError) {
		try {
			const id = await ctx.app.addNote(
				`Q: ${input}\nA: ${result.finalText}`,
				'conversation',
				{ source: 'generate' },
			);
			console.log(renderDetailLine(`stored as ${id.slice(0, 8)}`, colors));
		} catch {
			// Storage failed — ignore
		}
	}

	// 6. Auto-compact if conversation is getting large
	await maybeAutoCompact(ctx);

	return undefined;
}

// ---------------------------------------------------------------------------
// Skill invocation
// ---------------------------------------------------------------------------

async function handleSkillInvocation(
	ctx: AppContext,
	skill: import('./skills.js').SkillConfig,
	args: string,
): Promise<string | undefined> {
	const { colors, spinner, session } = ctx;

	console.log(renderSkillLoading(skill.name, colors));

	// 1. Resolve $ARGUMENTS in skill body
	const resolvedBody = ctx.skillRegistry.resolveBody(skill, args);

	// 2. Build system prompt: skill instructions first, then agent/project prompt
	const systemPromptParts: string[] = [resolvedBody];
	if (session.agentName) {
		const agent = ctx.configResult.agents.find(
			(a) => a.name === session.agentName,
		);
		if (agent?.systemPrompt) systemPromptParts.push(agent.systemPrompt);
	}
	if (ctx.configResult.workspacePrompt) {
		systemPromptParts.push(ctx.configResult.workspacePrompt);
	}

	// 3. User message: the args, or a default instruction
	const userMessage = args || `Execute the ${skill.name} skill.`;

	// 4. Create abort controller for SIGINT cancellation
	const abortController = new AbortController();
	session.abortController = abortController;

	// 5. Determine if agent manages its own tools
	const skillServerName =
		skill.serverName ??
		session.serverName ??
		ctx.configResult.config.acp.defaultServer ??
		ctx.configResult.config.acp.servers[0]?.name;
	const skillServerEntry = ctx.configResult.config.acp.servers.find(
		(s) => s.name === skillServerName,
	);
	const skillAgentManagesTools = !!skillServerEntry?.permissionPolicy;

	// 6. Run the agentic loop
	const loop = createAgenticLoop({
		acpClient: ctx.acpClient,
		toolRegistry: ctx.toolRegistry,
		conversation: ctx.conversation,
		maxTurns: session.maxTurns,
		serverName: skill.serverName ?? session.serverName,
		systemPrompt: systemPromptParts.join('\n\n'),
		signal: abortController.signal,
		agentManagesTools: skillAgentManagesTools,
	});

	spinner.start();
	let firstChunk = true;
	let hadError = false;
	const skillToolTimings = new Map<string, number>();
	const skillToolArgs = new Map<string, string>();

	const result = await loop.run(userMessage, {
		onStreamStart: () => {},
		onStreamDelta: (text) => {
			if (firstChunk) {
				spinner.stop();
				process.stdout.write(`\n  ${colors.magenta('●')} `);
				firstChunk = false;
			}
			writeStreamText(text);
		},
		onToolCallStart: (call) => {
			if (!firstChunk) {
				process.stdout.write('\n\n');
				firstChunk = true;
			}
			spinner.stop();
			const argsStr = JSON.stringify(call.arguments);
			skillToolTimings.set(call.id, Date.now());
			skillToolArgs.set(call.id, argsStr);
			console.log(renderToolCallActive(call.name, argsStr, colors));
			spinner.start();
		},
		onToolCallEnd: (toolResult) => {
			spinner.stop();
			const startTime = skillToolTimings.get(toolResult.id);
			const durationMs =
				startTime !== undefined ? Date.now() - startTime : undefined;
			const argsStr = skillToolArgs.get(toolResult.id) ?? '{}';
			if (toolResult.isError) {
				console.log(
					renderToolCallFailed(
						toolResult.name,
						argsStr,
						toolResult.output,
						colors,
					),
				);
			} else {
				const summary = deriveToolSummary(toolResult.name, toolResult.output);
				console.log(
					renderToolCallCompleted(toolResult.name, argsStr, colors, {
						durationMs,
						summary,
					}),
				);
			}
			console.log(
				renderToolResultCollapsed(
					toolResult.name,
					toolResult.output,
					toolResult.isError,
					colors,
				),
			);
			spinner.start();
		},
		onError: (error) => {
			spinner.stop();
			if (!firstChunk) {
				process.stdout.write('\n');
				firstChunk = true;
			}
			console.log(renderError(error.message, colors));
			hadError = true;
		},
	});

	spinner.stop();
	session.abortController = undefined;
	if (!firstChunk) process.stdout.write('\n');

	session.totalTurns += result.totalTurns;

	if (result.aborted) {
		console.log(colors.dim('  Generation interrupted.'));
		return undefined;
	}

	if (result.hitTurnLimit) {
		console.log(
			colors.yellow(`  Reached turn limit (${result.totalTurns} turns)`),
		);
	}

	// 6. Store in memory — but only meaningful responses
	const isError =
		hadError ||
		!result.finalText ||
		result.finalText.startsWith('Error communicating') ||
		result.finalText.startsWith('No response received');

	if (session.memoryEnabled && result.finalText && !isError) {
		try {
			const id = await ctx.app.addNote(
				`Q: /${skill.name} ${args}\nA: ${result.finalText}`,
				'conversation',
				{ source: 'skill', skill: skill.name },
			);
			console.log(renderDetailLine(`stored as ${id.slice(0, 8)}`, colors));
		} catch {
			// Storage failed — ignore
		}
	}

	// 7. Auto-compact if conversation is getting large
	await maybeAutoCompact(ctx);

	return undefined;
}

// ---------------------------------------------------------------------------
// AI commands
// ---------------------------------------------------------------------------

const chainCommand: Command = {
	name: 'chain',
	aliases: ['prompt'],
	usage: '/chain <name|template> [key=value ...]',
	description: 'Run a named prompt or ad-hoc chain',
	category: 'ai',
	handler: async (ctx, rest) => {
		const { colors, spinner, md } = ctx;
		if (!rest)
			return `${colors.yellow('Usage:')} /chain <name|template> [key=value ...]`;

		const skipMemory = !ctx.session.memoryEnabled;
		const parts = rest.split(/\s+/);
		const firstPart = parts[0];

		// Check if the first token is a named prompt from project config
		const namedPrompt = ctx.configResult.prompts[firstPart];
		if (namedPrompt) {
			const values: Record<string, string> = {};
			for (const part of parts.slice(1)) {
				const eqIdx = part.indexOf('=');
				if (eqIdx > 0) {
					values[part.slice(0, eqIdx)] = part.slice(eqIdx + 1);
				}
			}
			spinner.start(`Running prompt "${firstPart}"...`);
			const output = await ctx.app.runNamedPrompt(
				firstPart,
				namedPrompt,
				values,
				{ skipMemory },
			);
			spinner.stop();
			return renderAssistantMessage(output, md, colors);
		}

		// Fall back to ad-hoc template mode
		const templateParts: string[] = [];
		const values: Record<string, string> = {};

		for (const part of parts) {
			const eqIdx = part.indexOf('=');
			if (eqIdx > 0 && !part.startsWith('{')) {
				values[part.slice(0, eqIdx)] = part.slice(eqIdx + 1);
			} else {
				templateParts.push(part);
			}
		}

		const template = templateParts.join(' ');
		spinner.start('Running chain...');
		const output = await ctx.app.runChain(template, values, { skipMemory });
		spinner.stop();
		return renderAssistantMessage(output, md, colors);
	},
};

// ---------------------------------------------------------------------------
// Tool commands
// ---------------------------------------------------------------------------

const promptsCommand: Command = {
	name: 'prompts',
	usage: '/prompts',
	description: 'List named prompts',
	category: 'ai',
	handler: (ctx) => {
		const { colors } = ctx;
		const { prompts } = ctx.configResult;

		const names = Object.keys(prompts);
		if (names.length === 0)
			return colors.dim('No prompts defined in .simse/prompts.json');

		return names
			.map((name) => {
				const p = prompts[name];
				const desc = p.description
					? ` ${colors.dim('—')} ${colors.dim(p.description)}`
					: '';
				const stepCount = p.steps.length;
				const vars = new Set<string>();
				for (const step of p.steps) {
					for (const m of step.template.matchAll(/\{([\w-]+)\}/g)) {
						vars.add(m[1]);
					}
				}
				const varStr =
					vars.size > 0 ? ` ${colors.cyan(`[${[...vars].join(', ')}]`)}` : '';
				return `  ${colors.bold(name)}${desc} ${colors.dim(`(${stepCount} step${stepCount > 1 ? 's' : ''})`)}${varStr}`;
			})
			.join('\n');
	},
};

const toolsCommand: Command = {
	name: 'tools',
	usage: '/tools [server]',
	description: 'List available MCP tools',
	category: 'tools',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		const tools = await ctx.app.tools.listTools(rest || undefined);
		if (tools.length === 0) return colors.dim('No tools available.');
		return tools
			.map(
				(t) =>
					`  ${colors.magenta(`[${t.serverName}]`)} ${colors.bold(t.name)}: ${colors.dim(t.description ?? '')}`,
			)
			.join('\n');
	},
};

const agentsCommand: Command = {
	name: 'agents',
	usage: '/agents',
	description: 'List ACP agents and custom agents',
	category: 'tools',
	handler: async (ctx) => {
		const { colors } = ctx;
		const lines: string[] = [];

		// ACP server agents
		const acpAgents = await ctx.app.agents.client.listAgents();
		if (acpAgents.length > 0) {
			lines.push(colors.bold(colors.cyan('ACP agents:')));
			for (const a of acpAgents) {
				lines.push(
					`  ${colors.bold(a.id)}: ${colors.dim(a.description ?? a.name ?? '')}`,
				);
			}
		}

		// Custom agents from .simse/agents/*.md
		const { agents: customAgents } = ctx.configResult;
		if (customAgents.length > 0) {
			if (lines.length > 0) lines.push('');
			lines.push(colors.bold(colors.cyan('Custom agents:')));
			for (const a of customAgents) {
				const desc = a.description
					? ` ${colors.dim('—')} ${colors.dim(a.description)}`
					: '';
				const model = a.model ? ` ${colors.dim(`[${a.model}]`)}` : '';
				lines.push(`  ${colors.bold(a.name)}${desc}${model}`);
			}
		}

		if (lines.length === 0) return colors.dim('No agents configured.');
		return lines.join('\n');
	},
};

// ---------------------------------------------------------------------------
// MCP command — preset-driven server management
// ---------------------------------------------------------------------------

interface MCPPreset {
	readonly label: string;
	readonly description: string;
	readonly build: (
		rl: ReadlineInterface,
	) => Promise<Record<string, unknown> | undefined>;
}

const mcpPresets: readonly MCPPreset[] = [
	{
		label: 'Filesystem',
		description: 'Read/write/search local files',
		build: async (rl) => {
			const dirs = (
				await settingsAsk(rl, '  Allowed directories (space-separated) [.]: ')
			).trim();
			const allowedDirs = dirs ? dirs.split(/\s+/) : ['.'];
			return {
				name: 'filesystem',
				transport: 'stdio',
				command: 'bunx',
				args: ['@modelcontextprotocol/server-filesystem', ...allowedDirs],
			};
		},
	},
	{
		label: 'GitHub',
		description: 'Repos, issues, PRs, search',
		build: async (rl) => {
			const token = (
				await settingsAsk(
					rl,
					'  GitHub token (enter to use GITHUB_TOKEN env var): ',
				)
			).trim();
			return {
				name: 'github',
				transport: 'stdio',
				command: 'bunx',
				args: ['@modelcontextprotocol/server-github'],
				...(token
					? { env: { GITHUB_PERSONAL_ACCESS_TOKEN: token } }
					: { requiredEnv: ['GITHUB_PERSONAL_ACCESS_TOKEN'] }),
			};
		},
	},
	{
		label: 'Brave Search',
		description: 'Web and local search via Brave',
		build: async (rl) => {
			const key = (
				await settingsAsk(
					rl,
					'  Brave API key (enter to use BRAVE_API_KEY env var): ',
				)
			).trim();
			return {
				name: 'brave-search',
				transport: 'stdio',
				command: 'bunx',
				args: ['@modelcontextprotocol/server-brave-search'],
				...(key
					? { env: { BRAVE_API_KEY: key } }
					: { requiredEnv: ['BRAVE_API_KEY'] }),
			};
		},
	},
	{
		label: 'Fetch',
		description: 'Fetch and convert web pages to markdown',
		build: async () => ({
			name: 'fetch',
			transport: 'stdio',
			command: 'bunx',
			args: ['@modelcontextprotocol/server-fetch'],
		}),
	},
	{
		label: 'Memory',
		description: 'Persistent knowledge graph memory',
		build: async () => ({
			name: 'memory',
			transport: 'stdio',
			command: 'bunx',
			args: ['@modelcontextprotocol/server-memory'],
		}),
	},
	{
		label: 'Puppeteer',
		description: 'Browser automation and screenshots',
		build: async () => ({
			name: 'puppeteer',
			transport: 'stdio',
			command: 'bunx',
			args: ['@modelcontextprotocol/server-puppeteer'],
		}),
	},
	{
		label: 'Slack',
		description: 'Channels, messages, users',
		build: async (rl) => {
			const token = (
				await settingsAsk(
					rl,
					'  Slack bot token (enter to use SLACK_BOT_TOKEN env var): ',
				)
			).trim();
			const teamId = (
				await settingsAsk(
					rl,
					'  Slack team ID (enter to use SLACK_TEAM_ID env var): ',
				)
			).trim();
			return {
				name: 'slack',
				transport: 'stdio',
				command: 'bunx',
				args: ['@modelcontextprotocol/server-slack'],
				env: {
					...(token ? { SLACK_BOT_TOKEN: token } : {}),
					...(teamId ? { SLACK_TEAM_ID: teamId } : {}),
				},
				requiredEnv: [
					...(token ? [] : ['SLACK_BOT_TOKEN']),
					...(teamId ? [] : ['SLACK_TEAM_ID']),
				],
			};
		},
	},
	{
		label: 'PostgreSQL',
		description: 'Query PostgreSQL databases',
		build: async (rl) => {
			const connStr = (await settingsAsk(rl, '  Connection string: ')).trim();
			if (!connStr) return undefined;
			return {
				name: 'postgres',
				transport: 'stdio',
				command: 'bunx',
				args: ['@modelcontextprotocol/server-postgres', connStr],
			};
		},
	},
	{
		label: 'Custom',
		description: 'Any MCP-compatible server',
		build: async (rl) => {
			const name = await askSettingsRequired(rl, '  Server name: ');
			const command = await askSettingsRequired(rl, '  Command: ');
			const argsStr = (
				await settingsAsk(rl, '  Args (space-separated, enter to skip): ')
			).trim();
			const args = argsStr ? argsStr.split(/\s+/) : undefined;
			const envStr = (
				await settingsAsk(
					rl,
					'  Required env vars (space-separated, enter to skip): ',
				)
			).trim();
			const requiredEnv = envStr ? envStr.split(/\s+/) : undefined;

			return {
				name,
				transport: 'stdio',
				command,
				...(args && { args }),
				...(requiredEnv && { requiredEnv }),
			};
		},
	},
];

async function mcpMenu(ctx: AppContext): Promise<string | undefined> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'mcp.json');

	type McpConfig = { servers?: Array<Record<string, unknown>> };
	const config: McpConfig = readJsonSafe<McpConfig>(filePath) ?? {
		servers: [],
	};
	const servers = config.servers ?? [];

	console.log(`\n${colors.bold(colors.cyan('MCP Servers'))}\n`);

	if (servers.length > 0) {
		for (let i = 0; i < servers.length; i++) {
			const s = servers[i];
			const envInfo =
				Array.isArray(s.requiredEnv) && (s.requiredEnv as string[]).length > 0
					? colors.dim(` (requires: ${(s.requiredEnv as string[]).join(', ')})`)
					: '';
			console.log(
				`  ${colors.bold(`${i + 1})`)} ${colors.green('✓')} ${colors.bold(String(s.name ?? 'unnamed'))} ${colors.dim(`— ${s.command ?? ''} ${Array.isArray(s.args) ? (s.args as string[]).join(' ') : ''}`)}${envInfo}`,
			);
		}
	} else {
		console.log(colors.dim('  No MCP servers configured.'));
	}

	console.log(`\n  ${colors.dim('[a]dd / [r]emove <num> / [b]ack')}`);
	const choice = (
		await settingsAsk(rl, `\n${colors.cyan('mcp')}${colors.dim('>')} `)
	)
		.trim()
		.toLowerCase();

	if (choice === 'back' || choice === 'b' || choice === 'q' || choice === '')
		return undefined;

	if (choice === 'a' || choice === 'add') {
		console.log(`\n  ${colors.bold('Select a server to add:')}\n`);
		for (let i = 0; i < mcpPresets.length; i++) {
			const p = mcpPresets[i];
			console.log(
				`    ${colors.bold(`${i + 1})`)} ${p.label}  ${colors.dim(`—  ${p.description}`)}`,
			);
		}
		console.log('');

		const presetAnswer = (
			await settingsAsk(rl, `  Choice [1-${mcpPresets.length}]: `)
		).trim();
		const presetIdx = Number.parseInt(presetAnswer, 10) - 1;
		if (
			Number.isNaN(presetIdx) ||
			presetIdx < 0 ||
			presetIdx >= mcpPresets.length
		) {
			return colors.yellow('Invalid choice.');
		}

		const server = await mcpPresets[presetIdx].build(rl);
		if (!server) return colors.dim('Cancelled.');

		// Check for duplicate name
		const name = String(server.name);
		if (servers.some((s) => s.name === name)) {
			return `${colors.yellow('✗')} Server "${name}" already exists. Remove it first.`;
		}

		servers.push(server);
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Added "${name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('r') || choice.startsWith('remove')) {
		const numStr = choice.replace(/^r(emove)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return colors.yellow('Invalid server number.');
		}
		const removed = servers.splice(idx, 1)[0];
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Removed "${removed.name}". ${colors.dim('Restart to apply.')}`;
	}

	return `${colors.yellow('Unknown option.')} Enter "add", "remove <num>", or "back".`;
}

const mcpCommand: Command = {
	name: 'mcp',
	usage: '/mcp',
	description: 'Add, remove, or configure MCP tool servers',
	category: 'tools',
	handler: async (ctx) => {
		return mcpMenu(ctx);
	},
};

// ---------------------------------------------------------------------------
// ACP command — interactive ACP server management
// ---------------------------------------------------------------------------

async function acpMenu(ctx: AppContext): Promise<string | undefined> {
	const { colors, rl, dataDir, acpClient } = ctx;
	const filePath = join(dataDir, 'acp.json');

	type AcpConfig = {
		servers?: Array<Record<string, unknown>>;
		defaultServer?: string;
		defaultAgent?: string;
	};
	const config: AcpConfig = readJsonSafe<AcpConfig>(filePath) ?? {
		servers: [],
	};
	const servers = config.servers ?? [];

	console.log(`\n${colors.bold(colors.cyan('ACP Servers'))}\n`);

	if (servers.length > 0) {
		for (let i = 0; i < servers.length; i++) {
			const s = servers[i];
			const isDefault = config.defaultServer === s.name;
			const marker = isDefault ? colors.green(' (default)') : '';
			console.log(
				`  ${colors.bold(`${i + 1})`)} ${colors.bold(String(s.name ?? 'unnamed'))}${marker} ${colors.dim(`— ${s.command ?? ''} ${Array.isArray(s.args) ? (s.args as string[]).join(' ') : ''}`)}`,
			);
		}
	} else {
		console.log(colors.dim('  No ACP servers configured.'));
	}

	console.log(
		`\n  ${colors.dim('[a]dd / [r]emove <num> / [d]efault <num> / [t]est / [b]ack')}`,
	);
	const choice = (
		await settingsAsk(rl, `\n${colors.cyan('acp')}${colors.dim('>')} `)
	)
		.trim()
		.toLowerCase();

	if (choice === 'back' || choice === 'b' || choice === 'q' || choice === '')
		return undefined;

	if (choice === 'a' || choice === 'add') {
		const name = await askSettingsRequired(rl, '  Name: ');
		const command = await askSettingsRequired(
			rl,
			'  Command (e.g. bun, node): ',
		);
		const argsStr = (
			await settingsAsk(rl, '  Args (space-separated, enter to skip): ')
		).trim();
		const args = argsStr ? argsStr.split(/\s+/) : undefined;

		if (servers.some((s) => s.name === name)) {
			return `${colors.yellow('●')} Server "${name}" already exists. Remove it first.`;
		}

		const server: Record<string, unknown> = {
			name,
			command,
			...(args && { args }),
		};
		servers.push(server);
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('●')} Added "${name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('r') || choice.startsWith('remove')) {
		const numStr = choice.replace(/^r(emove)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return colors.yellow('Invalid server number.');
		}
		const removed = servers.splice(idx, 1)[0];
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('●')} Removed "${removed.name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('d') || choice.startsWith('default')) {
		const numStr = choice.replace(/^d(efault)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return colors.yellow('Invalid server number.');
		}
		config.defaultServer = String(servers[idx].name);
		writeJsonSafe(filePath, config);
		return `${colors.green('●')} Default server set to "${servers[idx].name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice === 't' || choice === 'test') {
		const lines: string[] = [];
		const names = acpClient.serverNames;
		if (names.length === 0) {
			return colors.dim('No ACP servers to test.');
		}
		for (const name of names) {
			try {
				const available = await acpClient.isAvailable(name);
				if (available) {
					lines.push(
						renderServiceStatus('ACP', 'ok', `${name} — connected`, colors),
					);
				} else {
					lines.push(
						renderServiceStatus(
							'ACP',
							'fail',
							`${name} — not responding`,
							colors,
						),
					);
				}
			} catch {
				lines.push(
					renderServiceStatus(
						'ACP',
						'fail',
						`${name} — connection error`,
						colors,
					),
				);
			}
		}
		return lines.join('\n');
	}

	return `${colors.yellow('Unknown option.')} Enter "add", "remove <num>", "default <num>", "test", or "back".`;
}

const acpCommand: Command = {
	name: 'acp',
	usage: '/acp',
	description: 'Add, remove, or configure ACP servers',
	category: 'tools',
	handler: async (ctx) => {
		return acpMenu(ctx);
	},
};

// ---------------------------------------------------------------------------
// Embed command — preset-driven embedding provider selection
// ---------------------------------------------------------------------------

interface EmbedPreset {
	readonly label: string;
	readonly description: string;
	readonly build: (rl: ReadlineInterface) => Promise<Record<string, unknown>>;
}

const embedPresets: readonly EmbedPreset[] = [
	{
		label: 'nomic-embed-text-v1.5 (default)',
		description: 'High-quality 768-dim embeddings, q8 quantized (~33MB)',
		build: async () => ({
			embeddingModel: 'nomic-ai/nomic-embed-text-v1.5',
			dtype: 'q8',
		}),
	},
	{
		label: 'all-MiniLM-L6-v2',
		description: 'Fast 384-dim embeddings, smallest model (~23MB)',
		build: async () => ({
			embeddingModel: 'Xenova/all-MiniLM-L6-v2',
			dtype: 'q8',
		}),
	},
	{
		label: 'bge-small-en-v1.5',
		description: 'Compact 384-dim embeddings, strong benchmarks (~33MB)',
		build: async () => ({
			embeddingModel: 'Xenova/bge-small-en-v1.5',
			dtype: 'q8',
		}),
	},
	{
		label: 'Custom',
		description: 'Any Hugging Face ONNX embedding model',
		build: async (rl) => {
			const model =
				(
					await settingsAsk(
						rl,
						'  HF model ID [nomic-ai/nomic-embed-text-v1.5]: ',
					)
				).trim() || 'nomic-ai/nomic-embed-text-v1.5';
			const dtype =
				(
					await settingsAsk(rl, '  Quantization (fp32/fp16/q8/q4) [q8]: ')
				).trim() || 'q8';
			return { embeddingModel: model, dtype };
		},
	},
];

async function embedMenu(ctx: AppContext): Promise<string | undefined> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'embed.json');

	type EmbedConfig = Record<string, unknown>;
	const config: EmbedConfig = readJsonSafe<EmbedConfig>(filePath) ?? {};

	console.log(`\n${colors.bold(colors.cyan('Embedding Provider'))}\n`);

	// Show current config
	const model = config.embeddingModel;
	const dtype = config.dtype;
	if (model) {
		console.log(`  ${colors.bold('Current:')}`);
		console.log(`    ${colors.bold('Model:')} ${colors.cyan(String(model))}`);
		if (dtype)
			console.log(`    ${colors.bold('Dtype:')} ${colors.cyan(String(dtype))}`);
	} else {
		console.log(
			colors.dim('  Using default: nomic-ai/nomic-embed-text-v1.5 (q8)'),
		);
	}

	console.log(`\n  ${colors.bold('Select a provider:')}\n`);
	for (let i = 0; i < embedPresets.length; i++) {
		const p = embedPresets[i];
		console.log(
			`    ${colors.bold(`${i + 1})`)} ${p.label}  ${colors.dim(`—  ${p.description}`)}`,
		);
	}
	console.log('');

	const choice = (
		await settingsAsk(rl, `  Choice [1-${embedPresets.length}] or "back": `)
	)
		.trim()
		.toLowerCase();

	if (choice === 'back' || choice === 'b' || choice === 'q' || choice === '')
		return undefined;

	const presetIdx = Number.parseInt(choice, 10) - 1;
	if (
		Number.isNaN(presetIdx) ||
		presetIdx < 0 ||
		presetIdx >= embedPresets.length
	) {
		return colors.yellow('Invalid choice.');
	}

	const result = await embedPresets[presetIdx].build(rl);
	writeJsonSafe(filePath, result);

	const newConfig = result as EmbedFileConfig;
	const savedState = readEmbedState(dataDir);
	const hasNotes = ctx.app.noteCount > 0;
	const changed = embedConfigChanged(newConfig, savedState);

	if (hasNotes && changed) {
		console.log('');
		console.log(
			`  ${colors.yellow('⚠')} Embedding provider changed. ${colors.cyan(String(ctx.app.noteCount))} notes need re-embedding.`,
		);
		const answer = (
			await settingsAsk(rl, `  Re-embed all notes now? ${colors.dim('[y/N]')} `)
		)
			.trim()
			.toLowerCase();

		if (answer === 'y') {
			const { spinner } = ctx;
			spinner.start('Re-embedding notes...');
			try {
				const count = await ctx.app.reembed((done, total) => {
					spinner.update(`Re-embedding notes... ${done}/${total}`);
				});
				spinner.succeed(`Re-embedded ${colors.cyan(String(count))} notes`);
				writeEmbedState(dataDir, newConfig);
				return undefined;
			} catch (err) {
				spinner.fail(
					`Re-embed failed: ${err instanceof Error ? err.message : err}`,
				);
				return `${colors.dim('Restart to retry, or run "/embed" again.')}`;
			}
		}

		writeEmbedState(dataDir, newConfig);
		return `${colors.green('✓')} Provider updated. ${colors.yellow('Notes not re-embedded — search may be degraded.')}`;
	}

	writeEmbedState(dataDir, newConfig);
	return `${colors.green('✓')} Embedding provider updated.`;
}

const embedCommand: Command = {
	name: 'embed',
	usage: '/embed',
	description: 'Select and configure embedding provider',
	category: 'tools',
	handler: async (ctx) => {
		return embedMenu(ctx);
	},
};

// ---------------------------------------------------------------------------
// Info commands
// ---------------------------------------------------------------------------

const learningCommand: Command = {
	name: 'learning',
	usage: '/learning',
	description: 'Show adaptive learning profile',
	category: 'info',
	handler: (ctx) => {
		const { colors } = ctx;
		const profile = ctx.app.getLearningProfile();
		if (!profile) return colors.dim('No learning data yet.');
		const w = profile.adaptedWeights;
		return [
			`  ${colors.bold('Queries:')}   ${profile.totalQueries}`,
			`  ${colors.bold('Weights:')}   vector=${colors.cyan(w.vector.toFixed(3))} recency=${colors.cyan(w.recency.toFixed(3))} frequency=${colors.cyan(w.frequency.toFixed(3))}`,
			profile.interestEmbedding
				? `  ${colors.bold('Interest:')}  ${profile.interestEmbedding.length} dimensions`
				: `  ${colors.bold('Interest:')}  ${colors.dim('none')}`,
		].join('\n');
	},
};

const statsCommand: Command = {
	name: 'stats',
	usage: '/stats',
	description: 'Show knowledge base statistics',
	category: 'info',
	handler: (ctx) => {
		const { colors } = ctx;
		const profile = ctx.app.getLearningProfile();
		const lines = [
			`  ${colors.bold('Notes:')}     ${ctx.app.noteCount}`,
			`  ${colors.bold('Topics:')}    ${ctx.app.getTopics().length}`,
			`  ${colors.bold('Queries:')}   ${profile?.totalQueries ?? 0}`,
			`  ${colors.bold('MCP:')}       ${ctx.app.tools.mcpClient.connectionCount} connections`,
			`  ${colors.bold('ACP:')}       ${ctx.app.agents.client.serverCount} servers`,
			`  ${colors.bold('Data:')}      ${colors.dim(ctx.dataDir)}`,
		];

		// Usage tracking stats
		if (ctx.usageTracker) {
			const totals = ctx.usageTracker.getTotals();
			lines.push('');
			lines.push(colors.bold(colors.cyan('Usage (all time):')));
			lines.push(`  ${colors.bold('Sessions:')}  ${totals.totalSessions}`);
			lines.push(`  ${colors.bold('Messages:')}  ${totals.totalMessages}`);
			lines.push(`  ${colors.bold('Tools:')}     ${totals.totalToolCalls}`);

			// 7-day chart
			const history = ctx.usageTracker.getHistory(7);
			if (history.some((d) => d.messages > 0)) {
				lines.push('');
				lines.push(colors.bold(colors.cyan('Last 7 days:')));
				lines.push(renderUsageChart(history, colors));
			}
		}

		return lines.join('\n');
	},
};

const configCommand: Command = {
	name: 'config',
	usage: '/config',
	description: 'Show current configuration',
	category: 'info',
	handler: (ctx) => {
		const { colors } = ctx;
		const {
			config,
			skippedServers,
			workspaceSettings,
			prompts,
			agents: customAgents,
			workspacePrompt,
		} = ctx.configResult;
		const lines: string[] = [];

		lines.push(colors.bold(colors.cyan('Global config:')));
		lines.push(`  ${colors.dim(join(ctx.dataDir, 'config.json'))}`);
		lines.push(`  ${colors.dim(join(ctx.dataDir, 'acp.json'))}`);
		lines.push(`  ${colors.dim(join(ctx.dataDir, 'mcp.json'))}`);
		lines.push(`  ${colors.dim(join(ctx.dataDir, 'embed.json'))}`);
		lines.push(`  ${colors.dim(join(ctx.dataDir, 'memory.json'))}`);

		lines.push('');
		lines.push(colors.bold(colors.cyan('Workspace:')));
		lines.push(
			`  ${colors.bold('SIMSE.md:')} ${workspacePrompt ? colors.green('loaded') : colors.dim('not found')}`,
		);
		if (workspaceSettings.defaultAgent)
			lines.push(
				`  ${colors.bold('Agent:')}    ${workspaceSettings.defaultAgent}`,
			);
		if (workspaceSettings.systemPrompt)
			lines.push(
				`  ${colors.bold('System:')}   ${colors.dim(workspaceSettings.systemPrompt.slice(0, 60))}...`,
			);
		const promptNames = Object.keys(prompts);
		if (promptNames.length > 0)
			lines.push(
				`  ${colors.bold('Prompts:')}  ${promptNames.map((n) => colors.cyan(n)).join(', ')}`,
			);
		if (customAgents.length > 0)
			lines.push(
				`  ${colors.bold('Agents:')}   ${customAgents.map((a) => colors.cyan(a.name)).join(', ')}`,
			);

		lines.push('');
		lines.push(colors.bold(colors.cyan('ACP servers:')));
		for (const s of config.acp.servers) {
			lines.push(
				`  ${colors.bold(s.name)}: ${colors.dim(`${s.command} ${(s.args ?? []).join(' ')}`)}`,
			);
		}

		lines.push('');
		lines.push(colors.bold(colors.cyan('MCP servers:')));
		if (config.mcp.client.servers.length === 0) {
			lines.push(`  ${colors.dim('none')}`);
		}
		for (const s of config.mcp.client.servers) {
			lines.push(
				`  ${colors.bold(s.name)}: ${colors.dim(`${s.command ?? ''} ${(s.args ?? []).join(' ')}`)}`,
			);
		}

		if (skippedServers.length > 0) {
			lines.push('');
			lines.push(colors.bold(colors.yellow('Skipped (missing keys):')));
			for (const s of skippedServers) {
				lines.push(
					`  ${colors.bold(s.name)}: ${colors.dim(s.missingEnv.join(', '))}`,
				);
			}
		}

		const { embedConfig } = ctx.configResult;
		lines.push('');
		lines.push(colors.bold(colors.cyan('Embedding:')));
		lines.push(
			`  ${colors.bold('Model:')} ${embedConfig.embeddingModel ?? colors.dim('nomic-ai/nomic-embed-text-v1.5')}`,
		);
		lines.push(
			`  ${colors.bold('Dtype:')} ${embedConfig.dtype ?? colors.dim('q8')}`,
		);

		lines.push('');
		lines.push(colors.bold(colors.cyan('Memory:')));
		lines.push(`  ${colors.bold('Enabled:')}     ${config.memory.enabled}`);
		lines.push(
			`  ${colors.bold('Threshold:')}   ${config.memory.similarityThreshold}`,
		);
		lines.push(`  ${colors.bold('Max results:')} ${config.memory.maxResults}`);

		return lines.join('\n');
	},
};

// ---------------------------------------------------------------------------
// Session commands
// ---------------------------------------------------------------------------

const initCommand: Command = {
	name: 'init',
	usage: '/init',
	description: 'Initialize global ~/.simse config',
	category: 'session',
	handler: async (ctx) => {
		const { colors } = ctx;
		const result = await runSetup({ dataDir: ctx.dataDir, rl: ctx.rl });
		if (result.filesCreated.length === 0)
			return colors.dim('All global config files already exist.');
		return `${colors.green('✓')} Created: ${result.filesCreated.join(', ')}\n${colors.dim('Restart the CLI to apply changes.')}`;
	},
};

// ---------------------------------------------------------------------------
// Settings command — interactive config editor
// ---------------------------------------------------------------------------

function settingsAsk(rl: ReadlineInterface, question: string): Promise<string> {
	return new Promise((resolve) => {
		rl.question(question, resolve);
	});
}

function readJsonSafe<T>(path: string): T | undefined {
	if (!existsSync(path)) return undefined;
	try {
		return JSON.parse(readFileSync(path, 'utf-8')) as T;
	} catch {
		return undefined;
	}
}

function writeJsonSafe(path: string, data: unknown): void {
	writeFileSync(path, `${JSON.stringify(data, null, '\t')}\n`, 'utf-8');
}

async function settingsMenu(ctx: AppContext): Promise<string | undefined> {
	const { colors, rl, dataDir } = ctx;

	const sections = [
		{ key: '1', label: 'Embedding', file: 'embed.json' },
		{ key: '2', label: 'Memory', file: 'memory.json' },
		{ key: '3', label: 'MCP servers', file: 'mcp.json' },
		{ key: '4', label: 'General config', file: 'config.json' },
		{ key: '5', label: 'ACP servers', file: 'acp.json' },
	] as const;

	console.log('');
	console.log(colors.bold(colors.cyan('Settings')));
	console.log('');
	for (const s of sections) {
		const filePath = join(dataDir, s.file);
		const exists = existsSync(filePath);
		const status = exists ? colors.green('✓') : colors.dim('—');
		console.log(
			`  ${colors.bold(s.key)}) ${status} ${s.label} ${colors.dim(s.file)}`,
		);
	}
	console.log(
		`\n  ${colors.dim('Enter number to edit, or "back" to return.')}`,
	);

	const choice = (
		await settingsAsk(rl, `\n${colors.cyan('settings')}${colors.dim('>')} `)
	)
		.trim()
		.toLowerCase();

	if (choice === 'back' || choice === 'q' || choice === '') return undefined;

	switch (choice) {
		case '1':
			return editEmbedSettings(ctx);
		case '2':
			return editMemorySettings(ctx);
		case '3':
			return editMCPSettings(ctx);
		case '4':
			return editGeneralSettings(ctx);
		case '5':
			return editACPSettings(ctx);
		default:
			return `${colors.yellow('Unknown option.')} Enter 1-5 or "back".`;
	}
}

async function editEmbedSettings(ctx: AppContext): Promise<string> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'embed.json');

	type EmbedConfig = Record<string, unknown>;
	const config: EmbedConfig = readJsonSafe<EmbedConfig>(filePath) ?? {};

	console.log(
		`\n${colors.bold('Embedding settings')} ${colors.dim(filePath)}\n`,
	);

	const fields = [
		{
			key: 'embeddingModel',
			label: 'HF model ID',
			type: 'str' as const,
		},
		{
			key: 'dtype',
			label: 'Quantization (fp32/fp16/q8/q4)',
			type: 'str' as const,
		},
	];

	return editFields(rl, colors, filePath, config, fields);
}

async function editMemorySettings(ctx: AppContext): Promise<string> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'memory.json');

	type MemConfig = Record<string, unknown>;
	const config: MemConfig = readJsonSafe<MemConfig>(filePath) ?? {};

	console.log(`\n${colors.bold('Memory settings')} ${colors.dim(filePath)}\n`);

	const fields = [
		{ key: 'enabled', label: 'Enabled', type: 'bool' as const },
		{
			key: 'similarityThreshold',
			label: 'Similarity threshold (0-1)',
			type: 'num' as const,
		},
		{
			key: 'maxResults',
			label: 'Max search results',
			type: 'int' as const,
		},
		{
			key: 'storageFilename',
			label: 'Storage filename',
			type: 'str' as const,
		},
		{ key: 'autoSave', label: 'Auto-save', type: 'bool' as const },
		{
			key: 'duplicateThreshold',
			label: 'Duplicate threshold (0-1)',
			type: 'num' as const,
		},
		{
			key: 'duplicateBehavior',
			label: 'Duplicate behavior (skip/warn/error)',
			type: 'str' as const,
		},
		{
			key: 'autoSummarizeThreshold',
			label: 'Auto-summarize threshold (0 = disabled)',
			type: 'num' as const,
		},
	];

	return editFields(rl, colors, filePath, config, fields);
}

async function editMCPSettings(ctx: AppContext): Promise<string> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'mcp.json');

	type McpConfig = { servers?: Array<Record<string, unknown>> };
	const config: McpConfig = readJsonSafe<McpConfig>(filePath) ?? {
		servers: [],
	};
	const servers = config.servers ?? [];

	console.log(`\n${colors.bold('MCP servers')} ${colors.dim(filePath)}\n`);

	if (servers.length > 0) {
		for (let i = 0; i < servers.length; i++) {
			const s = servers[i];
			console.log(
				`  ${colors.bold(`${i + 1})`)} ${colors.bold(String(s.name ?? 'unnamed'))} ${colors.dim(`— ${s.command ?? ''} ${Array.isArray(s.args) ? (s.args as string[]).join(' ') : ''}`)}`,
			);
		}
	} else {
		console.log(colors.dim('  No MCP servers configured.'));
	}

	console.log(`\n  ${colors.dim('[a]dd / [r]emove <num> / [b]ack')}`);
	const choice = (await settingsAsk(rl, `  ${colors.dim('>')} `))
		.trim()
		.toLowerCase();

	if (choice === 'a' || choice === 'add') {
		const name = await askSettingsRequired(rl, '  Name: ');
		const command = await askSettingsRequired(
			rl,
			'  Command (e.g. bunx, npx, node): ',
		);
		const argsStr = (
			await settingsAsk(rl, '  Args (space-separated, enter to skip): ')
		).trim();
		const args = argsStr ? argsStr.split(/\s+/) : undefined;

		const envStr = (
			await settingsAsk(
				rl,
				'  Required env vars (space-separated, enter to skip): ',
			)
		).trim();
		const requiredEnv = envStr ? envStr.split(/\s+/) : undefined;

		const server: Record<string, unknown> = {
			name,
			transport: 'stdio',
			command,
			...(args && { args }),
			...(requiredEnv && { requiredEnv }),
		};

		servers.push(server);
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Added MCP server "${name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('r') || choice.startsWith('remove')) {
		const numStr = choice.replace(/^r(emove)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return `${colors.yellow('Invalid server number.')}`;
		}
		const removed = servers.splice(idx, 1)[0];
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Removed "${removed.name}". ${colors.dim('Restart to apply.')}`;
	}

	return '';
}

async function editACPSettings(ctx: AppContext): Promise<string> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'acp.json');

	type AcpConfig = {
		servers?: Array<Record<string, unknown>>;
		defaultServer?: string;
		defaultAgent?: string;
	};
	const config: AcpConfig = readJsonSafe<AcpConfig>(filePath) ?? {
		servers: [],
	};
	const servers = config.servers ?? [];

	console.log(`\n${colors.bold('ACP servers')} ${colors.dim(filePath)}\n`);

	if (servers.length > 0) {
		for (let i = 0; i < servers.length; i++) {
			const s = servers[i];
			const isDefault = config.defaultServer === s.name;
			const marker = isDefault ? colors.green(' (default)') : '';
			console.log(
				`  ${colors.bold(`${i + 1})`)} ${colors.bold(String(s.name ?? 'unnamed'))}${marker} ${colors.dim(`— ${s.command ?? ''} ${Array.isArray(s.args) ? (s.args as string[]).join(' ') : ''}`)}`,
			);
		}
	} else {
		console.log(colors.dim('  No ACP servers configured.'));
	}

	console.log(
		`\n  ${colors.dim('[a]dd / [r]emove <num> / [d]efault <num> / [b]ack')}`,
	);
	const choice = (await settingsAsk(rl, `  ${colors.dim('>')} `))
		.trim()
		.toLowerCase();

	if (choice === 'a' || choice === 'add') {
		const name = await askSettingsRequired(rl, '  Name: ');
		const command = await askSettingsRequired(
			rl,
			'  Command (e.g. bun, node): ',
		);
		const argsStr = (
			await settingsAsk(rl, '  Args (space-separated, enter to skip): ')
		).trim();
		const args = argsStr ? argsStr.split(/\s+/) : undefined;

		const server: Record<string, unknown> = {
			name,
			command,
			...(args && { args }),
		};

		servers.push(server);
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Added ACP server "${name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('r') || choice.startsWith('remove')) {
		const numStr = choice.replace(/^r(emove)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return `${colors.yellow('Invalid server number.')}`;
		}
		const removed = servers.splice(idx, 1)[0];
		config.servers = servers;
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Removed "${removed.name}". ${colors.dim('Restart to apply.')}`;
	}

	if (choice.startsWith('d') || choice.startsWith('default')) {
		const numStr = choice.replace(/^d(efault)?\s*/, '').trim();
		const idx = Number.parseInt(numStr, 10) - 1;
		if (Number.isNaN(idx) || idx < 0 || idx >= servers.length) {
			return `${colors.yellow('Invalid server number.')}`;
		}
		config.defaultServer = String(servers[idx].name);
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Default server set to "${servers[idx].name}". ${colors.dim('Restart to apply.')}`;
	}

	return '';
}

async function editGeneralSettings(ctx: AppContext): Promise<string> {
	const { colors, rl, dataDir } = ctx;
	const filePath = join(dataDir, 'config.json');

	type GenConfig = Record<string, unknown>;
	const config: GenConfig = readJsonSafe<GenConfig>(filePath) ?? {};

	console.log(`\n${colors.bold('General config')} ${colors.dim(filePath)}\n`);

	const fields = [
		{ key: 'defaultAgent', label: 'Default agent ID', type: 'str' as const },
		{
			key: 'logLevel',
			label: 'Log level (debug/info/warn/error/none)',
			type: 'str' as const,
		},
	];

	return editFields(rl, colors, filePath, config, fields);
}

// -- Generic field editor -----------------------------------------------------

async function editFields(
	rl: ReadlineInterface,
	colors: TermColors,
	filePath: string,
	config: Record<string, unknown>,
	fields: readonly {
		key: string;
		label: string;
		type: 'str' | 'num' | 'int' | 'bool';
	}[],
): Promise<string> {
	let changed = false;

	for (const field of fields) {
		const current = config[field.key];
		const currentStr =
			current !== undefined && current !== null ? String(current) : '';
		const displayCurrent = currentStr
			? colors.cyan(currentStr)
			: colors.dim('not set');

		const answer = (
			await settingsAsk(rl, `  ${field.label} [${displayCurrent}]: `)
		).trim();

		if (!answer) continue; // keep current
		if (answer === '-') {
			// Clear the field
			delete config[field.key];
			changed = true;
			continue;
		}

		switch (field.type) {
			case 'bool':
				config[field.key] =
					answer === 'true' || answer === 'yes' || answer === 'y';
				changed = true;
				break;
			case 'num': {
				const num = Number(answer);
				if (!Number.isNaN(num)) {
					config[field.key] = num;
					changed = true;
				}
				break;
			}
			case 'int': {
				const num = Number.parseInt(answer, 10);
				if (!Number.isNaN(num)) {
					config[field.key] = num;
					changed = true;
				}
				break;
			}
			default:
				config[field.key] = answer;
				changed = true;
				break;
		}
	}

	if (changed) {
		writeJsonSafe(filePath, config);
		return `${colors.green('✓')} Saved ${colors.dim(filePath)}. ${colors.dim('Restart to apply.')}`;
	}

	return colors.dim('No changes.');
}

async function askSettingsRequired(
	rl: ReadlineInterface,
	question: string,
): Promise<string> {
	while (true) {
		const answer = (await settingsAsk(rl, question)).trim();
		if (answer) return answer;
		console.log('  This field is required.');
	}
}

const settingsCommand: Command = {
	name: 'settings',
	aliases: ['set'],
	usage: '/settings',
	description: 'Edit configuration (memory, MCP, ACP, project)',
	category: 'session',
	handler: async (ctx) => {
		return settingsMenu(ctx);
	},
};

const serverCommand: Command = {
	name: 'server',
	usage: '/server [name]',
	description: 'Show or set the active ACP server',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors } = ctx;
		const name = rest.trim();
		if (!name) {
			const current = ctx.session.serverName ?? 'default';
			const available = ctx.configResult.config.acp.servers
				.map((s) => s.name)
				.join(', ');
			return `${colors.bold('Server:')} ${current}\n${colors.dim(`Available: ${available}`)}`;
		}
		const found = ctx.configResult.config.acp.servers.find(
			(s) => s.name === name,
		);
		if (!found)
			return `${colors.red('✗')} Server "${name}" not found. Available: ${ctx.configResult.config.acp.servers.map((s) => s.name).join(', ')}`;
		ctx.session.serverName = name;
		return `${colors.green('✓')} Server set to ${colors.bold(name)}`;
	},
};

const agentCommand: Command = {
	name: 'agent',
	usage: '/agent [name|clear]',
	description: 'Show or set the active agent persona',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors } = ctx;
		const name = rest.trim();
		if (!name) {
			const current = ctx.session.agentName ?? 'none';
			const available = ctx.configResult.agents.map((a) => a.name).join(', ');
			return `${colors.bold('Agent:')} ${current}\n${colors.dim(`Available: ${available || 'none'}`)}`;
		}
		if (name === 'clear' || name === 'none') {
			ctx.session.agentName = undefined;
			return `${colors.green('✓')} Agent cleared`;
		}
		const agent = ctx.configResult.agents.find((a) => a.name === name);
		if (!agent)
			return `${colors.red('✗')} Agent "${name}" not found. Use ${colors.cyan('"/agents"')} to list available agents.`;
		ctx.session.agentName = name;
		return `${colors.green('✓')} Agent set to ${colors.bold(name)}`;
	},
};

const memoryCommand: Command = {
	name: 'memory',
	usage: '/memory [on|off]',
	description: 'Show or toggle memory for AI responses',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors } = ctx;
		const arg = rest.trim().toLowerCase();
		if (!arg) {
			const status = ctx.session.memoryEnabled ? 'on' : 'off';
			return `${colors.bold('Memory:')} ${status}`;
		}
		if (arg === 'on' || arg === 'enable') {
			ctx.session.memoryEnabled = true;
			return `${colors.green('✓')} Memory enabled`;
		}
		if (arg === 'off' || arg === 'disable') {
			ctx.session.memoryEnabled = false;
			return `${colors.green('✓')} Memory disabled`;
		}
		return `${colors.yellow('Usage:')} /memory [on|off]`;
	},
};

const bypassPermissionsCommand: Command = {
	name: 'bypass-permissions',
	usage: '/bypass-permissions',
	description: 'Auto-approve all ACP permission requests for this session',
	category: 'session',
	handler: (ctx) => {
		const { colors, session } = ctx;
		if (session.bypassPermissions) {
			return `${colors.dim('Permissions are already bypassed for this session.')}`;
		}
		session.bypassPermissions = true;
		ctx.acpClient.setPermissionPolicy('auto-approve');
		return `${colors.green('✓')} Permission bypass enabled — all tool use and edit requests will be auto-approved.`;
	},
};

const skillsCommand: Command = {
	name: 'skills',
	usage: '/skills',
	description: 'List available skills',
	category: 'ai',
	handler: (ctx) => {
		const { colors } = ctx;
		const skills = ctx.skillRegistry.getAll();
		if (skills.length === 0) {
			return colors.dim(
				'No skills found. Create .simse/skills/<name>/SKILL.md to add one.',
			);
		}

		const lines: string[] = [colors.bold(colors.cyan('Available skills:')), ''];
		for (const skill of skills) {
			const hint = skill.argumentHint ? ` ${skill.argumentHint}` : '';
			lines.push(
				`  ${colors.cyan(`/${skill.name}${hint}`)}  ${colors.dim(skill.description || 'No description')}`,
			);
		}
		return lines.join('\n');
	},
};

const compactCommand: Command = {
	name: 'compact',
	usage: '/compact [instructions]',
	description: 'Summarize conversation and reset context',
	category: 'session',
	handler: async (ctx, rest) => {
		const { colors, spinner, conversation } = ctx;

		if (conversation.messageCount === 0) {
			return colors.dim('No conversation to compact.');
		}

		const instructions = rest.trim();
		const focus = instructions ? `Focus on: ${instructions}\n\n` : '';

		spinner.start('Compacting conversation...');
		try {
			const conversationText = conversation.serialize();
			const prompt = `${focus}Summarize the following conversation concisely, preserving key decisions, code changes, and context needed for future turns:\n\n${conversationText}`;
			const result = await ctx.app.generate(prompt, { skipMemory: true });
			const prevCount = conversation.messageCount;
			conversation.compact(result.content);
			spinner.succeed(`Compacted ${prevCount} messages into summary`);
			return undefined;
		} catch (err) {
			spinner.fail(
				`Compaction failed: ${err instanceof Error ? err.message : err}`,
			);
			return undefined;
		}
	},
};

const costCommand: Command = {
	name: 'cost',
	usage: '/cost',
	description: 'Show session usage statistics',
	category: 'info',
	handler: (ctx) => {
		const { colors, session, conversation } = ctx;
		const chars = conversation.estimatedChars;
		const approxTokens = Math.round(chars / 4);
		const elapsed = Math.round((Date.now() - session.startedAt) / 1000);
		const hours = Math.floor(elapsed / 3600);
		const minutes = Math.floor((elapsed % 3600) / 60);
		const seconds = elapsed % 60;
		const wallParts: string[] = [];
		if (hours > 0) wallParts.push(`${hours}h`);
		if (minutes > 0) wallParts.push(`${minutes}m`);
		wallParts.push(`${seconds}s`);
		const wallDuration = wallParts.join(' ');

		const server =
			session.serverName ?? ctx.acpClient.defaultServerName ?? 'default';
		const agent = session.agentName ?? ctx.acpClient.defaultAgent ?? 'default';

		const lines: string[] = [];
		lines.push(
			`  ${colors.bold('Total turns:'.padEnd(22))}${session.totalTurns}`,
		);
		lines.push(`  ${colors.bold('Total duration:'.padEnd(22))}${wallDuration}`);
		lines.push(
			`  ${colors.bold('Context:'.padEnd(22))}~${approxTokens.toLocaleString()} tokens (${conversation.messageCount} messages)`,
		);
		lines.push(
			`  ${colors.bold('Memory:'.padEnd(22))}${session.memoryEnabled ? 'enabled' : 'disabled'} (${ctx.app.noteCount} notes)`,
		);
		lines.push(`  ${colors.bold('Model:'.padEnd(22))}${agent} via ${server}`);
		return lines.join('\n');
	},
};

const modelCommand: Command = {
	name: 'model',
	usage: '/model [agent]',
	description: 'Show or set the active ACP agent',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors, session } = ctx;
		const name = rest.trim();
		if (!name) {
			const current =
				session.agentName ?? ctx.acpClient.defaultAgent ?? 'default';
			const server =
				session.serverName ?? ctx.acpClient.defaultServerName ?? 'default';
			return `  ${colors.bold('Agent:')}  ${current}\n  ${colors.bold('Server:')} ${server}`;
		}
		session.agentName = name;
		return `${colors.green('✓')} Agent set to ${colors.bold(name)}`;
	},
};

const turnsCommand: Command = {
	name: 'turns',
	usage: '/turns [n]',
	description: 'Show or set the max agentic turns per interaction',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors, session } = ctx;
		const arg = rest.trim();
		if (!arg) {
			return `${colors.bold('Max turns:')} ${session.maxTurns}`;
		}
		const n = Number.parseInt(arg, 10);
		if (Number.isNaN(n) || n < 1 || n > 100) {
			return `${colors.yellow('Usage:')} /turns <1-100>`;
		}
		session.maxTurns = n;
		return `${colors.green('✓')} Max turns set to ${colors.bold(String(n))}`;
	},
};

const clearCommand: Command = {
	name: 'clear',
	usage: '/clear',
	description: 'Clear conversation history',
	category: 'session',
	handler: (ctx) => {
		ctx.conversation.clear();
		return renderDetailLine('(no content)', ctx.colors);
	},
};

const contextCommand: Command = {
	name: 'context',
	usage: '/context',
	description: 'Show context window usage with visual grid',
	category: 'info',
	handler: (ctx) => {
		const { colors, conversation } = ctx;
		const chars = conversation.estimatedChars;
		const maxChars = 100_000; // approximate budget

		// Use the 40x2 grid visualization
		const grid = renderContextGrid(chars, maxChars, colors);
		const lines: string[] = [grid];

		lines.push('');
		const approxTokens = Math.round(chars / 4);
		const maxTokens = Math.round(maxChars / 4);
		lines.push(`  ${colors.dim('Tokens:')}   ~${approxTokens} / ~${maxTokens}`);
		lines.push(`  ${colors.dim('Messages:')} ${conversation.messageCount}`);

		if (conversation.needsCompaction) {
			lines.push('');
			lines.push(
				`  ${colors.yellow('⚠')} Context is large. Run ${colors.cyan('/compact')} to reduce.`,
			);
		}

		return lines.join('\n');
	},
};

const exportCommand: Command = {
	name: 'export',
	usage: '/export [filename]',
	description: 'Export conversation to file or stdout',
	category: 'session',
	handler: (ctx, rest) => {
		const { colors, conversation } = ctx;
		if (conversation.messageCount === 0) {
			return colors.dim('No conversation to export.');
		}

		const text = conversation.serialize();
		const filename = rest.trim();

		if (filename) {
			try {
				writeFileSync(filename, `${text}\n`, 'utf-8');
				return `${colors.green('✓')} Exported ${conversation.messageCount} messages to ${colors.dim(filename)}`;
			} catch (err) {
				return `${colors.red('✗')} Failed to write: ${err instanceof Error ? err.message : err}`;
			}
		}

		// No filename — print to stdout
		return text;
	},
};

const copyCommand: Command = {
	name: 'copy',
	usage: '/copy',
	description: 'Copy last assistant response to clipboard',
	category: 'session',
	handler: (ctx) => {
		const { colors, conversation } = ctx;
		const messages = conversation.toMessages();
		const lastAssistant = [...messages]
			.reverse()
			.find((m) => m.role === 'assistant');
		if (!lastAssistant) {
			return colors.dim('No assistant response to copy.');
		}

		try {
			const platform = process.platform;
			if (platform === 'darwin') {
				execFileSync('pbcopy', [], { input: lastAssistant.content });
			} else if (platform === 'win32') {
				execFileSync('clip', [], { input: lastAssistant.content });
			} else {
				// Linux: try xclip, then xsel
				try {
					execFileSync('xclip', ['-selection', 'clipboard'], {
						input: lastAssistant.content,
					});
				} catch {
					execFileSync('xsel', ['--clipboard', '--input'], {
						input: lastAssistant.content,
					});
				}
			}
			return `${colors.green('✓')} Copied to clipboard`;
		} catch {
			return `${colors.red('✗')} Failed to copy — clipboard tool not available`;
		}
	},
};

const doctorCommand: Command = {
	name: 'doctor',
	usage: '/doctor',
	description: 'Health check of installation and services',
	category: 'info',
	handler: async (ctx) => {
		const { colors, acpClient, app } = ctx;
		const lines: string[] = [colors.bold(colors.cyan('Health Check')), ''];

		// ACP servers
		for (const name of acpClient.serverNames) {
			try {
				const available = await acpClient.isAvailable(name);
				if (available) {
					lines.push(
						renderServiceStatus('ACP', 'ok', `${name} — connected`, colors),
					);
				} else {
					lines.push(
						renderServiceStatus(
							'ACP',
							'fail',
							`${name} — not responding`,
							colors,
						),
					);
				}
			} catch {
				lines.push(
					renderServiceStatus(
						'ACP',
						'fail',
						`${name} — connection error`,
						colors,
					),
				);
			}
		}

		// MCP connections
		const mcpCount = app.tools.mcpClient.connectionCount;
		if (mcpCount > 0) {
			lines.push(
				renderServiceStatus(
					'MCP',
					'ok',
					`${mcpCount} server(s) connected`,
					colors,
				),
			);
		} else {
			lines.push(
				renderServiceStatus('MCP', 'warn', 'no servers connected', colors),
			);
		}

		// Memory
		lines.push(
			renderServiceStatus(
				'Memory',
				'ok',
				`${app.noteCount} notes stored`,
				colors,
			),
		);

		// Context usage
		const chars = ctx.conversation.estimatedChars;
		const approxTokens = Math.round(chars / 4);
		lines.push(
			renderServiceStatus(
				'Context',
				ctx.conversation.needsCompaction ? 'warn' : 'ok',
				`~${approxTokens} tokens, ${ctx.conversation.messageCount} messages`,
				colors,
			),
		);

		// Data directory
		if (existsSync(ctx.dataDir)) {
			lines.push(renderServiceStatus('Data', 'ok', ctx.dataDir, colors));
		} else {
			lines.push(
				renderServiceStatus('Data', 'fail', `${ctx.dataDir} — missing`, colors),
			);
		}

		return lines.join('\n');
	},
};

const helpCommand: Command = {
	name: 'help',
	aliases: ['?'],
	usage: '/help',
	description: 'Show available commands',
	category: 'session',
	handler: (ctx) => {
		const { colors } = ctx;
		const lines: string[] = [];

		// Render built-in commands
		lines.push(
			renderHelp(
				commands.map((c) => ({
					name: c.name,
					aliases: c.aliases,
					usage: c.usage,
					description: c.description,
					category: c.category,
				})),
				categoryLabels,
				colors,
			),
		);

		// Append skills if any
		const skills = ctx.skillRegistry.getAll();
		if (skills.length > 0) {
			lines.push('');
			lines.push(colors.bold(colors.cyan('Skills:')));
			for (const skill of skills) {
				const hint = skill.argumentHint ? ` ${skill.argumentHint}` : '';
				lines.push(
					`  ${colors.white(`/${skill.name}${hint}`.padEnd(30))} ${colors.dim(skill.description || 'No description')}`,
				);
			}
		}

		lines.push('');
		lines.push(
			colors.dim(
				'  Type a message to chat with the AI. Use \\ for multi-line, ! for shell.',
			),
		);
		lines.push(
			colors.dim('  Press Esc to interrupt generation. Ctrl+C to cancel.'),
		);
		return lines.join('\n');
	},
};

const permissionsCommand: Command = {
	name: 'permissions',
	usage: '/permissions',
	description: 'Show current permission state',
	category: 'info',
	handler: (ctx) => {
		const { colors, session } = ctx;
		const lines: string[] = [colors.bold(colors.cyan('Permissions')), ''];

		lines.push(
			`  ${colors.bold('Auto-approve:'.padEnd(18))}${session.bypassPermissions ? colors.yellow('enabled') : 'disabled'}`,
		);
		lines.push(
			`  ${colors.bold('Memory:'.padEnd(18))}${session.memoryEnabled ? 'enabled' : 'disabled'}`,
		);
		lines.push(`  ${colors.bold('Max turns:'.padEnd(18))}${session.maxTurns}`);

		// MCP server connections
		const mcpCount = ctx.app.tools.mcpClient.connectionCount;
		if (mcpCount > 0) {
			lines.push('');
			lines.push(colors.bold(colors.cyan('Connected MCP servers:')));
			for (const name of ctx.app.tools.mcpClient.connectedServerNames) {
				lines.push(`  ${colors.green('✓')} ${name}`);
			}
		}

		return lines.join('\n');
	},
};

const retryCommand: Command = {
	name: 'retry',
	usage: '/retry',
	description: 'Re-send the last user message',
	category: 'session',
	handler: (ctx) => {
		const { colors, session } = ctx;
		if (!session.lastUserInput) {
			return colors.dim('No previous message to retry.');
		}
		// Re-invoke the bare text handler with the last input
		return handleBareTextInput(ctx, session.lastUserInput);
	},
};

const exitCommand: Command = {
	name: 'exit',
	aliases: ['quit', 'q'],
	usage: '/exit',
	description: 'Exit the application',
	category: 'session',
	handler: () => null,
};

// ---------------------------------------------------------------------------
// VFS commands
// ---------------------------------------------------------------------------

function askUser(rl: ReadlineInterface, question: string): Promise<string> {
	return new Promise((resolve) => {
		rl.question(question, resolve);
	});
}

function displayValidation(ctx: AppContext): boolean {
	const { colors } = ctx;
	const snap = ctx.vfs.snapshot();
	const validation = validateSnapshot(snap, createDefaultValidators());

	if (validation.issues.length === 0) return true;

	console.log(
		`\n${colors.bold('Validation:')} ${colors.red(`${validation.errors} error(s)`)}, ${colors.yellow(`${validation.warnings} warning(s)`)}`,
	);
	for (const issue of validation.issues) {
		const lineStr = issue.line ? `:${issue.line}` : '';
		const prefix =
			issue.severity === 'error'
				? `  ${colors.red('ERROR')}`
				: `  ${colors.yellow('WARN ')}`;
		console.log(
			`${prefix} ${issue.path}${colors.dim(lineStr)}: ${issue.message}`,
		);
	}

	return validation.passed;
}

async function promptKeepOrDiscard(
	ctx: AppContext,
): Promise<string | undefined> {
	const { colors } = ctx;
	if (ctx.vfs.fileCount === 0) return undefined;

	while (true) {
		const answer = (
			await askUser(
				ctx.rl,
				`\n${colors.bold(`${ctx.vfs.fileCount}`)} file(s) in sandbox. ${colors.dim('[s]ave / [d]iscard / [r]eview?')} `,
			)
		)
			.trim()
			.toLowerCase();

		if (answer === 'r' || answer === 'review') {
			console.log(ctx.vfs.tree());
			continue;
		}

		if (
			answer === 's' ||
			answer === 'save' ||
			answer === 'y' ||
			answer === 'yes'
		) {
			const passed = displayValidation(ctx);
			if (!passed) {
				const proceed = (
					await askUser(
						ctx.rl,
						`${colors.yellow('Validation errors found.')} Save anyway? ${colors.dim('[y/n]')} `,
					)
				)
					.trim()
					.toLowerCase();
				if (proceed !== 'y' && proceed !== 'yes') continue;
			}
			const result = await ctx.disk.commit(undefined, { overwrite: true });
			ctx.vfs.clear();
			return `${colors.green('✓')} Saved ${colors.bold(`${result.filesWritten}`)} file(s), ${result.directoriesCreated} dir(s) ${colors.dim(`(${result.bytesWritten} bytes)`)}`;
		}

		if (
			answer === 'd' ||
			answer === 'discard' ||
			answer === 'n' ||
			answer === 'no'
		) {
			const count = ctx.vfs.fileCount;
			ctx.vfs.clear();
			return `${colors.yellow('⚠')} Discarded ${colors.bold(`${count}`)} file(s)`;
		}
	}
}

const filesCommand: Command = {
	name: 'files',
	aliases: ['vfs'],
	usage: '/files',
	description: 'Show files in VFS sandbox',
	category: 'session',
	handler: (ctx) => {
		if (ctx.vfs.fileCount === 0) return ctx.colors.dim('No files in sandbox.');
		return ctx.vfs.tree();
	},
};

const saveCommand: Command = {
	name: 'save',
	usage: '/save [--force]',
	description: 'Write all sandbox files to disk (validates first)',
	category: 'session',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (ctx.vfs.fileCount === 0) return colors.dim('No files to save.');

		const force = rest.trim() === '--force';

		if (!force) {
			const passed = displayValidation(ctx);
			if (!passed) {
				const proceed = (
					await askUser(
						ctx.rl,
						`${colors.yellow('Validation errors found.')} Save anyway? ${colors.dim('[y/n]')} `,
					)
				)
					.trim()
					.toLowerCase();
				if (proceed !== 'y' && proceed !== 'yes') {
					return colors.dim('Save cancelled.');
				}
			}
		}

		const result = await ctx.disk.commit(undefined, { overwrite: true });
		ctx.vfs.clear();
		return `${colors.green('✓')} Saved ${colors.bold(`${result.filesWritten}`)} file(s), ${result.directoriesCreated} dir(s) ${colors.dim(`(${result.bytesWritten} bytes)`)}`;
	},
};

const validateCommand: Command = {
	name: 'validate',
	aliases: ['check'],
	usage: '/validate',
	description: 'Validate sandbox files for formatting/syntax issues',
	category: 'session',
	handler: (ctx) => {
		const { colors } = ctx;
		if (ctx.vfs.fileCount === 0) return colors.dim('No files to validate.');

		const snap = ctx.vfs.snapshot();
		const validation = validateSnapshot(snap, createDefaultValidators());

		if (validation.issues.length === 0) {
			return `${colors.green('✓')} All ${colors.bold(`${ctx.vfs.fileCount}`)} file(s) passed validation.`;
		}

		const lines: string[] = [
			`${colors.red(`${validation.errors} error(s)`)}, ${colors.yellow(`${validation.warnings} warning(s)`)}:`,
		];
		for (const issue of validation.issues) {
			const lineStr = issue.line ? `:${issue.line}` : '';
			const prefix =
				issue.severity === 'error'
					? `  ${colors.red('ERROR')}`
					: `  ${colors.yellow('WARN ')}`;
			lines.push(
				`${prefix} ${issue.path}${colors.dim(lineStr)}: ${issue.message}`,
			);
		}
		return lines.join('\n');
	},
};

const discardCommand: Command = {
	name: 'discard',
	usage: '/discard',
	description: 'Discard all sandbox files',
	category: 'session',
	handler: (ctx) => {
		const { colors } = ctx;
		if (ctx.vfs.fileCount === 0) return colors.dim('No files to discard.');
		const count = ctx.vfs.fileCount;
		ctx.vfs.clear();
		return `${colors.yellow('⚠')} Discarded ${colors.bold(`${count}`)} file(s)`;
	},
};

// ---------------------------------------------------------------------------
// New feature commands
// ---------------------------------------------------------------------------

const themeCommand: Command = {
	name: 'theme',
	usage: '/theme [name]',
	description: 'Switch color theme',
	category: 'info',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (!ctx.themeManager) return colors.dim('Theme system not available.');
		if (rest) {
			if (ctx.themeManager.setActive(rest)) {
				return `${colors.green('✓')} Theme set to ${colors.cyan(rest)}`;
			}
			return `${colors.red('✗')} Unknown theme "${rest}". Available: ${ctx.themeManager.list().join(', ')}`;
		}
		const idx = await showPicker(
			ctx.themeManager.list().map((t) => ({ label: t })),
			ctx.rl,
			{ title: 'Select theme:', colors },
		);
		if (idx < 0) return colors.dim('Cancelled.');
		const name = ctx.themeManager.list()[idx];
		ctx.themeManager.setActive(name);
		return `${colors.green('✓')} Theme set to ${colors.cyan(name)}`;
	},
};

const resumeCommand: Command = {
	name: 'resume',
	usage: '/resume [id]',
	description: 'Resume a previous session',
	category: 'session',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (!ctx.sessionStore)
			return colors.dim('Session persistence not available.');
		const sessions = ctx.sessionStore.list();
		if (sessions.length === 0) return colors.dim('No saved sessions.');
		if (rest) {
			const session = ctx.sessionStore.load(rest);
			if (!session) return `${colors.red('✗')} Session "${rest}" not found.`;
			return `${colors.green('✓')} Session ${colors.cyan(rest)} loaded (${session.messages.length} messages)`;
		}
		const items = sessions.slice(0, 10).map((s) => ({
			label: formatSessionSummary(s, colors),
		}));
		const idx = await showPicker(items, ctx.rl, {
			title: 'Resume session:',
			colors,
		});
		if (idx < 0) return colors.dim('Cancelled.');
		const selected = sessions[idx];
		const session = ctx.sessionStore.load(selected.id);
		if (!session) return `${colors.red('✗')} Failed to load session.`;
		return `${colors.green('✓')} Session ${colors.cyan(selected.id)} loaded (${session.messages.length} messages)`;
	},
};

const planCommand: Command = {
	name: 'plan',
	usage: '/plan',
	description: 'Toggle plan mode (read-only)',
	category: 'session',
	handler: (ctx) => {
		const { colors } = ctx;
		if (!ctx.planMode) return colors.dim('Plan mode not available.');
		ctx.planMode.toggle();
		const badge = ctx.planMode.isActive
			? renderModeBadge('plan', colors)
			: colors.dim('off');
		return `  Plan mode: ${badge}`;
	},
};

const rewindCommand: Command = {
	name: 'rewind',
	usage: '/rewind [id]',
	description: 'Rewind to a checkpoint',
	category: 'session',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (!ctx.checkpointManager) return colors.dim('Checkpoints not available.');
		const checkpoints = ctx.checkpointManager.list();
		if (checkpoints.length === 0) return colors.dim('No checkpoints.');
		if (rest) {
			if (ctx.checkpointManager.rewind(rest)) {
				return `${colors.green('✓')} Rewound to checkpoint ${colors.cyan(rest)}`;
			}
			return `${colors.red('✗')} Checkpoint "${rest}" not found.`;
		}
		const items = checkpoints.map((cp) => ({
			label: `${cp.id} ${cp.label ?? ''} (${cp.messageCount} msgs)`,
			detail: new Date(cp.timestamp).toLocaleTimeString(),
		}));
		const idx = await showPicker(items, ctx.rl, {
			title: 'Rewind to:',
			colors,
		});
		if (idx < 0) return colors.dim('Cancelled.');
		const selected = checkpoints[idx];
		ctx.checkpointManager.rewind(selected.id);
		return `${colors.green('✓')} Rewound to ${colors.cyan(selected.id)}`;
	},
};

const statusCommand: Command = {
	name: 'status',
	usage: '/status',
	description: 'Show comprehensive status',
	category: 'info',
	handler: (ctx) => {
		const { colors, session, conversation } = ctx;
		const lines: string[] = [];

		lines.push(colors.bold(colors.cyan('Status')));
		lines.push('');
		lines.push(`  ${colors.bold('Version:'.padEnd(14))}1.0.0`);
		lines.push(
			`  ${colors.bold('Uptime:'.padEnd(14))}${formatUptime(Date.now() - session.startedAt)}`,
		);
		lines.push(
			`  ${colors.bold('Server:'.padEnd(14))}${session.serverName ?? colors.dim('default')}`,
		);
		lines.push(
			`  ${colors.bold('Agent:'.padEnd(14))}${session.agentName ?? colors.dim('default')}`,
		);
		lines.push(`  ${colors.bold('Turns:'.padEnd(14))}${session.totalTurns}`);
		lines.push(
			`  ${colors.bold('Messages:'.padEnd(14))}${conversation.messageCount}`,
		);

		// Context usage
		const chars = conversation.estimatedChars;
		const maxChars = 100_000;
		const pct = Math.min(Math.round((chars / maxChars) * 100), 100);
		const ctxColor =
			pct > 80 ? colors.red : pct > 50 ? colors.yellow : colors.green;
		lines.push(`  ${colors.bold('Context:'.padEnd(14))}${ctxColor(`${pct}%`)}`);

		// Memory
		lines.push(
			`  ${colors.bold('Memory:'.padEnd(14))}${session.memoryEnabled ? `${ctx.app.noteCount} notes` : colors.dim('disabled')}`,
		);

		// VFS
		lines.push(
			`  ${colors.bold('Sandbox:'.padEnd(14))}${ctx.vfs.fileCount} files`,
		);

		// Permission mode
		if (ctx.permissionMode) {
			lines.push(`  ${colors.bold('Mode:'.padEnd(14))}${ctx.permissionMode}`);
		}

		// MCP
		const mcpCount = ctx.app.tools.mcpClient.connectionCount;
		lines.push(`  ${colors.bold('MCP:'.padEnd(14))}${mcpCount} connections`);

		// ACP
		lines.push(
			`  ${colors.bold('ACP:'.padEnd(14))}${ctx.app.agents.client.serverCount} servers`,
		);

		return lines.join('\n');
	},
};

const todosCommand: Command = {
	name: 'todos',
	aliases: ['todo'],
	usage: '/todos [add|done|rm] [args]',
	description: 'Manage task list',
	category: 'session',
	handler: async (ctx, rest) => {
		const { colors } = ctx;
		if (!rest) {
			// List all todos
			const tasks = ctx.app.tasks.list();
			const items = tasks.map((t) => ({
				id: t.id,
				subject: t.subject,
				status: t.status,
				blockedBy: t.blockedBy?.filter((bid) => {
					const blocker = ctx.app.tasks.get(bid);
					return blocker && blocker.status !== 'completed';
				}),
			}));
			return renderTodoList(items, colors);
		}
		const parsed = parseTodoCommand(rest);
		if (!parsed)
			return `${colors.yellow('Usage:')} /todos [add|done|rm] <args>`;
		switch (parsed.action) {
			case 'add': {
				if (!parsed.args)
					return `${colors.yellow('Usage:')} /todos add <subject>`;
				const task = ctx.app.tasks.create({
					subject: parsed.args,
				});
				return `${colors.green('✓')} Added task ${colors.dim(`#${task.id}`)} ${task.subject}`;
			}
			case 'done': {
				if (!parsed.args) return `${colors.yellow('Usage:')} /todos done <id>`;
				const updated = ctx.app.tasks.update(parsed.args, {
					status: 'completed',
				});
				if (!updated)
					return `${colors.red('✗')} Task "${parsed.args}" not found.`;
				return `${colors.green('✓')} Completed ${colors.dim(`#${parsed.args}`)}`;
			}
			case 'rm': {
				if (!parsed.args) return `${colors.yellow('Usage:')} /todos rm <id>`;
				const deleted = ctx.app.tasks.update(parsed.args, {
					status: 'completed',
				});
				if (!deleted)
					return `${colors.red('✗')} Task "${parsed.args}" not found.`;
				return `${colors.green('✓')} Removed ${colors.dim(`#${parsed.args}`)}`;
			}
			default:
				return `${colors.yellow('Usage:')} /todos [add|done|rm] <args>`;
		}
	},
};

const diffCommand: Command = {
	name: 'diff',
	usage: '/diff',
	description: 'Show VFS file changes as unified diff',
	category: 'session',
	handler: (ctx) => {
		const { colors, vfs } = ctx;
		if (vfs.fileCount === 0) return colors.dim('No files in sandbox.');
		const files = vfs.listAll();
		const lines: string[] = [colors.bold(colors.cyan('File changes:')), ''];
		for (const entry of files) {
			if (entry.type === 'file') {
				const content = vfs.readFile(entry.path);
				const lineCount = content.text.split('\n').length;
				lines.push(
					`  ${colors.green('+')} ${entry.path} ${colors.dim(`(${lineCount} lines)`)}`,
				);
			}
		}
		if (ctx.fileTracker) {
			const totals = ctx.fileTracker.getTotals();
			if (totals.additions > 0 || totals.deletions > 0) {
				lines.push('');
				lines.push(
					`  ${renderChangeCount(totals.additions, totals.deletions, colors)}`,
				);
			}
		}
		return lines.join('\n');
	},
};

const verboseCommand: Command = {
	name: 'verbose',
	usage: '/verbose',
	description: 'Toggle verbose mode',
	category: 'info',
	handler: (ctx) => {
		const { colors } = ctx;
		if (!ctx.verbose) return colors.dim('Verbose mode not available.');
		ctx.verbose.toggle();
		const badge = ctx.verbose.isVerbose
			? renderModeBadge('verbose', colors)
			: colors.dim('off');
		return `  Verbose: ${badge}`;
	},
};

const hooksCommand: Command = {
	name: 'hooks',
	usage: '/hooks',
	description: 'List configured hooks',
	category: 'tools',
	handler: (ctx) => {
		const { colors, dataDir } = ctx;
		const manager = createHooksManager({ dataDir });
		return renderHooksList(manager.list(), colors);
	},
};

const bgTasksCommand: Command = {
	name: 'tasks',
	usage: '/tasks',
	description: 'List background tasks',
	category: 'session',
	handler: (ctx) => {
		const { colors } = ctx;
		if (!ctx.backgroundManager)
			return colors.dim('Background tasks not available.');
		return renderBackgroundTasks(ctx.backgroundManager.list(), colors);
	},
};

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

const commands: readonly Command[] = [
	addCommand,
	searchCommand,
	recommendCommand,
	topicsCommand,
	notesCommand,
	getCommand,
	deleteCommand,
	chainCommand,
	promptsCommand,
	toolsCommand,
	agentsCommand,
	skillsCommand,
	mcpCommand,
	acpCommand,
	embedCommand,
	learningCommand,
	statsCommand,
	costCommand,
	contextCommand,
	permissionsCommand,
	configCommand,
	filesCommand,
	validateCommand,
	saveCommand,
	discardCommand,
	settingsCommand,
	initCommand,
	serverCommand,
	agentCommand,
	modelCommand,
	memoryCommand,
	bypassPermissionsCommand,
	compactCommand,
	exportCommand,
	copyCommand,
	turnsCommand,
	retryCommand,
	clearCommand,
	helpCommand,
	doctorCommand,
	exitCommand,
	// New feature commands
	themeCommand,
	resumeCommand,
	planCommand,
	rewindCommand,
	statusCommand,
	todosCommand,
	diffCommand,
	verboseCommand,
	hooksCommand,
	bgTasksCommand,
];

const commandMap = new Map<string, Command>();
for (const cmd of commands) {
	commandMap.set(cmd.name, cmd);
	if (cmd.aliases) {
		for (const alias of cmd.aliases) {
			commandMap.set(alias, cmd);
		}
	}
}

const categoryLabels: Record<Command['category'], string> = {
	notes: 'Notes',
	ai: 'AI',
	tools: 'Tools',
	info: 'Info',
	session: 'Session',
};

async function handleBashCommand(
	ctx: AppContext,
	command: string,
): Promise<string | undefined> {
	const { colors, conversation } = ctx;
	try {
		const shell = process.platform === 'win32' ? 'cmd' : '/bin/sh';
		const shellArgs =
			process.platform === 'win32' ? ['/c', command] : ['-c', command];
		const output = execFileSync(shell, shellArgs, {
			encoding: 'utf-8',
			timeout: 30_000,
			maxBuffer: 1024 * 1024,
			cwd: process.cwd(),
		});
		const trimmed = output.trimEnd();

		// Add output to conversation context
		if (trimmed) {
			conversation.addUser(`Shell command: ${command}\nOutput:\n${trimmed}`);
			console.log(trimmed);
		} else {
			conversation.addUser(`Shell command: ${command}\n(no output)`);
			console.log(colors.dim('(no output)'));
		}
		return undefined;
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		conversation.addUser(`Shell command: ${command}\nError: ${msg}`);
		return `${colors.red('✗')} ${msg}`;
	}
}

async function handleCommand(
	ctx: AppContext,
	input: string,
): Promise<string | null | undefined> {
	const trimmed = input.trim();
	if (!trimmed) return undefined;

	// ! prefix — run shell command and add output to context
	if (trimmed.startsWith('!') && trimmed.length > 1) {
		return handleBashCommand(ctx, trimmed.slice(1).trim());
	}

	// Slash commands require a leading /
	if (!trimmed.startsWith('/')) {
		return handleBareTextInput(ctx, trimmed);
	}

	// Parse slash command: /name args...
	const spaceIdx = trimmed.indexOf(' ');
	const name = (
		spaceIdx === -1 ? trimmed.slice(1) : trimmed.slice(1, spaceIdx)
	).toLowerCase();
	const rest = spaceIdx === -1 ? '' : trimmed.slice(spaceIdx + 1).trim();

	// Built-in commands
	const cmd = commandMap.get(name);
	if (cmd) return cmd.handler(ctx, rest);

	// Skills
	const skill = ctx.skillRegistry.get(name);
	if (skill) return handleSkillInvocation(ctx, skill, rest);

	// Unknown slash command
	return `  ${ctx.colors.red('●')} Unknown command: "/${name}". Type ${ctx.colors.cyan('/help')} for commands.`;
}

// ---------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------

function printUsage(): void {
	console.log(`
simse-code — AI agent with ACP + MCP

Usage:
  bun run cli.ts [options]

Options:
  --data-dir <path>           Data directory (default: ~/.simse)
  --log-level <level>         Log level: debug|info|warn|error|none
  -p, --prompt <text>         Non-interactive mode: run prompt and exit
  --format text|json          Output format for non-interactive mode
  --server <name>             ACP server name
  --agent <name>              Agent ID
  --continue                  Resume the most recent session
  --resume <id>               Resume a specific session
  -h, --help                  Show this help

Global config (in data directory):
  config.json                 General user preferences
  acp.json                    ACP server configuration (required)
  mcp.json                    MCP server configuration (optional)
  embed.json                  Embedding provider config (agent, server, model)
  memory.json                 Memory & vector store config

Project config (in .simse/ relative to cwd):
  settings.json               Project-specific overrides (agent, system prompt)
  prompts.json                Named prompt templates and chain definitions
  agents/                     Custom agent prompts (markdown with frontmatter)
  SIMSE.md (project root)     Project instructions (injected as system prompt)

Just type to talk to the AI. Use /help for slash commands.
`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
	const cliArgs = parseArgs();
	const colors = createColors();

	mkdirSync(cliArgs.dataDir, { recursive: true });

	// -- First-run setup ------------------------------------------------------

	if (!existsSync(join(cliArgs.dataDir, 'acp.json'))) {
		console.log(
			`${colors.bold(colors.cyan('Welcome to simse!'))} No configuration found.\n`,
		);
		const setupRl = createInterface({
			input: process.stdin,
			output: process.stdout,
			terminal: true,
		});
		await runSetup({ dataDir: cliArgs.dataDir, rl: setupRl });
		setupRl.close();
		console.log('');
	}

	// -- Load config ----------------------------------------------------------

	const configResult = createCLIConfig({
		dataDir: cliArgs.dataDir,
		logLevel: cliArgs.logLevel,
	});

	const { config, logger, memoryConfig, skippedServers } = configResult;

	// -- Create services ------------------------------------------------------

	const spinner = createThinkingSpinner({ colors });
	const serviceSpinner = createSpinner({ colors });

	// Mutable reference to the main REPL readline — set later once the REPL
	// starts.  The permission handler needs to pause it so a temporary
	// readline can take over stdin.
	let mainRl: ReadlineInterface | undefined;

	const storage = createFileStorageBackend({
		path: join(cliArgs.dataDir, memoryConfig.storageFilename ?? 'memory.simk'),
		atomicWrite: memoryConfig.atomicWrite,
		compressionLevel: memoryConfig.compressionLevel,
	});

	const acpClient = createACPClient(config.acp, {
		logger,
		onPermissionRequest: async (info: ACPPermissionRequestInfo) => {
			// Derive a human-readable description from the best available source.
			// ACP spec puts tool details in toolCall; legacy servers may use
			// top-level title/description.
			const desc =
				info.toolCall?.title ??
				info.description ??
				info.title ??
				'Agent requests permission';

			const allowOption = info.options.find((o) => o.kind === 'allow_once');
			const alwaysOption = info.options.find((o) => o.kind === 'allow_always');
			const rejectOption = info.options.find((o) => o.kind === 'reject_once');

			// Stop the spinner so it doesn't overwrite the readline prompt
			spinner.stop();

			// Show the permission request to the user
			console.log(
				`\n  ${colors.yellow('⚠')} ${colors.bold('Permission requested:')} ${desc}`,
			);

			// Show tool input details if available (command, content, etc.)
			// The title usually contains the file path already, so we
			// prioritise fields that add new information.
			if (info.toolCall?.rawInput != null) {
				const raw = info.toolCall.rawInput;
				let detail: string | undefined;
				if (typeof raw === 'object' && raw !== null) {
					const obj = raw as Record<string, unknown>;
					if (typeof obj.command === 'string') {
						detail = obj.command;
					} else if (typeof obj.content === 'string') {
						detail = obj.content;
					} else if (typeof obj.old_string === 'string') {
						detail = obj.old_string;
					} else if (typeof obj.pattern === 'string') {
						detail = obj.pattern;
					}
				} else if (typeof raw === 'string') {
					detail = raw;
				}
				if (detail) {
					// Show first few lines, trimmed
					const lines = detail.split('\n').slice(0, 6);
					if (detail.split('\n').length > 6) {
						lines.push('...');
					}
					for (const line of lines) {
						const trimmed =
							line.length > 120 ? `${line.slice(0, 117)}...` : line;
						console.log(`    ${colors.dim(trimmed)}`);
					}
				}
			}

			// Build prompt choices based on available options
			const choices: string[] = ['[y]es', '[n]o'];
			if (alwaysOption) {
				choices.push('[a]lways');
			}

			// Pause the main REPL readline so a temporary one can own stdin
			if (mainRl) {
				mainRl.pause();
				process.stdin.pause();
			}

			const permRl = createInterface({
				input: process.stdin,
				output: process.stdout,
				terminal: true,
			});

			let answer: string;
			try {
				answer = await new Promise<string>((resolve) => {
					permRl.question(`  ${colors.dim(choices.join(' / '))} `, resolve);
				});
			} finally {
				permRl.close();
				// Always resume the main REPL readline, even if the prompt threw
				if (mainRl) {
					process.stdin.resume();
					mainRl.resume();
				}
				spinner.start();
			}

			const choice = answer.trim().toLowerCase();
			if (choice === 'a' || choice === 'always') {
				return alwaysOption?.optionId ?? allowOption?.optionId;
			}
			if (choice === 'y' || choice === 'yes' || choice === 'allow') {
				return allowOption?.optionId;
			}
			return rejectOption?.optionId;
		},
	});

	// Apply bypass-permissions flag before initialization so auto-approve
	// is active from the very first ACP interaction
	if (cliArgs.bypassPermissions) {
		acpClient.setPermissionPolicy('auto-approve');
	}

	const { embedConfig } = configResult;
	const embedder = createLocalEmbedder({
		model: embedConfig.embeddingModel,
		dtype: embedConfig.dtype,
	});
	const textGenerator = createACPGenerator({ client: acpClient });

	const { workspaceSettings, workspacePrompt } = configResult;

	// Compose system prompt: SIMSE.md + workspace settings system prompt
	const systemPromptParts: string[] = [];
	if (workspacePrompt) systemPromptParts.push(workspacePrompt);
	if (workspaceSettings.systemPrompt)
		systemPromptParts.push(workspaceSettings.systemPrompt);
	const composedSystemPrompt =
		systemPromptParts.length > 0 ? systemPromptParts.join('\n\n') : undefined;

	const app = createApp({
		config,
		logger,
		storage,
		embedder,
		textGenerator,
		duplicateThreshold: memoryConfig.duplicateThreshold,
		duplicateBehavior: memoryConfig.duplicateBehavior,
		autoSave: memoryConfig.autoSave,
		flushIntervalMs: memoryConfig.flushIntervalMs,
		conversationTopic: workspaceSettings.conversationTopic,
		chainTopic: workspaceSettings.chainTopic,
		systemPrompt: composedSystemPrompt,
		autoSummarizeThreshold: memoryConfig.autoSummarizeThreshold,
	});

	// -- VFS sandbox ----------------------------------------------------------

	const vfs = createVirtualFS({
		logger,
		onFileWrite: (event: VFSWriteEvent) => {
			const label = event.isNew
				? colors.green('created')
				: colors.yellow('updated');
			console.log(
				`  ${colors.dim('[vfs]')} ${label} ${event.path} ${colors.dim(`(${event.size} bytes)`)}`,
			);
		},
	});

	const disk = createVFSDisk(vfs, { logger, baseDir: process.cwd() });

	// -- Initialize (memory → ACP → MCP) -------------------------------------

	serviceSpinner.start('Loading memory...');
	await app.initialize();
	serviceSpinner.succeed(
		renderServiceStatus('Memory', 'ok', `${app.noteCount} notes`, colors),
	);

	serviceSpinner.start('Starting ACP servers...');
	try {
		await acpClient.initialize();
		serviceSpinner.succeed(
			renderServiceStatus(
				'ACP',
				'ok',
				acpClient.serverNames.join(', '),
				colors,
			),
		);
	} catch (err) {
		serviceSpinner.fail(
			renderServiceStatus('ACP', 'error', toError(err).message, colors),
		);
	}

	try {
		serviceSpinner.start('Connecting MCP tools...');
		const connected = await app.tools.connectAll();
		if (connected.length > 0) {
			const toolCounts = await Promise.all(
				connected.map(async (name) => {
					const tools = await app.tools.listTools(name);
					return `${name} (${tools.length} tools)`;
				}),
			);
			serviceSpinner.succeed(
				renderServiceStatus('MCP', 'ok', toolCounts.join(', '), colors),
			);
		} else {
			serviceSpinner.stop();
		}
	} catch (err) {
		serviceSpinner.fail(
			renderServiceStatus(
				'MCP',
				'fail',
				`connection failed — ${err instanceof Error ? err.message : err}`,
				colors,
			),
		);
	}

	for (const skipped of skippedServers) {
		console.log(
			renderServiceStatus(
				'MCP',
				'warn',
				`"${skipped.name}" skipped — missing: ${skipped.missingEnv.join(', ')}`,
				colors,
			),
		);
	}

	// -- Embed provider change detection --------------------------------------

	const savedEmbedState = readEmbedState(cliArgs.dataDir);

	if (app.noteCount > 0 && embedConfigChanged(embedConfig, savedEmbedState)) {
		console.log('');
		console.log(
			`  ${colors.yellow('⚠')} ${colors.bold('Embedding provider changed.')} Your ${colors.cyan(`${app.noteCount}`)} notes were embedded with a different provider.`,
		);
		console.log(
			`  ${colors.dim('Mismatched embeddings will produce poor search results.')}`,
		);

		const embedRl = createInterface({
			input: process.stdin,
			output: process.stdout,
			terminal: true,
		});
		const answer = await new Promise<string>((resolve) => {
			embedRl.question(
				`  Re-embed all notes with the new provider? ${colors.dim('[y/N]')} `,
				resolve,
			);
		});
		embedRl.close();

		if (answer.trim().toLowerCase() === 'y') {
			serviceSpinner.start('Re-embedding notes...');
			try {
				const count = await app.reembed((done, total) => {
					serviceSpinner.update(`Re-embedding notes... ${done}/${total}`);
				});
				serviceSpinner.succeed(
					`Re-embedded ${colors.cyan(String(count))} notes`,
				);
				writeEmbedState(cliArgs.dataDir, embedConfig);
			} catch (err) {
				serviceSpinner.fail(
					`Re-embed failed: ${err instanceof Error ? err.message : err}`,
				);
			}
		} else {
			console.log(
				`  ${colors.dim('Skipped. Run "/embed" to change providers later.')}`,
			);
		}
	} else if (!savedEmbedState) {
		// First time tracking — save current config as baseline
		writeEmbedState(cliArgs.dataDir, embedConfig);
	}

	// -- Tool registry + conversation (agentic loop) --------------------------

	const toolRegistry = createToolRegistry({
		mcpClient: app.tools.mcpClient,
		memoryManager: app.memory,
		vfs,
		logger,
	});

	// Discover MCP tools (built-ins are registered automatically)
	await toolRegistry.discover();

	// -- Banner (after all services are initialized) --------------------------

	const defaultAgent = config.acp.defaultAgent ?? config.acp.servers[0]?.name;
	const defaultServer = config.acp.servers[0]?.name;
	const modelLabel = defaultAgent
		? `${defaultAgent}${defaultServer ? ` · ${defaultServer}` : ''}`
		: undefined;

	console.log('');
	console.log(
		renderBanner(
			{
				version: '1.0.0',
				workDir: process.cwd(),
				dataDir: cliArgs.dataDir,
				model: modelLabel,
				toolCount: toolRegistry.toolCount,
				noteCount: app.noteCount,
			},
			colors,
		),
	);
	console.log('');

	// -- REPL -----------------------------------------------------------------

	const md = createMarkdownRenderer(colors);

	const rl = createInterface({
		input: process.stdin,
		output: process.stdout,
		terminal: true,
	});
	mainRl = rl;

	const session: SessionState = {
		serverName: undefined,
		agentName: undefined,
		memoryEnabled: true,
		bypassPermissions: cliArgs.bypassPermissions,
		maxTurns: 10,
		totalTurns: 0,
		abortController: undefined,
		startedAt: Date.now(),
		lastUserInput: undefined,
	};

	const conversation = createConversation();

	// -- Skill registry -------------------------------------------------------

	const skillRegistry = createSkillRegistry({
		skills: configResult.skills,
	});

	// -- Feature initialization -----------------------------------------------

	const keybindings = createKeybindingManager();
	const fileTracker = createFileTracker();
	const verbose = createVerboseState({
		onChange: (v) => {
			console.log(
				`  ${colors.dim('Verbose:')} ${v ? renderModeBadge('verbose', colors) : colors.dim('off')}`,
			);
		},
	});
	const planMode = createPlanMode({
		onChange: (active) => {
			console.log(
				`  ${colors.dim('Plan mode:')} ${active ? renderModeBadge('plan', colors) : colors.dim('off')}`,
			);
		},
	});
	const themeManager = createThemeManager({ dataDir: cliArgs.dataDir });
	const sessionStore = createSessionStore({ dataDir: cliArgs.dataDir });
	const permissionManager = createPermissionManager({
		dataDir: cliArgs.dataDir,
	});
	const checkpointManager = createCheckpointManager({
		vfs,
		conversation,
	});
	const backgroundManager = createBackgroundManager({
		onComplete: (id, label) => {
			console.log(
				`\n  ${colors.green('●')} Background task completed: ${label} ${colors.dim(`(${id})`)}`,
			);
		},
		onError: (_id, label, error) => {
			console.log(
				`\n  ${colors.red('●')} Background task failed: ${label} — ${error.message}`,
			);
		},
	});
	const usageTracker = createUsageTracker({ dataDir: cliArgs.dataDir });
	const statusLine = createStatusLine({ colors });

	// Record session start
	usageTracker.record({ type: 'message', model: modelLabel });

	// Register keybindings
	keybindings.register({ name: 'o', ctrl: true }, 'Toggle verbose', () =>
		verbose.toggle(),
	);
	keybindings.register(
		{ name: 'tab', shift: true },
		'Cycle permission mode',
		() => {
			const _mode = permissionManager.cycleMode();
			console.log(
				`\n  ${colors.dim('Mode:')} ${permissionManager.formatMode()}`,
			);
		},
	);

	if (process.stdin.isTTY) {
		keybindings.attach(process.stdin);
	}

	const ctx: AppContext = {
		app,
		acpClient,
		configResult,
		dataDir: cliArgs.dataDir,
		vfs,
		disk,
		rl,
		colors,
		spinner,
		md,
		session,
		toolRegistry,
		conversation,
		skillRegistry,
		// Feature extensions
		keybindings,
		fileTracker,
		verbose,
		planMode,
		themeManager,
		sessionStore,
		checkpointManager,
		backgroundManager,
		usageTracker,
		statusLine,
		permissionMode: permissionManager.getMode(),
	};
	let shuttingDown = false;
	let pendingInput = '';

	const buildPrompt = (): string => {
		const parts: string[] = [];
		if (ctx.planModeState.isActive) {
			parts.push(colors.yellow('[plan]'));
		}
		if (ctx.verboseState.isVerbose) {
			parts.push(colors.cyan('[verbose]'));
		}
		const prefix = parts.length > 0 ? `${parts.join(' ')} ` : '';
		return `${prefix}${colors.dim('>')} `;
	};
	const continuationStr = `${colors.dim('…')} `;

	const processInput = async (fullInput: string): Promise<void> => {
		try {
			const result = await handleCommand(ctx, fullInput);
			if (result === null) {
				await shutdown();
				return;
			}
			if (result !== undefined) {
				console.log(`${result}\n`);
			}

			// Prompt to keep/discard after AI commands (bare text or /chain)
			const first = fullInput.trim().split(/\s+/)[0]?.toLowerCase() ?? '';
			const isAiCommand =
				(!first.startsWith('/') && !first.startsWith('!')) ||
				first === '/chain' ||
				first === '/prompt';
			if (isAiCommand) {
				const vfsResult = await promptKeepOrDiscard(ctx);
				if (vfsResult) console.log(`${vfsResult}\n`);
			}
		} catch (err) {
			spinner.stop();
			console.error(
				`${colors.red('Error:')} ${err instanceof Error ? err.message : err}\n`,
			);
		}
		prompt();
	};

	const prompt = (): void => {
		rl.question(pendingInput ? continuationStr : buildPrompt(), (line) => {
			// Multi-line: backslash continuation
			if (line.endsWith('\\')) {
				pendingInput += `${line.slice(0, -1)}\n`;
				prompt();
				return;
			}

			const fullInput = pendingInput + line;
			pendingInput = '';
			processInput(fullInput);
		});
	};

	const shutdown = async (): Promise<void> => {
		if (shuttingDown) return;
		shuttingDown = true;
		console.log(colors.dim('Shutting down...'));
		rl.close();
		await app.dispose();
		await acpClient.dispose();
		console.log(colors.dim('Bye.'));
		process.exit(0);
	};

	process.on('SIGINT', () => {
		spinner.stop();
		if (shuttingDown) return;

		// If a generation is in progress, abort it
		if (session.abortController) {
			session.abortController.abort();
			session.abortController = undefined;
			return;
		}

		console.log('');
		prompt();
	});

	// Esc key — also interrupts generation (like Claude Code)
	if (process.stdin.isTTY) {
		process.stdin.on('keypress', (_ch: string, key: { name?: string }) => {
			if (key?.name === 'escape' && session.abortController) {
				spinner.stop();
				session.abortController.abort();
				session.abortController = undefined;
			}
		});
	}

	rl.on('close', () => {
		shutdown().catch(() => process.exit(1));
	});

	prompt();
}

main().catch((err) => {
	console.error('Fatal error:', err);
	process.exit(1);
});
