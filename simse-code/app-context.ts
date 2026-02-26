/**
 * SimSE Code — Application Context
 *
 * Central AppContext interface used by all features and commands.
 * Extended from the original cli.ts AppContext with new feature fields.
 */

import type { Interface as ReadlineInterface } from 'node:readline';
import type { ACPClient, VFSDisk, VirtualFS } from 'simse';
import type { KnowledgeBaseApp } from './app.js';
import type { CLIConfigResult } from './config.js';
import type { Conversation } from './conversation.js';
import type { KeybindingManager } from './keybindings.js';
import type { SkillRegistry } from './skills.js';
import type { ToolRegistry } from './tool-registry.js';
import type { MarkdownRenderer, Spinner, TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Session State
// ---------------------------------------------------------------------------

export interface SessionState {
	serverName: string | undefined;
	agentName: string | undefined;
	memoryEnabled: boolean;
	bypassPermissions: boolean;
	maxTurns: number;
	totalTurns: number;
	abortController: AbortController | undefined;
	startedAt: number;
	lastUserInput: string | undefined;
}

// ---------------------------------------------------------------------------
// Permission Modes
// ---------------------------------------------------------------------------

export type PermissionMode = 'default' | 'acceptEdits' | 'plan' | 'dontAsk';

// ---------------------------------------------------------------------------
// Feature Interfaces (forward declarations for optional features)
// ---------------------------------------------------------------------------

export interface FileTracker {
	readonly track: (
		path: string,
		additions: number,
		deletions: number,
		isNew: boolean,
	) => void;
	readonly getChanges: () => readonly FileChange[];
	readonly getTotals: () => { additions: number; deletions: number };
	readonly clear: () => void;
}

export interface FileChange {
	readonly path: string;
	readonly additions: number;
	readonly deletions: number;
	readonly isNew: boolean;
}

export interface VerboseState {
	isVerbose: boolean;
	readonly toggle: () => void;
	readonly set: (value: boolean) => void;
}

export interface PlanModeState {
	isActive: boolean;
	readonly toggle: () => void;
	readonly set: (value: boolean) => void;
}

export interface StatusLine {
	readonly render: () => void;
	readonly update: (data: Partial<StatusLineData>) => void;
}

export interface StatusLineData {
	readonly model: string;
	readonly contextPercent: number;
	readonly costEstimate: string;
	readonly additions: number;
	readonly deletions: number;
	readonly permissionMode: PermissionMode;
	readonly planMode: boolean;
	readonly bgTaskCount: number;
	readonly todoCount: number;
	readonly todoDone: number;
}

export interface UsageTracker {
	readonly record: (event: UsageEvent) => void;
	readonly getToday: () => DailyUsage;
	readonly getHistory: (days: number) => readonly DailyUsage[];
	readonly getTotals: () => UsageTotals;
}

export interface UsageEvent {
	readonly type: 'message' | 'tool_call';
	readonly model?: string;
	readonly tokensEstimate?: number;
}

export interface DailyUsage {
	readonly date: string;
	readonly sessions: number;
	readonly messages: number;
	readonly toolCalls: number;
	readonly tokensEstimate: number;
}

export interface UsageTotals {
	readonly totalSessions: number;
	readonly totalMessages: number;
	readonly totalToolCalls: number;
	readonly totalTokens: number;
}

export interface SessionStore {
	readonly save: (session: SessionRecord) => void;
	readonly load: (id: string) => SessionRecord | undefined;
	readonly list: () => readonly SessionSummary[];
	readonly remove: (id: string) => void;
}

export interface SessionRecord {
	readonly id: string;
	readonly createdAt: number;
	readonly updatedAt: number;
	readonly model: string;
	readonly directory: string;
	readonly branch?: string;
	readonly messages: readonly unknown[];
	readonly metadata?: Readonly<Record<string, unknown>>;
}

export interface SessionSummary {
	readonly id: string;
	readonly createdAt: number;
	readonly updatedAt: number;
	readonly model: string;
	readonly directory: string;
	readonly messageCount: number;
}

export interface CheckpointManager {
	readonly save: (label?: string) => string;
	readonly rewind: (id: string) => boolean;
	readonly list: () => readonly CheckpointSummary[];
	readonly clear: () => void;
	readonly lastCheckpoint: () => CheckpointSummary | undefined;
}

export interface CheckpointSummary {
	readonly id: string;
	readonly label?: string;
	readonly timestamp: number;
	readonly messageCount: number;
}

export interface BackgroundManager {
	readonly background: (label: string, promise: Promise<unknown>) => string;
	readonly foreground: (id: string) => Promise<unknown> | undefined;
	readonly list: () => readonly BackgroundTask[];
	readonly abort: (id: string) => void;
	readonly activeCount: () => number;
}

export interface BackgroundTask {
	readonly id: string;
	readonly label: string;
	readonly startedAt: number;
	readonly status: 'running' | 'completed' | 'failed';
}

export interface ThemeManager {
	readonly getActive: () => Theme;
	readonly setActive: (name: string) => boolean;
	readonly list: () => readonly string[];
}

export interface Theme {
	readonly name: string;
	readonly colors: ThemeColors;
}

export interface ThemeColors {
	readonly ui: Readonly<Record<string, string>>;
	readonly diff: {
		readonly add: string;
		readonly remove: string;
		readonly context: string;
	};
	readonly syntax: Readonly<Record<string, string>>;
}

// ---------------------------------------------------------------------------
// App Context
// ---------------------------------------------------------------------------

/** Return string to print, undefined for no output, null to signal exit. */
export type CommandHandler = (
	ctx: AppContext,
	rest: string,
) => Promise<string | null | undefined> | string | null | undefined;

export interface Command {
	readonly name: string;
	readonly aliases?: readonly string[];
	readonly usage: string;
	readonly description: string;
	readonly category: 'notes' | 'ai' | 'tools' | 'info' | 'session';
	readonly handler: CommandHandler;
}

export interface AppContext {
	readonly app: KnowledgeBaseApp;
	readonly acpClient: ACPClient;
	readonly configResult: CLIConfigResult;
	readonly dataDir: string;
	readonly vfs: VirtualFS;
	readonly disk: VFSDisk;
	readonly rl: ReadlineInterface;
	readonly colors: TermColors;
	readonly spinner: Spinner;
	readonly md: MarkdownRenderer;
	readonly session: SessionState;
	readonly toolRegistry: ToolRegistry;
	readonly conversation: Conversation;
	readonly skillRegistry: SkillRegistry;

	// Feature extensions (populated as features are wired in)
	readonly keybindings?: KeybindingManager;
	readonly fileTracker?: FileTracker;
	readonly verbose?: VerboseState;
	readonly planMode?: PlanModeState;
	readonly statusLine?: StatusLine;
	readonly usageTracker?: UsageTracker;
	readonly sessionStore?: SessionStore;
	readonly checkpointManager?: CheckpointManager;
	readonly backgroundManager?: BackgroundManager;
	readonly themeManager?: ThemeManager;
	readonly permissionMode?: PermissionMode;
}
