import { Box, Text } from 'ink';

const MASCOT_LINES = ['╭──╮', '╰─╮│', '  ╰╯'];
const MASCOT_COLOR = '#00afd7';

const DEFAULT_TIPS: readonly string[] = [
	'Run /help for all commands',
	'Use /add <text> to save a note',
];

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly tips?: readonly string[];
}

export function Banner({ version, workDir, server, model, tips }: BannerProps) {
	const tipList = tips ?? DEFAULT_TIPS;

	const modelLabel = server ? (model ? `${server}: ${model}` : server) : model;

	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor="gray"
			paddingX={2}
			paddingY={0}
		>
			{/* Top row: mascot + tips */}
			<Box>
				<Box flexDirection="column" marginRight={4}>
					{MASCOT_LINES.map((line) => (
						<Text key={line} color={MASCOT_COLOR}>
							{line}
						</Text>
					))}
				</Box>
				<Box flexDirection="column">
					<Text bold>Tips</Text>
					{tipList.map((tip) => (
						<Text key={tip}>{tip}</Text>
					))}
				</Box>
			</Box>

			{/* Blank separator */}
			<Text> </Text>

			{/* Bottom section: version, model, workDir */}
			<Text>simse-code v{version}</Text>
			{modelLabel ? <Text>{modelLabel}</Text> : null}
			<Text dimColor>{workDir}</Text>
		</Box>
	);
}
