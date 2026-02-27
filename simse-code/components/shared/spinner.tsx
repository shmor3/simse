import { Box, Text } from 'ink';
import InkSpinner from 'ink-spinner';

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

interface ThinkingSpinnerProps {
	readonly label?: string;
	readonly tokens?: number;
	readonly server?: string;
	readonly elapsed?: number;
}

export function ThinkingSpinner({
	label = 'Thinking',
	tokens,
	server,
	elapsed,
}: ThinkingSpinnerProps) {
	const parts: string[] = [];
	if (elapsed !== undefined) parts.push(formatDuration(elapsed));
	if (tokens !== undefined) parts.push(formatTokens(tokens));
	if (server) parts.push(server);

	const suffix = parts.length > 0 ? ` (${parts.join(' \u00b7 ')})` : '';

	return (
		<Box paddingLeft={2} gap={1}>
			<Text color="magenta">
				<InkSpinner type="dots" />
			</Text>
			<Text>
				<Text dimColor>{label}...</Text>
				{suffix && <Text dimColor>{suffix}</Text>}
			</Text>
		</Box>
	);
}
