#!/usr/bin/env bun
import { render } from 'ink';
import React from 'react';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { App } from './app-ink.js';

function parseArgs(): { dataDir: string; serverName?: string } {
	const args = process.argv.slice(2);
	let dataDir = join(homedir(), '.simse');

	for (let i = 0; i < args.length; i++) {
		if (args[i] === '--data-dir' && args[i + 1]) {
			dataDir = args[i + 1]!;
			i++;
		}
	}

	return { dataDir };
}

const { dataDir, serverName } = parseArgs();

if (!process.stdin.isTTY) {
	console.error(
		'Error: simse-code requires an interactive terminal (TTY).\n' +
			'Use "bun run start:legacy" for non-interactive mode.',
	);
	process.exit(1);
}

render(<App dataDir={dataDir} serverName={serverName} />);
