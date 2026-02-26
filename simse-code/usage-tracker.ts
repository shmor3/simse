/**
 * SimSE Code — Usage Tracker
 *
 * Tracks daily usage statistics: sessions, messages, tool calls,
 * token estimates. Persists to JSONL for append-only storage.
 * No external deps.
 */

import { join } from 'node:path';
import type {
	DailyUsage,
	UsageEvent,
	UsageTotals,
	UsageTracker,
} from './app-context.js';
import { appendJsonLine, readJsonLines } from './json-io.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { DailyUsage, UsageEvent, UsageTracker, UsageTotals };

export interface UsageTrackerOptions {
	readonly dataDir: string;
}

// ---------------------------------------------------------------------------
// Persistence format
// ---------------------------------------------------------------------------

interface UsageLogEntry {
	readonly date: string;
	readonly type: 'session_start' | 'message' | 'tool_call';
	readonly model?: string;
	readonly tokensEstimate?: number;
	readonly timestamp: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createUsageTracker(options: UsageTrackerOptions): UsageTracker {
	const logPath = join(options.dataDir, 'usage.jsonl');

	const today = (): string => {
		const d = new Date();
		return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
	};

	const record = (event: UsageEvent): void => {
		const entry: UsageLogEntry = {
			date: today(),
			type: event.type,
			model: event.model,
			tokensEstimate: event.tokensEstimate,
			timestamp: Date.now(),
		};
		appendJsonLine(logPath, entry);
	};

	const aggregateByDate = (
		entries: readonly UsageLogEntry[],
	): Map<string, DailyUsage> => {
		const map = new Map<
			string,
			{
				sessions: number;
				messages: number;
				toolCalls: number;
				tokensEstimate: number;
			}
		>();

		for (const entry of entries) {
			let day = map.get(entry.date);
			if (!day) {
				day = { sessions: 0, messages: 0, toolCalls: 0, tokensEstimate: 0 };
				map.set(entry.date, day);
			}

			switch (entry.type) {
				case 'session_start':
					day.sessions++;
					break;
				case 'message':
					day.messages++;
					day.tokensEstimate += entry.tokensEstimate ?? 0;
					break;
				case 'tool_call':
					day.toolCalls++;
					break;
			}
		}

		return new Map(
			Array.from(map.entries()).map(([date, data]) => [
				date,
				{ date, ...data },
			]),
		);
	};

	const getToday = (): DailyUsage => {
		const entries = readJsonLines<UsageLogEntry>(logPath);
		const todayStr = today();
		const todayEntries = entries.filter((e) => e.date === todayStr);
		const agg = aggregateByDate(todayEntries);
		return (
			agg.get(todayStr) ?? {
				date: todayStr,
				sessions: 0,
				messages: 0,
				toolCalls: 0,
				tokensEstimate: 0,
			}
		);
	};

	const getHistory = (days: number): readonly DailyUsage[] => {
		const entries = readJsonLines<UsageLogEntry>(logPath);
		const agg = aggregateByDate(entries);

		// Get last N days
		const result: DailyUsage[] = [];
		const now = new Date();
		for (let i = 0; i < days; i++) {
			const d = new Date(now);
			d.setDate(d.getDate() - i);
			const dateStr = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
			result.push(
				agg.get(dateStr) ?? {
					date: dateStr,
					sessions: 0,
					messages: 0,
					toolCalls: 0,
					tokensEstimate: 0,
				},
			);
		}

		return result.reverse(); // oldest first
	};

	const getTotals = (): UsageTotals => {
		const entries = readJsonLines<UsageLogEntry>(logPath);
		let totalSessions = 0;
		let totalMessages = 0;
		let totalToolCalls = 0;
		let totalTokens = 0;

		for (const entry of entries) {
			switch (entry.type) {
				case 'session_start':
					totalSessions++;
					break;
				case 'message':
					totalMessages++;
					totalTokens += entry.tokensEstimate ?? 0;
					break;
				case 'tool_call':
					totalToolCalls++;
					break;
			}
		}

		return { totalSessions, totalMessages, totalToolCalls, totalTokens };
	};

	return Object.freeze({ record, getToday, getHistory, getTotals });
}

// ---------------------------------------------------------------------------
// Stats display
// ---------------------------------------------------------------------------

/**
 * Render a 7-day usage chart with ASCII horizontal bars.
 */
export function renderUsageChart(
	history: readonly DailyUsage[],
	colors: {
		dim: (s: string) => string;
		green: (s: string) => string;
		cyan: (s: string) => string;
	},
): string {
	const lines: string[] = [];
	const maxMessages = Math.max(...history.map((d) => d.messages), 1);

	for (const day of history) {
		const date = day.date.slice(5); // MM-DD
		const barLen = Math.round((day.messages / maxMessages) * 30);
		const bar = '█'.repeat(barLen);
		const count = String(day.messages).padStart(4);
		lines.push(
			`  ${colors.dim(date)} ${colors.green(bar)} ${colors.cyan(count)}`,
		);
	}

	return lines.join('\n');
}
