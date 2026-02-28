/**
 * Shared types for the Ink-based CLI.
 */

import type { ReactNode } from 'react';
import type { ImageAttachment } from './image-input.js';

// ---------------------------------------------------------------------------
// Command System
// ---------------------------------------------------------------------------

export type CommandCategory =
	| 'ai'
	| 'library'
	| 'tools'
	| 'session'
	| 'files'
	| 'config'
	| 'meta';

export interface CommandResult {
	/** React element to render in the output area. */
	readonly element?: ReactNode;
	/** Plain text output (rendered as <Text>). */
	readonly text?: string;
}

export interface CommandDefinition {
	readonly name: string;
	readonly aliases?: readonly string[];
	readonly usage: string;
	readonly description: string;
	readonly category: CommandCategory;
	/** Execute the command. Return result to render or undefined for no output. */
	readonly execute: (
		args: string,
	) => CommandResult | Promise<CommandResult> | undefined;
}

// ---------------------------------------------------------------------------
// Output Items (rendered in <Static> after completion)
// ---------------------------------------------------------------------------

export type OutputItem =
	| {
			readonly kind: 'message';
			readonly role: 'user' | 'assistant';
			readonly text: string;
			readonly images?: readonly ImageAttachment[];
	  }
	| {
			readonly kind: 'tool-call';
			readonly name: string;
			readonly args: string;
			readonly status: 'completed' | 'failed';
			readonly duration?: number;
			readonly summary?: string;
			readonly error?: string;
			readonly diff?: string;
	  }
	| { readonly kind: 'command-result'; readonly element: ReactNode }
	| { readonly kind: 'error'; readonly message: string }
	| { readonly kind: 'info'; readonly text: string };

// ---------------------------------------------------------------------------
// Tool Call State (for active area rendering)
// ---------------------------------------------------------------------------

export interface ToolCallState {
	readonly id: string;
	readonly name: string;
	readonly args: string;
	readonly status: 'active' | 'completed' | 'failed';
	readonly startedAt: number;
	readonly duration?: number;
	readonly summary?: string;
	readonly error?: string;
	readonly diff?: string;
}

// ---------------------------------------------------------------------------
// Permission Dialog
// ---------------------------------------------------------------------------

export interface PermissionRequest {
	readonly id: string;
	readonly toolName: string;
	readonly args: Record<string, unknown>;
	readonly options: readonly PermissionOption[];
}

export interface PermissionOption {
	readonly id: string;
	readonly label: string;
}
