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
	readonly color?: string;
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
		const leftColWidth = Math.floor(contentWidth * 0.27);
		const gapWidth = 3; // " │ "
		const rightColWidth = contentWidth - leftColWidth - gapWidth;

		// Helper: center text within a given width
		const centerPad = (text: string, width: number): number => {
			return Math.max(0, Math.floor((width - text.length) / 2));
		};

		// Title: " ╭── simse-code v1.0.0 ──────...──╮"
		const titleLabel = `simse-code v${version}`;
		// ╭ + ── + space + title + space + trailer + ╮ = contentWidth
		const titleTrailerLen = Math.max(
			0,
			contentWidth - 1 - 2 - 1 - titleLabel.length - 1 - 1,
		);

		// Bottom border: ╰──...──╯ fills contentWidth
		const bottomInner = contentWidth - 2; // minus ╰ and ╯

		// Build left column content (raw, no padding yet)
		const leftContent: ColumnLine[] = [];

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

		// Build right column lines (no leading empty line — start immediately)
		const rightLines: ColumnLine[] = [];

		// Tips section header
		const tipList = tips ?? DEFAULT_TIPS;
		rightLines.push({
			text: 'Tips for getting started',
			isMascot: false,
			isBold: true,
			isDim: false,
			color: 'green',
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

		// Colored separator between sections
		rightLines.push({
			text: DIVIDER.repeat(rightColWidth),
			isMascot: false,
			isBold: false,
			isDim: false,
			color: '#d77757',
		});

		// Recent activity section
		const activity = recentActivity ?? ['No recent activity'];
		rightLines.push({
			text: 'Recent activity',
			isMascot: false,
			isBold: true,
			isDim: false,
			color: 'yellow',
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
		}[] = [];

		for (let i = 0; i < totalRows; i++) {
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
			bottomInner,
			rows,
		};
	}, [version, workDir, server, model, tips, recentActivity, cols]);

	return (
		<Box flexDirection="column" marginBottom={1}>
			{/* Title: " ╭── simse-code v1.0.0 ──────...──╮" */}
			<Text>
				{' '}
				<Text dimColor>
					{'\u256D'}{DIVIDER.repeat(2)}
				</Text>{' '}
				{layout.titleLabel}{' '}
				<Text dimColor>
					{DIVIDER.repeat(layout.titleTrailerLen)}{'\u256E'}
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
						{row.rightStyle.color ? (
							<Text
								bold={row.rightStyle.isBold}
								color={row.rightStyle.color}
							>
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
				</Box>
			))}

			{/* Bottom border: ╰──...──╯ */}
			<Text>
				{' '}
				<Text dimColor>
					{'\u2570'}{DIVIDER.repeat(layout.bottomInner)}{'\u256F'}
				</Text>
			</Text>

		</Box>
	);
}
