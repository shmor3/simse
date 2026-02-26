/**
 * SimSE CLI — Setup
 *
 * Global setup: asks the bare minimum to get running (one ACP server).
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Interface as ReadlineInterface } from 'node:readline';
import type {
	ACPFileConfig,
	ACPServerConfig,
	EmbedFileConfig,
	SummarizeFileConfig,
} from './config.js';

// ---------------------------------------------------------------------------
// Options & result
// ---------------------------------------------------------------------------

export interface SetupOptions {
	readonly dataDir: string;
	readonly rl: ReadlineInterface;
}

export interface SetupResult {
	readonly filesCreated: readonly string[];
}

// ---------------------------------------------------------------------------
// Prompt helpers
// ---------------------------------------------------------------------------

function ask(rl: ReadlineInterface, question: string): Promise<string> {
	return new Promise((resolve) => {
		rl.question(question, resolve);
	});
}

async function askRequired(
	rl: ReadlineInterface,
	question: string,
): Promise<string> {
	while (true) {
		const answer = (await ask(rl, question)).trim();
		if (answer) return answer;
		console.log('  This field is required.');
	}
}

async function askOptional(
	rl: ReadlineInterface,
	question: string,
): Promise<string | undefined> {
	const answer = (await ask(rl, question)).trim();
	return answer || undefined;
}

// ---------------------------------------------------------------------------
// ACP Presets
// ---------------------------------------------------------------------------

interface PresetResult {
	readonly server: ACPServerConfig;
	readonly embed?: EmbedFileConfig;
}

interface Preset {
	readonly label: string;
	readonly description: string;
	readonly build: (rl: ReadlineInterface) => Promise<PresetResult>;
}

const presets: readonly Preset[] = [
	{
		label: 'Ollama',
		description: 'Local AI via Ollama + bundled ACP bridge',
		build: async (rl) => {
			const url =
				(await askOptional(rl, '  Ollama URL [http://127.0.0.1:11434]: ')) ??
				'http://127.0.0.1:11434';
			const model =
				(await askOptional(rl, '  Model [llama3.2]: ')) ?? 'llama3.2';
			const embeddingModel =
				(await askOptional(rl, '  Embedding model [nomic-embed-text]: ')) ??
				'nomic-embed-text';

			return {
				server: {
					name: 'ollama',
					command: 'bun',
					args: [
						'run',
						'acp-ollama-bridge.ts',
						'--ollama',
						url,
						'--model',
						model,
						'--embedding-model',
						embeddingModel,
					],
				},
				embed: {
					embeddingModel,
				},
			};
		},
	},
	{
		label: 'Claude Code',
		description: 'Anthropic Claude via claude-code-acp',
		build: async () => ({
			server: {
				name: 'claude',
				command: 'claude-code-acp',
			},
		}),
	},
	{
		label: 'GitHub Copilot',
		description: 'GitHub Copilot CLI',
		build: async () => ({
			server: {
				name: 'copilot',
				command: 'copilot',
				args: ['--acp'],
			},
		}),
	},
	{
		label: 'Custom',
		description: 'Any ACP-compatible server',
		build: async (rl) => {
			const name = await askRequired(rl, '  Server name: ');
			const command = await askRequired(rl, '  Command: ');
			const argsStr = await askOptional(
				rl,
				'  Args (space-separated, enter to skip): ',
			);
			const args = argsStr ? argsStr.split(/\s+/) : undefined;
			const embeddingModel = await askOptional(
				rl,
				'  Embedding model (enter to skip): ',
			);

			return {
				server: {
					name,
					command,
					...(args && { args }),
				},
				...(embeddingModel && { embed: { embeddingModel } }),
			};
		},
	},
];

// ---------------------------------------------------------------------------
// Global setup — ACP only (first-run wizard)
// ---------------------------------------------------------------------------

export async function runSetup(options: SetupOptions): Promise<SetupResult> {
	const { dataDir, rl } = options;
	const filesCreated: string[] = [];

	mkdirSync(dataDir, { recursive: true });

	// -- ACP config (interactive preset selection) ---------------------------

	const acpPath = join(dataDir, 'acp.json');
	let embedFromPreset: EmbedFileConfig | undefined;

	if (existsSync(acpPath)) {
		console.log('  acp.json already exists, skipping.');
	} else {
		console.log('\n  Select your AI provider:\n');
		for (let i = 0; i < presets.length; i++) {
			const p = presets[i];
			console.log(`    ${i + 1}) ${p.label}  —  ${p.description}`);
		}
		console.log('');

		let choiceIdx = -1;
		while (choiceIdx < 0 || choiceIdx >= presets.length) {
			const answer = (await ask(rl, `  Choice [1-${presets.length}]: `)).trim();
			const num = Number.parseInt(answer, 10);
			if (!Number.isNaN(num) && num >= 1 && num <= presets.length) {
				choiceIdx = num - 1;
			}
		}

		const result = await presets[choiceIdx].build(rl);
		const config: ACPFileConfig = { servers: [result.server] };
		embedFromPreset = result.embed;

		writeFileSync(acpPath, `${JSON.stringify(config, null, '\t')}\n`, 'utf-8');
		filesCreated.push('acp.json');
		console.log(`\n  Wrote ${acpPath}`);
	}

	// -- Capture ACP config for reuse in summarization -------------------------

	let acpFileConfig: ACPFileConfig | undefined;
	try {
		const rawAcp = readFileSync(acpPath, 'utf-8');
		acpFileConfig = JSON.parse(rawAcp) as ACPFileConfig;
	} catch {
		// Ignore — preset may not have been written if acp.json already existed
	}

	// -- Summarization ACP config (interactive) --------------------------------

	const summarizePath = join(dataDir, 'summarize.json');

	if (existsSync(summarizePath)) {
		console.log('  summarize.json already exists, skipping.');
	} else {
		console.log(
			'\n  Configure summarization? (uses a separate LLM for auto-summarizing notes)\n',
		);
		console.log('    1) Same as above  —  Reuse main ACP server');
		console.log(
			'    2) Different provider  —  Configure a separate ACP server',
		);
		console.log('    3) Skip  —  No auto-summarization');
		console.log('');

		let summarizeChoice = -1;
		while (summarizeChoice < 1 || summarizeChoice > 3) {
			const answer = (await ask(rl, '  Choice [1-3]: ')).trim();
			const num = Number.parseInt(answer, 10);
			if (!Number.isNaN(num) && num >= 1 && num <= 3) {
				summarizeChoice = num;
			}
		}

		if (summarizeChoice === 1 && acpFileConfig) {
			// Reuse the first ACP server entry
			const mainServer = acpFileConfig.servers[0];
			const summarizeConfig: SummarizeFileConfig = {
				server: mainServer.name,
				command: mainServer.command,
				...(mainServer.args && { args: mainServer.args }),
			};
			writeFileSync(
				summarizePath,
				`${JSON.stringify(summarizeConfig, null, '\t')}\n`,
				'utf-8',
			);
			filesCreated.push('summarize.json');
			console.log(`  Wrote ${summarizePath}`);
		} else if (summarizeChoice === 2) {
			const serverName = await askRequired(rl, '  Server name: ');
			const command = await askRequired(rl, '  Command: ');
			const argsStr = await askOptional(
				rl,
				'  Args (space-separated, enter to skip): ',
			);
			const args = argsStr ? argsStr.split(/\s+/) : undefined;
			const agent = await askOptional(rl, '  Agent ID (enter to skip): ');

			const summarizeConfig: SummarizeFileConfig = {
				server: serverName,
				command,
				...(args && { args }),
				...(agent && { agent }),
			};
			writeFileSync(
				summarizePath,
				`${JSON.stringify(summarizeConfig, null, '\t')}\n`,
				'utf-8',
			);
			filesCreated.push('summarize.json');
			console.log(`  Wrote ${summarizePath}`);
		} else {
			console.log('  Skipping summarization config.');
		}
	}

	// -- Generate remaining config files with defaults (skip existing) ------

	const defaults: readonly {
		readonly file: string;
		readonly content: object;
	}[] = [
		{ file: 'embed.json', content: embedFromPreset ?? {} },
		{ file: 'config.json', content: {} },
		{ file: 'mcp.json', content: { servers: [] } },
		{
			file: 'memory.json',
			content: {
				enabled: true,
				similarityThreshold: 0.7,
				maxResults: 10,
				autoSummarizeThreshold: 20,
			},
		},
	];

	for (const { file, content } of defaults) {
		const filePath = join(dataDir, file);
		if (existsSync(filePath)) continue;
		writeFileSync(
			filePath,
			`${JSON.stringify(content, null, '\t')}\n`,
			'utf-8',
		);
		filesCreated.push(file);
		console.log(`  Wrote ${filePath}`);
	}

	return Object.freeze({ filesCreated: Object.freeze(filesCreated) });
}
