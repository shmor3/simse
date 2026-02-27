#!/usr/bin/env bun
import { render } from 'ink';
import React from 'react';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { createACPClient, toError } from 'simse';
import type { ACPClient, ACPPermissionRequestInfo } from 'simse';
import { App } from './app-ink.js';
import { createCLIConfig } from './config.js';
import { createConversation } from './conversation.js';
import type { Conversation } from './conversation.js';
import { createToolRegistry } from './tool-registry.js';
import type { ToolRegistry } from './tool-registry.js';

function parseArgs(): {
	dataDir: string;
	serverName?: string;
	bypassPermissions?: boolean;
} {
	const args = process.argv.slice(2);
	let dataDir = join(homedir(), '.simse');
	let bypassPermissions = false;

	for (let i = 0; i < args.length; i++) {
		if (args[i] === '--data-dir' && args[i + 1]) {
			dataDir = args[i + 1]!;
			i++;
		}
		if (args[i] === '--bypass-permissions' || args[i] === '-y') {
			bypassPermissions = true;
		}
	}

	return { dataDir, bypassPermissions };
}

const { dataDir, bypassPermissions } = parseArgs();

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
}> {
	const configResult = createCLIConfig({ dataDir });
	const { config, logger } = configResult;

	// Determine if the agent manages tools (has permissionPolicy)
	const defaultServerName =
		config.acp.defaultServer ?? config.acp.servers[0]?.name;

	const acpClient = createACPClient(config.acp, {
		logger,
		onPermissionRequest: async (
			info: ACPPermissionRequestInfo,
		): Promise<string | undefined> => {
			// For now, auto-approve all permissions in the Ink CLI.
			// A proper permission dialog component will be added later.
			const allowOption = info.options.find((o) => o.kind === 'allow_once');
			return allowOption?.optionId;
		},
	});

	if (bypassPermissions) {
		acpClient.setPermissionPolicy('auto-approve');
	}

	// Initialize ACP servers
	try {
		await acpClient.initialize();
	} catch (err) {
		console.error(`ACP initialization failed: ${toError(err).message}`);
	}

	// Get model info for banner
	let modelName: string | undefined;
	if (defaultServerName) {
		try {
			const modelInfo =
				await acpClient.getServerModelInfo(defaultServerName);
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

	return {
		acpClient,
		conversation,
		toolRegistry,
		serverName: defaultServerName,
		modelName,
	};
}

// Bootstrap services then render
bootstrap()
	.then(({ acpClient, conversation, toolRegistry, serverName, modelName }) => {
		render(
			<App
				dataDir={dataDir}
				serverName={serverName}
				modelName={modelName}
				acpClient={acpClient}
				conversation={conversation}
				toolRegistry={toolRegistry}
			/>,
		);
	})
	.catch((err) => {
		console.error(`Failed to start: ${toError(err).message}`);
		process.exit(1);
	});
