// simse-code/e2e/harness/keys.ts

export const KEYS = {
	enter: '\r',
	escape: '\x1b',
	tab: '\t',
	backspace: '\x7f',
	delete: '\x1b[3~',
	up: '\x1b[A',
	down: '\x1b[B',
	right: '\x1b[C',
	left: '\x1b[D',
	home: '\x1b[H',
	end: '\x1b[F',
	pageUp: '\x1b[5~',
	pageDown: '\x1b[6~',
} as const;

export type KeyName = keyof typeof KEYS;

export function ctrlKey(char: string): string {
	return String.fromCharCode(char.toLowerCase().charCodeAt(0) - 96);
}
