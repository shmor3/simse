#!/usr/bin/env bun
import { homedir } from 'node:os';
import { join } from 'node:path';
import { render } from 'ink';
import React from 'react';
import type { ACPClient, ACPPermissionRequestInfo } from 'simse';
import { createACPClient, toError } from 'simse';
import { App } from './app-ink.js';
import { createCLIConfig } from './config.js';
import type { Conversation } from './conversation.js';
import { createConversation } from './conversation.js';
import { createPermissionManager } from './permission-manager.js';
import { createSessionStore } from './session-store.js';
import type { ToolRegistry } from './tool-registry.js';
import { createToolRegistry } from './tool-registry.js';

function parseArgs(): {
	dataDir: string;
	serverName?: string;
	bypassPermissions?: boolean;
	continueSession?: boolean;
	resumeId?: string;
} {
	const args = process.argv.slice(2);
	let dataDir = join(homedir(), '.simse');
	let bypassPermissions = false;
	let continueSession = false;
	let resumeId: string | undefined;

	for (let i = 0; i < args.length; i++) {
		if (args[i] === '--data-dir' && args[i + 1]) {
			dataDir = args[i + 1]!;
			i++;
		}
		if (args[i] === '--bypass-permissions' || args[i] === '-y') {
			bypassPermissions = true;
		}
		if (args[i] === '--continue' || args[i] === '-c') {
			continueSession = true;
		}
		if (args[i] === '--resume' && args[i + 1]) {
			resumeId = args[i + 1]!;
			i++;
		}
	}

	return { dataDir, bypassPermissions, continueSession, resumeId };
}

const { dataDir, bypassPermissions, continueSession, resumeId } = parseArgs();

if (!process.stdin.isTTY) {
	console.error(
		'Error: simse-code requires an interactive terminal (TTY).\n' +
			'Use "bun run start:legacy" for non-interactive mode.',
	);
	process.exit(1);
}

// ---------------------------------------------------------------------------
// Service initialization
// ---------------------------------------------------------------------------

async function bootstrap(): Promise<{
	acpClient: ACPClient;
	conversation: Conversation;
	toolRegistry: ToolRegistry;
	serverName?: string;
	modelName?: string;
	hasACP: boolean;
	permissionManager: ReturnType<typeof createPermissionManager>;
	sessionStore: ReturnType<typeof createSessionStore>;
	sessionId: string;
}> {
	const configResult = createCLIConfig({ dataDir });
	const { config, logger } = configResult;

	const hasServers = config.acp.servers.length > 0;
	const defaultServerName = hasServers
		? (config.acp.defaultServer ?? config.acp.servers[0]?.name)
		: undefined;

	const acpClient = createACPClient(config.acp, {
		logger,
		onPermissionRequest: async (
			info: ACPPermissionRequestInfo,
		): Promise<string | undefined> => {
			// Fallback handler — should rarely fire since policy is auto-approve.
			// Try allow_always, then allow_once, then first option.
			const pick =
				info.options.find((o) => o.kind === 'allow_always') ??
				info.options.find((o) => o.kind === 'allow_once') ??
				info.options[0];
			return pick?.optionId;
		},
	});

	// Always auto-approve permissions — simse-code handles tool safety at
	// the agentic loop level, not via ACP permission dialogs.
	acpClient.setPermissionPolicy('auto-approve');

	// Initialize ACP servers (skip if none configured)
	if (hasServers) {
		try {
			await acpClient.initialize();
		} catch (err) {
			console.error(`ACP initialization failed: ${toError(err).message}`);
		}
	}

	// Get model info for banner
	let modelName: string | undefined;
	if (defaultServerName) {
		try {
			const modelInfo = await acpClient.getServerModelInfo(defaultServerName);
			modelName = modelInfo?.currentModelId;
		} catch {
			// Model info is optional
		}
	}

	const conversation = createConversation();

	// Compose system prompt from workspace config
	const systemPromptParts: string[] = [];
	if (configResult.workspacePrompt) {
		systemPromptParts.push(configResult.workspacePrompt);
	}
	if (configResult.workspaceSettings.systemPrompt) {
		systemPromptParts.push(configResult.workspaceSettings.systemPrompt);
	}
	if (systemPromptParts.length > 0) {
		conversation.setSystemPrompt(systemPromptParts.join('\n\n'));
	}

	const toolRegistry = createToolRegistry({ logger });

	// Discover MCP tools
	await toolRegistry.discover();

	const permissionManager = createPermissionManager({
		dataDir,
		initialMode: bypassPermissions ? 'dontAsk' : undefined,
	});

	// Session store + ID
	const sessionStore = createSessionStore(dataDir);
	let sessionId: string;

	if (resumeId) {
		// --resume <id-prefix>: find matching session
		const sessions = sessionStore.list();
		const match = sessions.find((s) => s.id.startsWith(resumeId));
		if (!match) {
			console.error(`No session found matching "${resumeId}".`);
			process.exit(1);
		}
		sessionId = match.id;
		const messages = sessionStore.load(sessionId);
		conversation.loadMessages(messages);
	} else if (continueSession) {
		// --continue: resume most recent session for this workDir
		const latest = sessionStore.latest(process.cwd());
		if (latest) {
			sessionId = latest;
			const messages = sessionStore.load(sessionId);
			conversation.loadMessages(messages);
		} else {
			sessionId = sessionStore.create(process.cwd());
		}
	} else {
		sessionId = sessionStore.create(process.cwd());
	}

	return {
		acpClient,
		conversation,
		toolRegistry,
		serverName: defaultServerName,
		modelName,
		hasACP: hasServers,
		permissionManager,
		sessionStore,
		sessionId,
	};
}

// Bootstrap services then render
bootstrap()
	.then(
		({
			acpClient,
			conversation,
			toolRegistry,
			serverName,
			modelName,
			hasACP,
			permissionManager,
			sessionStore,
			sessionId,
		}) => {
			const inkInstance = render(
				<App
					dataDir={dataDir}
					serverName={serverName}
					modelName={modelName}
					acpClient={acpClient}
					conversation={conversation}
					toolRegistry={toolRegistry}
					permissionManager={permissionManager}
					sessionStore={sessionStore}
					sessionId={sessionId}
					hasACP={hasACP}
				/>,
				{ exitOnCtrlC: false },
			);

			// Work around Ink resize bug: Ink erases previous output by counting
			// newline-delimited lines, but doesn't account for terminal line wrapping.
			// When width changes, old rendered lines reflow into different row counts,
			// so eraseLines() misses rows → ghost content accumulates.
			// Fix: clear the visible screen before Ink's handler fires.
			// Static (conversation) content remains in scrollback buffer.
			process.stdout.prependListener('resize', () => {
				inkInstance.clear();
				process.stdout.write('\x1b[2J\x1b[H');
			});
		},
	)
	.catch((err) => {
		console.error(`Failed to start: ${toError(err).message}`);
		process.exit(1);
	});
