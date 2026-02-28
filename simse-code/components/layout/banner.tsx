import { Box, Text, useStdout } from 'ink';
import { useMemo } from 'react';

const MASCOT_LINES = [
	'\u256D\u2500\u2500\u256E',
	'\u2570\u2500\u256E\u2502',
	'  \u2570\u256F',
];
const PRIMARY = '#87D6D6';
const SECONDARY = '#86d6ae';
const DIVIDER = '\u2500';

const DEFAULT_TIPS: readonly string[] = [
	'Run /help for all commands',
	'Use /add <text> to save a volume',
	'Use /search <query> to find volumes',
];

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly tips?: readonly string[];
	readonly recentActivity?: readonly string[];
	readonly greeting?: string;
}

interface ColumnLine {
	readonly text: string;
	readonly isMascot: boolean;
	readonly isBold: boolean;
	readonly isDim: boolean;
	readonly color?: string;
	/** When true, this row is rendered as a separator in the right column. */
	readonly isSeparator?: boolean;
}

export function Banner({
	version,
	workDir,
	server,
	model,
	tips,
	recentActivity,
	greeting,
}: BannerProps) {
	const { stdout } = useStdout();
	const cols = stdout?.columns ?? 80;

	const layout = useMemo(() => {
		// Row structure: margin(1) + │(1) + leftCol + " │ "(3) + rightCol + │(1)
		// So inner content width = cols - 1 (margin) - 1 (left │) - 1 (right │)
		const contentWidth = cols - 3;
		const leftColWidth = Math.floor(contentWidth * 0.27);
		const gapWidth = 3; // " │ "
		const rightColWidth = contentWidth - leftColWidth - gapWidth;

		// Helper: center text within a given width
		const centerPad = (text: string, width: number): number => {
			return Math.max(0, Math.floor((width - text.length) / 2));
		};

		// Title: " ╭── simse-code v1.0.0 ──────...──╮"
		// Title row = margin(1) + ╭(1) + ── + space + title + space + trailer + ╮(1)
		// Inner width between ╭ and ╮ = cols - 1 (margin) - 2 (╭ and ╮)
		const titleLabel = `simse-code v${version}`;
		const titleInner = cols - 3;
		const titleTrailerLen = Math.max(
			0,
			titleInner - 2 - 1 - titleLabel.length - 1,
		);

		// Bottom border inner width (between ╰ and ╯)
		const bottomInner = cols - 3;

		// Build left column content (raw, no padding yet)
		const leftContent: ColumnLine[] = [];

		// Greeting line (e.g. "Welcome back!")
		if (greeting) {
			leftContent.push({
				text: greeting,
				isMascot: false,
				isBold: true,
				isDim: false,
			});
			leftContent.push({
				text: '',
				isMascot: false,
				isBold: false,
				isDim: false,
			});
		}

		// Mascot lines
		for (const ml of MASCOT_LINES) {
			leftContent.push({
				text: ml,
				isMascot: true,
				isBold: false,
				isDim: false,
			});
		}

		// Empty line between mascot and info
		leftContent.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		});

		// Model label (e.g. "Opus 4.6 · Claude Max")
		const modelLabel = server
			? model
				? `${server} \u00b7 ${model}`
				: server
			: model;
		if (modelLabel) {
			leftContent.push({
				text: modelLabel,
				isMascot: false,
				isBold: false,
				isDim: false,
			});
		}

		// Working dir, dim
		const workDirTrunc =
			workDir.length > leftColWidth
				? `...${workDir.slice(-(leftColWidth - 3))}`
				: workDir;
		leftContent.push({
			text: workDirTrunc,
			isMascot: false,
			isBold: false,
			isDim: true,
		});

		// Build right column lines
		const rightLines: ColumnLine[] = [];

		// Tips section header
		const tipList = tips ?? DEFAULT_TIPS;
		rightLines.push({
			text: 'Tips for getting started',
			isMascot: false,
			isBold: true,
			isDim: false,
			color: SECONDARY,
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

		// Separator row between tips and recent
		rightLines.push({
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
			isSeparator: true,
		});

		// Recent activity section
		const activity = recentActivity ?? ['No recent activity'];
		rightLines.push({
			text: 'Recent activity',
			isMascot: false,
			isBold: true,
			isDim: false,
			color: PRIMARY,
		});
		for (const item of activity) {
			rightLines.push({
				text: item,
				isMascot: false,
				isBold: false,
				isDim: true,
			});
		}

		// Vertically center left content within right column height
		const totalRows = Math.max(leftContent.length, rightLines.length);
		const topPad = Math.max(
			0,
			Math.floor((totalRows - leftContent.length) / 2),
		);

		const emptyLine: ColumnLine = {
			text: '',
			isMascot: false,
			isBold: false,
			isDim: false,
		};

		// Build left column with vertical + horizontal centering
		const leftLines: ColumnLine[] = [];
		for (let i = 0; i < totalRows; i++) {
			const contentIdx = i - topPad;
			if (contentIdx >= 0 && contentIdx < leftContent.length) {
				// biome-ignore lint/style/noNonNullAssertion: index is bounds-checked by if condition above
				const line = leftContent[contentIdx]!;
				const lp = centerPad(line.text, leftColWidth);
				leftLines.push({
					...line,
					text: ' '.repeat(lp) + line.text,
				});
			} else {
				leftLines.push(emptyLine);
			}
		}

		// Merge columns into rows
		const rows: {
			leftText: string;
			leftStyle: ColumnLine;
			rightText: string;
			rightStyle: ColumnLine;
			leftPad: number;
			rightPad: number;
			isSeparator: boolean;
		}[] = [];

		for (let i = 0; i < totalRows; i++) {
			const left = leftLines[i] ?? emptyLine;
			const right = rightLines[i] ?? emptyLine;
			const isSep = right.isSeparator === true;
			const leftPad = Math.max(0, leftColWidth - left.text.length);
			const rightPad = isSep
				? 0
				: Math.max(0, rightColWidth - right.text.length);
			rows.push({
				leftText: left.text,
				leftStyle: left,
				rightText: right.text,
				rightStyle: right,
				leftPad,
				rightPad,
				isSeparator: isSep,
			});
		}

		return {
			titleLabel,
			titleTrailerLen,
			bottomInner,
			leftColWidth,
			rightColWidth,
			rows,
		};
	}, [version, workDir, server, model, tips, recentActivity, greeting, cols]);

	return (
		<Box flexDirection="column" marginBottom={1}>
			{/* Title: " ╭── simse-code v1.0.0 ──────...──╮" */}
			<Text>
				{' '}
				<Text color={PRIMARY}>
					{'\u256D'}
					{DIVIDER.repeat(2)}
				</Text>{' '}
				<Text color={PRIMARY}>{layout.titleLabel}</Text>{' '}
				<Text color={PRIMARY}>
					{DIVIDER.repeat(layout.titleTrailerLen)}
					{'\u256E'}
				</Text>
			</Text>

			{/* Two-column content rows with left/right borders */}
			{layout.rows.map((row, i) => {
				if (row.isSeparator) {
					// Separator row: │ <left content> │ ──────── │
					return (
						// biome-ignore lint/suspicious/noArrayIndexKey: static layout
						<Box key={i}>
							<Text>
								{' '}
								<Text color={PRIMARY}>{'\u2502'}</Text>
								{row.leftStyle.isMascot ? (
									<Text color={PRIMARY}>{row.leftText}</Text>
								) : row.leftStyle.isDim ? (
									<Text dimColor>{row.leftText}</Text>
								) : row.leftStyle.isBold ? (
									<Text bold>{row.leftText}</Text>
								) : (
									row.leftText
								)}
								{' '.repeat(row.leftPad)}
							</Text>
							<Text color={PRIMARY}> {'\u2502'} </Text>
							<Text color={PRIMARY}>
								{DIVIDER.repeat(Math.max(0, layout.rightColWidth - 1))}
							</Text>
							<Text>
								{' '}
								<Text color={PRIMARY}>{'\u2502'}</Text>
							</Text>
						</Box>
					);
				}

				return (
					// biome-ignore lint/suspicious/noArrayIndexKey: static layout
					<Box key={i}>
						<Text>
							{' '}
							<Text color={PRIMARY}>{'\u2502'}</Text>
							{row.leftStyle.isMascot ? (
								<Text color={PRIMARY}>{row.leftText}</Text>
							) : row.leftStyle.isDim ? (
								<Text dimColor>{row.leftText}</Text>
							) : row.leftStyle.isBold ? (
								<Text bold>{row.leftText}</Text>
							) : (
								row.leftText
							)}
							{' '.repeat(row.leftPad)}
						</Text>
						<Text color={PRIMARY}> {'\u2502'} </Text>
						<Text>
							{row.rightStyle.color ? (
								<Text bold={row.rightStyle.isBold} color={row.rightStyle.color}>
									{row.rightText}
								</Text>
							) : row.rightStyle.isBold ? (
								<Text bold>{row.rightText}</Text>
							) : row.rightStyle.isDim ? (
								<Text dimColor>{row.rightText}</Text>
							) : (
								row.rightText
							)}
							{' '.repeat(row.rightPad)}
						</Text>
						<Text color={PRIMARY}>{'\u2502'}</Text>
					</Box>
				);
			})}

			{/* Bottom border: ╰──...──╯ */}
			<Text>
				{' '}
				<Text color={PRIMARY}>
					{'\u2570'}
					{DIVIDER.repeat(layout.bottomInner)}
					{'\u256F'}
				</Text>
			</Text>
		</Box>
	);
}
