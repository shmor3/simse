import { Box, Text, useStdout } from 'ink';
import React, { useMemo } from 'react';

const MASCOT_LINES = ['╭──╮', '╰─╮│', '  ╰╯'];
const MASCOT_COLOR = '#00afd7'; // ANSI 256-color 38 (deep cyan / teal)
const DIVIDER = '─';

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly noteCount?: number;
	readonly toolCount?: number;
	readonly agentCount?: number;
	readonly tips?: readonly string[];
	readonly recentActivity?: readonly string[];
}

interface RowData {
	readonly leftText: string;
	readonly leftIsMascot: boolean;
	readonly leftIsDim: boolean;
	readonly rightText: string;
	readonly rightIsBold: boolean;
	readonly rightIsDim: boolean;
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

		// Title bar
		const titleText = ` simse-code v${version} `;
		const titlePadding = Math.max(0, boxWidth - titleText.length);
		const titleLine = `${DIVIDER.repeat(2)}${titleText}${DIVIDER.repeat(titlePadding)}`;

		// Bottom border
		const bottomLine = DIVIDER.repeat(boxWidth + 2);

		// Build left column
		type LeftLine = { text: string; isMascot: boolean; isDim: boolean };
		const leftLines: LeftLine[] = [];
		leftLines.push({ text: '', isMascot: false, isDim: false });

		for (const ml of MASCOT_LINES) {
			const pad = Math.max(0, Math.floor((leftColWidth - ml.length) / 2));
			leftLines.push({
				text: ' '.repeat(pad) + ml,
				isMascot: true,
				isDim: false,
			});
		}
		leftLines.push({ text: '', isMascot: false, isDim: false });

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
			isDim: true,
		});

		// Build right column
		type RightLine = { text: string; isBold: boolean; isDim: boolean };
		const rightLines: RightLine[] = [];
		rightLines.push({ text: '', isBold: false, isDim: false });

		const tipList = tips ?? [
			'Run /help for all commands',
			'Use /add <text> to save a note',
			'Use /search <query> to find notes',
		];
		rightLines.push({
			text: 'Tips for getting started',
			isBold: true,
			isDim: false,
		});
		for (const tip of tipList) {
			rightLines.push({ text: tip, isBold: false, isDim: false });
		}
		rightLines.push({
			text: DIVIDER.repeat(Math.min(rightColWidth, 28)),
			isBold: false,
			isDim: true,
		});

		const activity = recentActivity ?? ['No recent activity'];
		rightLines.push({
			text: 'Recent activity',
			isBold: true,
			isDim: false,
		});
		for (const item of activity) {
			rightLines.push({ text: item, isBold: false, isDim: true });
		}

		// Merge columns
		const maxRows = Math.max(leftLines.length, rightLines.length);
		const rows: RowData[] = [];

		for (let i = 0; i < maxRows; i++) {
			const left: LeftLine = leftLines[i] ?? {
				text: '',
				isMascot: false,
				isDim: false,
			};
			const right: RightLine = rightLines[i] ?? {
				text: '',
				isBold: false,
				isDim: false,
			};

			const leftPad = Math.max(0, leftColWidth - left.text.length);
			rows.push({
				leftText: left.text + ' '.repeat(leftPad),
				leftIsMascot: left.isMascot,
				leftIsDim: left.isDim,
				rightText: right.text,
				rightIsBold: right.isBold,
				rightIsDim: right.isDim,
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
						{row.leftIsMascot ? (
							<Text color={MASCOT_COLOR}>{row.leftText}</Text>
						) : row.leftIsDim ? (
							<Text dimColor>{row.leftText}</Text>
						) : (
							row.leftText
						)}
					</Text>
					<Text dimColor> │ </Text>
					<Text>
						{row.rightIsBold ? (
							<Text bold>{row.rightText}</Text>
						) : row.rightIsDim ? (
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
