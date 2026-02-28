/**
 * Onboarding file writer — takes all wizard results and writes config files atomically.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface OnboardingResult {
	readonly acp: {
		readonly name: string;
		readonly command: string;
		readonly args?: readonly string[];
	};
	readonly summarize:
		| 'same'
		| 'skip'
		| {
				readonly name: string;
				readonly command: string;
				readonly args?: readonly string[];
		  };
	readonly embed:
		| { readonly kind: 'local'; readonly model: string }
		| { readonly kind: 'tei'; readonly url: string };
	readonly library?: {
		readonly enabled?: boolean;
		readonly similarityThreshold?: number;
		readonly maxResults?: number;
		readonly autoSummarizeThreshold?: number;
	};
	readonly logLevel: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function ensureDir(dir: string): void {
	if (!existsSync(dir)) {
		mkdirSync(dir, { recursive: true });
	}
}

function writeJson(path: string, data: unknown): void {
	writeFileSync(path, `${JSON.stringify(data, null, '\t')}\n`, 'utf-8');
}

function writeJsonIfMissing(path: string, data: unknown): boolean {
	if (existsSync(path)) return false;
	writeJson(path, data);
	return true;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function writeOnboardingFiles(
	dataDir: string,
	result: OnboardingResult,
): readonly string[] {
	ensureDir(dataDir);
	const created: string[] = [];

	// acp.json — always written
	const acpConfig = {
		servers: [
			{
				name: result.acp.name,
				command: result.acp.command,
				...(result.acp.args &&
					result.acp.args.length > 0 && { args: result.acp.args }),
			},
		],
		defaultServer: result.acp.name,
	};
	writeJson(join(dataDir, 'acp.json'), acpConfig);
	created.push('acp.json');

	// summarize.json — written when 'same' or separate server
	if (result.summarize === 'same') {
		// Copy main ACP config for summarization
		writeJson(join(dataDir, 'summarize.json'), {
			servers: [
				{
					name: result.acp.name,
					command: result.acp.command,
					...(result.acp.args &&
						result.acp.args.length > 0 && { args: result.acp.args }),
				},
			],
			defaultServer: result.acp.name,
		});
		created.push('summarize.json');
	} else if (result.summarize !== 'skip') {
		writeJson(join(dataDir, 'summarize.json'), {
			servers: [
				{
					name: result.summarize.name,
					command: result.summarize.command,
					...(result.summarize.args &&
						result.summarize.args.length > 0 && {
							args: result.summarize.args,
						}),
				},
			],
			defaultServer: result.summarize.name,
		});
		created.push('summarize.json');
	}

	// embed.json — local model or TEI URL
	if (result.embed.kind === 'local') {
		writeJson(join(dataDir, 'embed.json'), {
			embeddingModel: result.embed.model,
		});
	} else {
		writeJson(join(dataDir, 'embed.json'), {
			teiUrl: result.embed.url,
		});
	}
	created.push('embed.json');

	// memory.json — library settings with defaults
	const memoryConfig = {
		enabled: result.library?.enabled ?? true,
		similarityThreshold: result.library?.similarityThreshold ?? 0.7,
		maxResults: result.library?.maxResults ?? 10,
		autoSummarizeThreshold: result.library?.autoSummarizeThreshold ?? 20,
	};
	writeJson(join(dataDir, 'memory.json'), memoryConfig);
	created.push('memory.json');

	// config.json — only if doesn't exist
	const configData =
		result.logLevel !== 'warn' ? { logLevel: result.logLevel } : {};
	if (writeJsonIfMissing(join(dataDir, 'config.json'), configData)) {
		created.push('config.json');
	}

	// mcp.json — only if doesn't exist
	if (writeJsonIfMissing(join(dataDir, 'mcp.json'), { servers: [] })) {
		created.push('mcp.json');
	}

	return created;
}
