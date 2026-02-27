import { Box, Text, useInput } from 'ink';
import React, { useMemo, useState } from 'react';
import type { CommandDefinition } from '../../ink-types.js';
import { TextInput } from './text-input.js';

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

interface PromptInputProps {
	readonly onSubmit: (value: string) => void;
	readonly disabled?: boolean;
	readonly planMode?: boolean;
	readonly commands?: readonly CommandDefinition[];
}

export function PromptInput({
	onSubmit,
	disabled = false,
	planMode,
	commands = [],
}: PromptInputProps) {
	const [value, setValue] = useState('');
	const [mode, setMode] = useState<'normal' | 'shortcuts' | 'autocomplete'>(
		'normal',
	);
	const [selectedIndex, setSelectedIndex] = useState(0);

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

	const handleChange = (newValue: string) => {
		// Intercept `?` on empty input to show shortcuts
		if (newValue === '?' && value === '') {
			setMode('shortcuts');
			return;
		}

		setValue(newValue);

		if (newValue.startsWith('/') && newValue.length >= 1) {
			if (mode !== 'autocomplete') setMode('autocomplete');
			setSelectedIndex(0);
		} else if (mode === 'autocomplete') {
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
					placeholder="Send a message..."
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
