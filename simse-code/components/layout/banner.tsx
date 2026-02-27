import { Box, Text, useStdout } from 'ink';
import { useMemo } from 'react';

const MASCOT_LINES = ['\u256D\u2500\u2500\u256E', '\u2570\u2500\u256E\u2502', '  \u2570\u256F'];
const MASCOT_COLOR = '#00afd7';
const DIVIDER = '\u2500';

const DEFAULT_TIPS: readonly string[] = [
	'Run /help for all commands',
	'Use /add <text> to save a note',
	'Use /search <query> to find notes',
];

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly tips?: readonly string[];
	readonly recentActivity?: readonly string[];
}

interface ColumnLine {
	readonly text: string;
	readonly isMascot: boolean;
	readonly isBold: boolean;
	readonly isDim: boolean;
}

export function Banner({
	version,
	workDir,
	server,
	model,
	tips,
	recentActivity,
}: BannerProps) {
	const { stdout } = useStdout();
	const cols = stdout?.columns ?? 80;

	const layout = useMemo(() => {
		// 1-char left margin like Claude Code, content fills rest
		const contentWidth = cols - 1;
		const leftColWidth = Math.floor(contentWidth * 0.35);
		const gapWidth = 3; // " \u2502 "
		const rightColWidth = contentWidth - leftColWidth - gapWidth;

		// Title: " \u2500\u2500 simse-code v1.0.0 \u2500\u2500...\u2500\u2500"
		const titleLabel = `simse-code v${version}`;
		const titleTrailerLen = Math.max(
			0,
			contentWidth - 2 - 1 - titleLabel.length - 1,
		);

		// Bottom border: fills contentWidth
		const bottomLine = DIVIDER.repeat(contentWidth);

		// Build left column lines
		const leftLines: ColumnLine[] = [];

		// Empty line at top for spacing
		leftLines.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		});

		// Mascot centered
		for (const ml of MASCOT_LINES) {
			const pad = Math.max(
				0,
				Math.floor((leftColWidth - ml.length) / 2),
			);
			leftLines.push({
				text: ' '.repeat(pad) + ml,
				isMascot: true,
				isBold: false,
				isDim: false,
			});
		}

		// Empty line after mascot
		leftLines.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		});

		// Model label centered (e.g. "Opus 4.6 \u00b7 Claude Max")
		const modelLabel = server
			? model
				? `${server} \u00b7 ${model}`
				: server
			: model;
		if (modelLabel) {
			const pad = Math.max(
				0,
				Math.floor((leftColWidth - modelLabel.length) / 2),
			);
			leftLines.push({
				text: ' '.repeat(pad) + modelLabel,
				isMascot: false,
				isBold: false,
				isDim: false,
			});
		}

		// Working dir centered, dim
		const workDirTrunc =
			workDir.length > leftColWidth
				? `...${workDir.slice(-(leftColWidth - 3))}`
				: workDir;
		const wdPad = Math.max(
			0,
			Math.floor((leftColWidth - workDirTrunc.length) / 2),
		);
		leftLines.push({
			text: ' '.repeat(wdPad) + workDirTrunc,
			isMascot: false,
			isBold: false,
			isDim: true,
		});

		// Build right column lines
		const rightLines: ColumnLine[] = [];

		// Empty line at top for spacing
		rightLines.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		});

		// Tips section
		const tipList = tips ?? DEFAULT_TIPS;
		rightLines.push({
			text: 'Tips for getting started',
			isMascot: false,
			isBold: true,
			isDim: false,
		});
		for (const tip of tipList) {
			const truncated =
				tip.length > rightColWidth
					? `${tip.slice(0, rightColWidth - 3)}...`
					: tip;
			rightLines.push({
				text: truncated,
				isMascot: false,
				isBold: false,
				isDim: false,
			});
		}

		// Section divider
		rightLines.push({
			text: DIVIDER.repeat(rightColWidth),
			isMascot: false,
			isBold: false,
			isDim: true,
		});

		// Recent activity section
		const activity = recentActivity ?? ['No recent activity'];
		rightLines.push({
			text: 'Recent activity',
			isMascot: false,
			isBold: true,
			isDim: false,
		});
		for (const item of activity) {
			rightLines.push({
				text: item,
				isMascot: false,
				isBold: false,
				isDim: true,
			});
		}

		// Merge columns into rows
		const maxRows = Math.max(leftLines.length, rightLines.length);
		const emptyLine: ColumnLine = {
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		};

		const rows: {
			leftText: string;
			leftStyle: ColumnLine;
			rightText: string;
			rightStyle: ColumnLine;
			leftPad: number;
			rightPad: number;
		}[] = [];

		for (let i = 0; i < maxRows; i++) {
			const left = leftLines[i] ?? emptyLine;
			const right = rightLines[i] ?? emptyLine;
			const leftPad = Math.max(0, leftColWidth - left.text.length);
			const rightPad = Math.max(0, rightColWidth - right.text.length);
			rows.push({
				leftText: left.text,
				leftStyle: left,
				rightText: right.text,
				rightStyle: right,
				leftPad,
				rightPad,
			});
		}

		return {
			titleLabel,
			titleTrailerLen,
			bottomLine,
			rows,
		};
	}, [version, workDir, server, model, tips, recentActivity, cols]);

	return (
		<Box flexDirection="column" marginBottom={1}>
			{/* Title: " ── simse-code v1.0.0 ──────...──" */}
			<Text>
				{' '}
				<Text dimColor>{DIVIDER.repeat(2)}</Text>{' '}
				{layout.titleLabel}{' '}
				<Text dimColor>
					{DIVIDER.repeat(layout.titleTrailerLen)}
				</Text>
			</Text>

			{/* Two-column content rows */}
			{layout.rows.map((row, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: static layout
				<Box key={i}>
					<Text>
						{' '}
						{row.leftStyle.isMascot ? (
							<Text color={MASCOT_COLOR}>
								{row.leftText}
							</Text>
						) : row.leftStyle.isDim ? (
							<Text dimColor>{row.leftText}</Text>
						) : (
							row.leftText
						)}
						{' '.repeat(row.leftPad)}
					</Text>
					<Text dimColor> {'\u2502'} </Text>
					<Text>
						{row.rightStyle.isBold ? (
							<Text bold>{row.rightText}</Text>
						) : row.rightStyle.isDim ? (
							<Text dimColor>{row.rightText}</Text>
						) : (
							row.rightText
						)}
						{' '.repeat(row.rightPad)}
					</Text>
				</Box>
			))}

			{/* Bottom border */}
			<Text>
				{' '}
				<Text dimColor>{layout.bottomLine}</Text>
			</Text>

			{/* Hint lines */}
			<Text> </Text>
			<Text>
				{' '}
				<Text dimColor>{'\u25A2'}</Text> Try{' '}
				<Text color="cyan">
					&quot;add {'<text>'}&quot;
				</Text>{' '}
				to save a note
			</Text>
			<Text> </Text>
			<Text>
				{' '}
				<Text dimColor>?</Text> for shortcuts
			</Text>
		</Box>
	);
}
