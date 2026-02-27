/**
 * /setup command — configure ACP servers and default config files.
 *
 * Usage:
 *   /setup                  — Show available presets
 *   /setup claude-code      — Configure Claude Code as the ACP server
 *   /setup ollama [url]     — Configure Ollama (default: http://127.0.0.1:11434)
 *   /setup copilot          — Configure GitHub Copilot
 *   /setup custom <cmd>     — Configure a custom ACP server command
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { CommandDefinition } from '../../ink-types.js';

// ---------------------------------------------------------------------------
// Preset configs
// ---------------------------------------------------------------------------

interface ACPServerEntry {
	readonly name: string;
	readonly command: string;
	readonly args?: readonly string[];
}

interface ACPFileConfig {
	readonly servers: readonly ACPServerEntry[];
	readonly defaultServer?: string;
}

const PRESETS: Record<string, { description: string; build: (args: string) => ACPServerEntry }> = {
	'claude-code': {
		description: 'Anthropic Claude via claude-code-acp',
		build: () => ({
			name: 'claude',
			command: 'bunx',
			args: ['claude-code-acp'],
		}),
	},
	ollama: {
		description: 'Local AI via Ollama + ACP bridge',
		build: (args) => {
			const parts = args.split(/\s+/).filter(Boolean);
			const url = parts[0] || 'http://127.0.0.1:11434';
			const model = parts[1] || 'llama3.2';
			return {
				name: 'ollama',
				command: 'bun',
				args: ['run', 'acp-ollama-bridge.ts', '--ollama', url, '--model', model],
			};
		},
	},
	copilot: {
		description: 'GitHub Copilot CLI',
		build: () => ({
			name: 'copilot',
			command: 'copilot',
			args: ['--acp'],
		}),
	},
	custom: {
		description: 'Any ACP-compatible server command',
		build: (args) => {
			const parts = args.split(/\s+/).filter(Boolean);
			if (parts.length === 0) {
				throw new Error('Usage: /setup custom <command> [args...]');
			}
			const command = parts[0]!;
			const cmdArgs = parts.slice(1);
			const name = command.replace(/[^a-zA-Z0-9-]/g, '').toLowerCase() || 'custom';
			return {
				name,
				command,
				...(cmdArgs.length > 0 && { args: cmdArgs }),
			};
		},
	},
};

// ---------------------------------------------------------------------------
// File writers
// ---------------------------------------------------------------------------

function ensureDir(dir: string): void {
	if (!existsSync(dir)) {
		mkdirSync(dir, { recursive: true });
	}
}

function writeJsonIfMissing(path: string, data: unknown): boolean {
	if (existsSync(path)) return false;
	writeFileSync(path, JSON.stringify(data, null, 2) + '\n', 'utf-8');
	return true;
}

function writeSetupFiles(
	dataDir: string,
	server: ACPServerEntry,
): string[] {
	ensureDir(dataDir);
	const created: string[] = [];

	// Always write acp.json (overwrite to update server config)
	const acpConfig: ACPFileConfig = {
		servers: [server],
		defaultServer: server.name,
	};
	const acpPath = join(dataDir, 'acp.json');
	writeFileSync(acpPath, JSON.stringify(acpConfig, null, 2) + '\n', 'utf-8');
	created.push('acp.json');

	// Create default config files if they don't exist
	if (writeJsonIfMissing(join(dataDir, 'config.json'), {})) {
		created.push('config.json');
	}
	if (writeJsonIfMissing(join(dataDir, 'mcp.json'), { servers: [] })) {
		created.push('mcp.json');
	}
	if (
		writeJsonIfMissing(join(dataDir, 'memory.json'), {
			enabled: true,
			autoSummarizeThreshold: 20,
		})
	) {
		created.push('memory.json');
	}
	if (
		writeJsonIfMissing(join(dataDir, 'embed.json'), {
			embeddingModel: 'nomic-ai/nomic-embed-text-v1.5',
		})
	) {
		created.push('embed.json');
	}

	return created;
}

// ---------------------------------------------------------------------------
// Command factory
// ---------------------------------------------------------------------------

export function createSetupCommands(
	dataDir: string,
): readonly CommandDefinition[] {
	return [
		{
			name: 'setup',
			usage: '/setup [preset] [options]',
			description: 'Configure ACP server and default settings',
			category: 'config',
			execute: (args) => {
				const trimmed = args.trim();

				// No args — show available presets
				if (!trimmed) {
					const lines = [
						'Available presets:',
						'',
						...Object.entries(PRESETS).map(
							([name, preset]) =>
								`  /setup ${name.padEnd(14)} ${preset.description}`,
						),
						'',
						'Examples:',
						'  /setup claude-code',
						'  /setup ollama',
						'  /setup ollama http://localhost:11434 llama3.2',
						'  /setup custom my-server --port 8080',
						'',
						`Config directory: ${dataDir}`,
					];
					return { text: lines.join('\n') };
				}

				// Parse preset name and remaining args
				const parts = trimmed.split(/\s+/);
				const presetName = parts[0]!.toLowerCase();
				const presetArgs = parts.slice(1).join(' ');

				const preset = PRESETS[presetName];
				if (!preset) {
					return {
						text: `Unknown preset: "${presetName}". Run /setup to see available presets.`,
					};
				}

				try {
					const server = preset.build(presetArgs);
					const created = writeSetupFiles(dataDir, server);

					const lines = [
						`Configured ACP server: ${server.name}`,
						`  Command: ${server.command}${server.args ? ' ' + server.args.join(' ') : ''}`,
						'',
						`Files written to ${dataDir}:`,
						...created.map((f) => `  ${f}`),
						'',
						'Restart simse to connect to the new server.',
					];
					return { text: lines.join('\n') };
				} catch (err) {
					return {
						text:
							err instanceof Error
								? err.message
								: 'Setup failed.',
					};
				}
			},
		},
	];
}
