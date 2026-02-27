import { randomUUID } from 'node:crypto';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export type ACPBackend = 'claude' | 'ollama' | 'none';

export interface TempConfigResult {
	readonly dataDir: string;
	readonly cleanup: () => Promise<void>;
}

export async function createTempConfig(
	backend: ACPBackend,
): Promise<TempConfigResult> {
	const dataDir = join(tmpdir(), `simse-e2e-${randomUUID().slice(0, 8)}`);
	await mkdir(dataDir, { recursive: true });

	await writeFile(
		join(dataDir, 'config.json'),
		JSON.stringify({ logLevel: 'none' }),
	);

	await writeFile(
		join(dataDir, 'memory.json'),
		JSON.stringify({ autoSummarizeThreshold: 20 }),
	);

	await writeFile(
		join(dataDir, 'embed.json'),
		JSON.stringify({
			embeddingModel: 'nomic-ai/nomic-embed-text-v1.5',
			provider: 'local',
		}),
	);

	if (backend === 'claude') {
		await writeFile(
			join(dataDir, 'acp.json'),
			JSON.stringify({
				servers: [
					{
						name: 'claude',
						command: 'bunx',
						args: ['claude-code-acp'],
					},
				],
				defaultServer: 'claude',
			}),
		);
	} else if (backend === 'ollama') {
		const ollamaUrl =
			process.env.OLLAMA_URL ?? 'http://127.0.0.1:11434';
		const ollamaModel = process.env.OLLAMA_MODEL ?? 'llama3.2';
		await writeFile(
			join(dataDir, 'acp.json'),
			JSON.stringify({
				servers: [
					{
						name: 'ollama',
						command: 'bun',
						args: [
							'run',
							'acp-ollama-bridge.ts',
							'--ollama',
							ollamaUrl,
							'--model',
							ollamaModel,
						],
					},
				],
				defaultServer: 'ollama',
			}),
		);
	}

	return {
		dataDir,
		cleanup: async () => {
			await rm(dataDir, { recursive: true, force: true });
		},
	};
}
