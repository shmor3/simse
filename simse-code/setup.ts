/**
 * SimSE CLI — Setup
 *
 * Global setup: asks the bare minimum to get running (one ACP server).
 * All config files are written atomically after the full setup flow completes.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
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
// ACP Presets — only configure ACP servers, never embedding
// ---------------------------------------------------------------------------

interface Preset {
	readonly label: string;
	readonly description: string;
	readonly build: (rl: ReadlineInterface) => Promise<ACPServerConfig>;
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

			return {
				name: 'ollama',
				command: 'bun',
				args: [
					'run',
					'acp-ollama-bridge.ts',
					'--ollama',
					url,
					'--model',
					model,
				],
			};
		},
	},
	{
		label: 'Claude Code',
		description: 'Anthropic Claude via claude-code-acp',
		build: async () => ({
			name: 'claude',
			command: 'bunx',
			args: ['claude-code-acp'],
		}),
	},
	{
		label: 'GitHub Copilot',
		description: 'GitHub Copilot CLI',
		build: async () => ({
			name: 'copilot',
			command: 'copilot',
			args: ['--acp'],
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

			return {
				name,
				command,
				...(args && { args }),
			};
		},
	},
];

// ---------------------------------------------------------------------------
// Embedding presets — small / medium / large / TEI
// ---------------------------------------------------------------------------

interface EmbeddingPreset {
	readonly label: string;
	readonly description: string;
	readonly model: string;
}

const embeddingPresets: readonly EmbeddingPreset[] = [
	{
		label: 'Small',
		description: 'Snowflake/snowflake-arctic-embed-xs  (fast, 22M params)',
		model: 'Snowflake/snowflake-arctic-embed-xs',
	},
	{
		label: 'Medium',
		description: 'nomic-ai/nomic-embed-text-v1.5  (recommended, 137M params)',
		model: 'nomic-ai/nomic-embed-text-v1.5',
	},
	{
		label: 'Large',
		description:
			'Snowflake/snowflake-arctic-embed-l  (best quality, 335M params)',
		model: 'Snowflake/snowflake-arctic-embed-l',
	},
];

/**
 * Run the preset picker and return the selected server config.
 */
async function pickPreset(
	rl: ReadlineInterface,
	header: string,
): Promise<ACPServerConfig> {
	console.log(`\n  ${header}\n`);
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

	return presets[choiceIdx].build(rl);
}

// ---------------------------------------------------------------------------
// Pending file — collected during the flow, written at the end
// ---------------------------------------------------------------------------

interface PendingFile {
	readonly file: string;
	readonly content: object;
}

// ---------------------------------------------------------------------------
// Global setup
// ---------------------------------------------------------------------------

export async function runSetup(options: SetupOptions): Promise<SetupResult> {
	const { dataDir, rl } = options;

	mkdirSync(dataDir, { recursive: true });

	// Accumulate files to write — nothing touches disk until the end
	const pendingFiles: PendingFile[] = [];

	// -- Step 1: ACP provider ------------------------------------------------

	const acpPath = join(dataDir, 'acp.json');
	let acpConfig: ACPFileConfig | undefined;

	if (existsSync(acpPath)) {
		console.log('  acp.json already exists, skipping.');
	} else {
		const server = await pickPreset(rl, 'Select your AI provider:');
		acpConfig = { servers: [server] };
		pendingFiles.push({ file: 'acp.json', content: acpConfig });
	}

	// -- Step 2: Summarization -----------------------------------------------

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

		if (summarizeChoice === 1 && acpConfig) {
			const mainServer = acpConfig.servers[0];
			const summarizeConfig: SummarizeFileConfig = {
				server: mainServer.name,
				command: mainServer.command,
				...(mainServer.args && { args: mainServer.args }),
			};
			pendingFiles.push({ file: 'summarize.json', content: summarizeConfig });
		} else if (summarizeChoice === 2) {
			const server = await pickPreset(rl, 'Select summarization provider:');
			const summarizeConfig: SummarizeFileConfig = {
				server: server.name,
				command: server.command,
				...(server.args && { args: server.args }),
			};
			pendingFiles.push({ file: 'summarize.json', content: summarizeConfig });
		} else {
			console.log('  Skipping summarization config.');
		}
	}

	// -- Step 3: Embedding model ---------------------------------------------

	const embedPath = join(dataDir, 'embed.json');
	let embedConfig: EmbedFileConfig | undefined;

	if (existsSync(embedPath)) {
		console.log('  embed.json already exists, skipping.');
	} else {
		console.log(
			'\n  Select embedding provider (used for memory search across all AI providers)\n',
		);
		for (let i = 0; i < embeddingPresets.length; i++) {
			const p = embeddingPresets[i];
			console.log(`    ${i + 1}) ${p.label}  —  ${p.description}`);
		}
		console.log(
			`    ${embeddingPresets.length + 1}) TEI  —  Text Embeddings Inference server (custom URL)`,
		);
		console.log('');

		const totalChoices = embeddingPresets.length + 1;
		let embedChoice = -1;
		while (embedChoice < 1 || embedChoice > totalChoices) {
			const answer = (await ask(rl, `  Choice [1-${totalChoices}]: `)).trim();
			const num = Number.parseInt(answer, 10);
			if (!Number.isNaN(num) && num >= 1 && num <= totalChoices) {
				embedChoice = num;
			}
		}

		if (embedChoice <= embeddingPresets.length) {
			// Built-in local model
			const preset = embeddingPresets[embedChoice - 1];
			embedConfig = { embeddingModel: preset.model };
		} else {
			// TEI bridge
			const teiUrl =
				(await askOptional(rl, '  TEI server URL [http://localhost:8080]: ')) ??
				'http://localhost:8080';
			embedConfig = { teiUrl };
		}

		pendingFiles.push({ file: 'embed.json', content: embedConfig });
	}

	// -- Default config files (skip existing) ---------------------------------

	const defaults: readonly PendingFile[] = [
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

	for (const entry of defaults) {
		if (!existsSync(join(dataDir, entry.file))) {
			pendingFiles.push(entry);
		}
	}

	// -- Write all files atomically ------------------------------------------

	console.log('');
	const filesCreated: string[] = [];

	for (const { file, content } of pendingFiles) {
		const filePath = join(dataDir, file);
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
