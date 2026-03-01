import { Box, Text } from 'ink';

interface SearchResult {
	readonly id: string;
	readonly text: string;
	readonly topic: string;
	readonly score: number;
}

interface SearchResultsProps {
	readonly results: readonly SearchResult[];
	readonly query: string;
}

export function SearchResults({ results, query }: SearchResultsProps) {
	if (results.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>No results for "{query}"</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>
				{results.length} result{results.length === 1 ? '' : 's'} for "{query}"
			</Text>
			{results.map((r) => (
				<Box key={r.id} flexDirection="column" marginTop={1}>
					<Box gap={2}>
						<Text dimColor>[{r.id.slice(0, 8)}]</Text>
						<Text bold color="cyan">
							{r.topic}
						</Text>
						<Text dimColor>{r.score.toFixed(3)}</Text>
					</Box>
					<Text wrap="truncate-end">{r.text.slice(0, 200)}</Text>
				</Box>
			))}
		</Box>
	);
}

interface VolumeListProps {
	readonly volumes: readonly {
		id: string;
		text: string;
		topic: string;
		createdAt?: number;
	}[];
	readonly topic?: string;
}

export function VolumeList({ volumes, topic }: VolumeListProps) {
	if (volumes.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>
					{topic ? `No volumes in "${topic}"` : 'No volumes'}
				</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>
				{volumes.length} volume{volumes.length === 1 ? '' : 's'}
				{topic ? ` in "${topic}"` : ''}
			</Text>
			{volumes.map((n) => (
				<Box key={n.id} gap={2}>
					<Text dimColor>[{n.id.slice(0, 8)}]</Text>
					<Text wrap="truncate-end">{n.text.slice(0, 100)}</Text>
				</Box>
			))}
		</Box>
	);
}

interface TopicListProps {
	readonly topics: readonly { name: string; count: number }[];
}

export function TopicList({ topics }: TopicListProps) {
	if (topics.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>No topics</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>
				{topics.length} topic{topics.length === 1 ? '' : 's'}
			</Text>
			{topics.map((t) => (
				<Box key={t.name} gap={2}>
					<Text color="cyan">{t.name}</Text>
					<Text dimColor>({t.count})</Text>
				</Box>
			))}
		</Box>
	);
}
