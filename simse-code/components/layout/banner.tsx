import { Box, Text, useStdout } from 'ink';
import { useMemo } from 'react';

const MASCOT_LINES = ['╭──╮', '╰─╮│', '  ╰╯'];
const MASCOT_COLOR = '#00afd7';
const DIVIDER = '─';

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
		const boxWidth = Math.min(cols - 2, 72);
		const leftColWidth = Math.floor(boxWidth * 0.45);
		const rightColWidth = boxWidth - leftColWidth - 3; // 3 for " │ "

		// Title bar: ── simse-code v1.0.0 ──────────
		const titleText = ` simse-code v${version} `;
		const titlePadding = Math.max(0, boxWidth - titleText.length);
		const titleLine = `${DIVIDER.repeat(2)}${titleText}${DIVIDER.repeat(titlePadding)}`;

		// Bottom border
		const bottomLine = DIVIDER.repeat(boxWidth + 2);

		// Build left column lines
		const leftLines: ColumnLine[] = [];
		leftLines.push({ text: '', isMascot: false, isBold: false, isDim: false });

		for (const ml of MASCOT_LINES) {
			const pad = Math.max(0, Math.floor((leftColWidth - ml.length) / 2));
			leftLines.push({
				text: ' '.repeat(pad) + ml,
				isMascot: true,
				isBold: false,
				isDim: false,
			});
		}
		leftLines.push({ text: '', isMascot: false, isBold: false, isDim: false });

		const modelLabel = server
			? model
				? `${server}: ${model}`
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
		rightLines.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		});

		const tipList = tips ?? DEFAULT_TIPS;
		rightLines.push({
			text: 'Tips for getting started',
			isMascot: false,
			isBold: true,
			isDim: false,
		});
		for (const tip of tipList) {
			rightLines.push({
				text: tip,
				isMascot: false,
				isBold: false,
				isDim: false,
			});
		}
		rightLines.push({
			text: DIVIDER.repeat(Math.min(rightColWidth, 28)),
			isMascot: false,
			isBold: false,
			isDim: true,
		});

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
		}[] = [];

		for (let i = 0; i < maxRows; i++) {
			const left = leftLines[i] ?? emptyLine;
			const right = rightLines[i] ?? emptyLine;
			const leftPad = Math.max(0, leftColWidth - left.text.length);
			rows.push({
				leftText: left.text,
				leftStyle: left,
				rightText: right.text,
				rightStyle: right,
				leftPad,
			});
		}

		return { titleLine, bottomLine, rows };
	}, [version, workDir, server, model, tips, recentActivity, cols]);

	return (
		<Box flexDirection="column" marginBottom={1}>
			<Text dimColor> {layout.titleLine}</Text>
			{layout.rows.map((row, i) => (
				<Box key={i}>
					<Text>
						{' '}
						{row.leftStyle.isMascot ? (
							<Text color={MASCOT_COLOR}>{row.leftText}</Text>
						) : row.leftStyle.isDim ? (
							<Text dimColor>{row.leftText}</Text>
						) : (
							row.leftText
						)}
						{' '.repeat(row.leftPad)}
					</Text>
					<Text dimColor> │ </Text>
					<Text>
						{row.rightStyle.isBold ? (
							<Text bold>{row.rightText}</Text>
						) : row.rightStyle.isDim ? (
							<Text dimColor>{row.rightText}</Text>
						) : (
							row.rightText
						)}
					</Text>
				</Box>
			))}
			<Text dimColor> {layout.bottomLine}</Text>
			<Text> </Text>
			<Text>
				{' '}
				<Text dimColor>▢</Text> Try{' '}
				<Text color="cyan">&quot;add {'<text>'}&quot;</Text> to save a
				note
			</Text>
			<Text> </Text>
			<Text>
				{' '}
				<Text dimColor>?</Text> for shortcuts
			</Text>
		</Box>
	);
}
