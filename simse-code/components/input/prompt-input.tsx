import { Box, Text, useInput } from 'ink';
import React, { useMemo, useState } from 'react';
import type { CommandDefinition } from '../../ink-types.js';
import { TextInput } from './text-input.js';

const PLACEHOLDER_TIPS: readonly string[] = [
	'Try "add a note about today\'s meeting"',
	'Try "search for deployment notes"',
	'Try "summarize my recent notes"',
	'Try "help me write a changelog"',
	'Try "find notes about authentication"',
	'Try "create a checklist for release"',
	'Try "what did I save last week?"',
	'Try "organize my project notes"',
	'Try "draft a quick status update"',
	'Try "list everything about the API"',
];

const SHORTCUTS: readonly { key: string; desc: string }[] = [
	{ key: '!', desc: 'bash mode' },
	{ key: '/', desc: 'commands' },
	{ key: '@', desc: 'file paths' },
	{ key: 'shift+\u21B5', desc: 'newline' },
	{ key: 'esc', desc: 'dismiss' },
	{ key: 'ctrl+c', desc: 'exit' },
	{ key: '/help', desc: 'all commands' },
	{ key: '?', desc: 'shortcuts' },
	{ key: 'ctrl+l', desc: 'clear' },
];

export type PromptMode = 'normal' | 'shortcuts' | 'autocomplete' | 'at-mention';

interface PromptInputProps {
	readonly onSubmit: (value: string) => void;
	readonly disabled?: boolean;
	readonly planMode?: boolean;
	readonly commands?: readonly CommandDefinition[];
	readonly onModeChange?: (mode: PromptMode) => void;
	readonly onCompleteAtMention?: (partial: string) => readonly string[];
}

