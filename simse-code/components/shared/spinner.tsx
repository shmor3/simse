import { Box, Text } from 'ink';
import { useEffect, useRef, useState } from 'react';

export function formatDuration(ms: number): string {
	if (ms < 1000) return `${Math.round(ms)}ms`;
	if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
	const minutes = Math.floor(ms / 60000);
	const seconds = Math.round((ms % 60000) / 1000);
	return `${minutes}m${seconds}s`;
}

export function formatTokens(tokens: number): string {
	if (tokens >= 1000) return `${(tokens / 1000).toFixed(1)}k tokens`;
	return `${tokens} tokens`;
}

// Claude Code spinner characters (Windows-safe variant)
const SPINNER_CHARS = ['\u00b7', '\u2722', '*', '\u2736', '\u273B', '\u273D'];
const SPINNER_INTERVAL = 120;

const THINKING_VERBS = [
	'Thinking',
	'Pondering',
	'Brewing',
	'Cooking',
	'Crafting',
	'Computing',
	'Processing',
	'Analyzing',
	'Hatching',
	'Mulling',
	'Generating',
	'Composing',
	'Synthesizing',
	'Deliberating',
	'Considering',
	'Noodling',
	'Percolating',
	'Simmering',
	'Working',
	'Conjuring',
	'Channeling',
	'Cogitating',
	'Ruminating',
	'Contemplating',
	'Incubating',
];

function useSpinner(): string {
	const [frame, setFrame] = useState(0);
	const dirRef = useRef(1);

	useEffect(() => {
		const timer = setInterval(() => {
			setFrame((prev) => {
				const next = prev + dirRef.current;
				if (next >= SPINNER_CHARS.length - 1) {
					dirRef.current = -1;
					return SPINNER_CHARS.length - 1;
				}
				if (next <= 0) {
					dirRef.current = 1;
					return 0;
				}
				return next;
			});
		}, SPINNER_INTERVAL);

		return () => clearInterval(timer);
	}, []);

	return SPINNER_CHARS[frame] ?? '\u00b7';
}

interface ThinkingSpinnerProps {
	readonly label?: string;
	readonly tokens?: number;
	readonly server?: string;
	readonly elapsed?: number;
}

export function ThinkingSpinner({
	label,
	tokens,
	server,
	elapsed,
}: ThinkingSpinnerProps) {
	// Pick a random verb on mount
	const [verb] = useState(
		() => THINKING_VERBS[Math.floor(Math.random() * THINKING_VERBS.length)],
	);
	const char = useSpinner();
	const displayLabel = label ?? verb ?? 'Thinking';

	const suffixParts: string[] = [];
	if (elapsed !== undefined) suffixParts.push(formatDuration(elapsed));
	if (tokens !== undefined && tokens > 0)
		suffixParts.push(formatTokens(tokens));
	if (server) suffixParts.push(server);

	const suffix =
		suffixParts.length > 0 ? ` (${suffixParts.join(' \u00b7 ')})` : '';

	return (
		<Box paddingLeft={2} gap={1}>
			<Text color="magenta">{char}</Text>
			<Text dimColor>
				{displayLabel}...{suffix}
			</Text>
		</Box>
	);
}