export function PromptInput({
	onSubmit,
	disabled = false,
	planMode,
	commands = [],
	onModeChange,
	onCompleteAtMention,
}: PromptInputProps) {
	const [value, setValue] = useState('');
	const [mode, _setMode] = useState<PromptMode>('normal');

	const setMode = (next: PromptMode) => {
		_setMode(next);
		onModeChange?.(next);
	};
	const [selectedIndex, setSelectedIndex] = useState(0);
	const [placeholder] = useState(
		() =>
			PLACEHOLDER_TIPS[Math.floor(Math.random() * PLACEHOLDER_TIPS.length)]!,
	);

	// Extract the @partial from the end of the current value
	const atQuery = useMemo(() => {
		const m = value.match(/@(\S*)$/);
		return m ? m[1] : '';
	}, [value]);

	// Filter commands for autocomplete
	const filteredCommands = useMemo(() => {
		if (!value.startsWith('/')) return [];
		const filter = value.slice(1).toLowerCase();
		return commands
			.filter(
				(cmd) =>
					cmd.name.toLowerCase().includes(filter) ||
					cmd.aliases?.some((a) => a.toLowerCase().includes(filter)),
			)
			.slice(0, 8);
	}, [value, commands]);

	// @-mention completion candidates
	const atCandidates = useMemo(() => {
		if (mode !== 'at-mention' || !onCompleteAtMention) return [];
		return onCompleteAtMention(atQuery).slice(0, 8);
	}, [mode, atQuery, onCompleteAtMention]);

	// Ghost text suggestion: complete a slash command when there's exactly one match
	const suggestion = useMemo(() => {
		// @ ghost text
		if (mode === 'at-mention' && atCandidates.length === 1) {
			const candidate = atCandidates[0]!;
			if (candidate.startsWith(atQuery) && candidate !== atQuery) {
				return candidate.slice(atQuery.length);
			}
			return undefined;
		}

		if (!value.startsWith('/') || value.length < 2) return undefined;
		if (mode !== 'normal' && mode !== 'autocomplete') return undefined;
		const prefix = value.slice(1).toLowerCase();
		const matches = commands.filter((cmd) =>
			cmd.name.toLowerCase().startsWith(prefix),
		);
		if (matches.length === 1 && matches[0]!.name.toLowerCase() !== prefix) {
			return matches[0]!.name.slice(prefix.length);
		}
		return undefined;
	}, [value, commands, mode, atCandidates, atQuery]);

	const handleChange = (newValue: string) => {
		// Intercept `?` on empty input to show shortcuts
		if (newValue === '?' && value === '') {
			setMode('shortcuts');
			return;
		}

		setValue(newValue);

		// Check for @-mention at end of input
		const atMatch = newValue.match(/@(\S*)$/);
		if (atMatch && onCompleteAtMention) {
			if (mode !== 'at-mention') setMode('at-mention');
			setSelectedIndex(0);
			return;
		}

		// Check for / command autocomplete
		if (newValue.startsWith('/') && newValue.length >= 1) {
			if (mode !== 'autocomplete') setMode('autocomplete');
			setSelectedIndex(0);
		} else if (mode === 'autocomplete' || mode === 'at-mention') {
			setMode('normal');
		}
	};

	const handleSubmit = (input: string) => {
		if (!input.trim()) return;
		onSubmit(input);
		setValue('');
		setMode('normal');
	};

	// Handle special keys for shortcuts and autocomplete overlays
	useInput(
		(_input, key) => {
			if (mode === 'shortcuts') {
				// Any key dismisses shortcuts
				setMode('normal');
				return;
			}

			if (mode === 'at-mention') {
				if (key.escape) {
					// Remove the @partial from the end
					const cleaned = value.replace(/@\S*$/, '');
					setValue(cleaned);
					setMode('normal');
					return;
				}
				if (key.tab && atCandidates.length > 0) {
					const candidate = atCandidates[selectedIndex];
					if (candidate) {
						// Replace the @partial at end with @candidate
						const newVal = value.replace(/@\S*$/, `@${candidate}`);
						setValue(newVal);
						// Stay in at-mention mode if it's a directory
						if (candidate.endsWith('/')) {
							setSelectedIndex(0);
						} else {
							setMode('normal');
						}
					}
					return;
				}
				if (key.upArrow) {
					setSelectedIndex((i) => Math.max(0, i - 1));
					return;
				}
				if (key.downArrow) {
					setSelectedIndex((i) =>
						Math.min(atCandidates.length - 1, i + 1),
					);
					return;
				}
			}

			if (mode === 'autocomplete') {
				if (key.escape) {
					setValue('');
					setMode('normal');
					return;
				}
				if (key.tab && filteredCommands.length > 0) {
					const cmd = filteredCommands[selectedIndex];
					if (cmd) {
						onSubmit(`/${cmd.name}`);
						setValue('');
						setMode('normal');
					}
					return;
				}
				if (key.upArrow) {
					setSelectedIndex((i) => Math.max(0, i - 1));
					return;
				}
				if (key.downArrow) {
					setSelectedIndex((i) => Math.min(filteredCommands.length - 1, i + 1));
					return;
				}
			}
		},
		{ isActive: !disabled && mode !== 'normal' },
	);

	const borderColor = disabled ? 'gray' : planMode ? 'cyan' : 'gray';

	return (
		<Box flexDirection="column">
			{/* Input box with border */}
			<Box borderStyle="round" borderColor={borderColor} paddingX={1}>
				<Text bold color="cyan">
					{'\u276F'}
				</Text>
				<Text> </Text>
				<TextInput
					value={value}
					onChange={handleChange}
					onSubmit={handleSubmit}
					isActive={!disabled && mode !== 'shortcuts'}
					placeholder={placeholder}
					suggestion={suggestion}
				/>
			</Box>

			{/* Shortcuts overlay - below input */}
			{mode === 'shortcuts' && <ShortcutsPanel />}

			{/* Command autocomplete - below input */}
			{mode === 'autocomplete' && filteredCommands.length > 0 && (
				<Box flexDirection="column" paddingLeft={2}>
					{filteredCommands.map((cmd, i) => (
						<Box key={cmd.name}>
							<Text
								color={i === selectedIndex ? 'cyan' : undefined}
								bold={i === selectedIndex}
							>
								{i === selectedIndex ? '\u276F' : ' '} /{cmd.name.padEnd(24)}
							</Text>
							<Text dimColor>{cmd.description}</Text>
						</Box>
					))}
				</Box>
			)}

			{/* @-mention autocomplete - below input */}
			{mode === 'at-mention' && atCandidates.length > 0 && (
				<Box flexDirection="column" paddingLeft={2}>
					{atCandidates.map((candidate, i) => (
						<Box key={candidate}>
							<Text
								color={i === selectedIndex ? 'cyan' : undefined}
								bold={i === selectedIndex}
							>
								{i === selectedIndex ? '\u276F' : ' '} @{candidate}
							</Text>
						</Box>
					))}
				</Box>
			)}
		</Box>
	);
}

function ShortcutsPanel() {
	const cols = 3;
	const rows: { key: string; desc: string }[][] = [];
	for (let i = 0; i < SHORTCUTS.length; i += cols) {
		rows.push(SHORTCUTS.slice(i, i + cols));
	}

	return (
		<Box flexDirection="column" paddingX={2} paddingY={1}>
			<Text bold> Keyboard shortcuts</Text>
			<Text> </Text>
			{rows.map((row, ri) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: static layout
				<Box key={ri}>
					{row.map((s) => (
						<Box key={s.key} width={26}>
							<Text color="cyan">{s.key}</Text>
							<Text dimColor> {s.desc}</Text>
						</Box>
					))}
				</Box>
			))}
		</Box>
	);
}
