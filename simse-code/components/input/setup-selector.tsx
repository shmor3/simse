import { Box, Text, useInput } from 'ink';
import { useState } from 'react';
import { TextInput } from './text-input.js';

export interface SetupPresetOption {
	readonly key: string;
	readonly label: string;
	readonly description: string;
	readonly needsInput: boolean;
}

interface SetupSelectorProps {
	readonly presets: readonly SetupPresetOption[];
	readonly onSelect: (selection: {
		presetKey: string;
		customArgs: string;
	}) => void;
	readonly onDismiss: () => void;
}

type SelectorMode = 'selecting' | 'custom-input';

export function SetupSelector({
	presets,
	onSelect,
	onDismiss,
}: SetupSelectorProps) {
	const [mode, setMode] = useState<SelectorMode>('selecting');
	const [selectedIndex, setSelectedIndex] = useState(0);
	const [customValue, setCustomValue] = useState('');

	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setSelectedIndex((prev) =>
					prev < presets.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				const preset = presets[selectedIndex];
				if (!preset) return;
				if (preset.needsInput) {
					setMode('custom-input');
				} else {
					onSelect({ presetKey: preset.key, customArgs: '' });
				}
			}
		},
		{ isActive: mode === 'selecting' },
	);

	useInput(
		(_input, key) => {
			if (key.escape) {
				setMode('selecting');
				setCustomValue('');
			}
		},
		{ isActive: mode === 'custom-input' },
	);

	if (mode === 'custom-input') {
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Enter custom ACP server command:</Text>
				<Text> </Text>
				<Box paddingLeft={2}>
					<Text dimColor>{'> '}</Text>
					<TextInput
						value={customValue}
						onChange={setCustomValue}
						onSubmit={(val) => {
							const trimmed = val.trim();
							if (trimmed) {
								onSelect({ presetKey: 'custom', customArgs: trimmed });
							}
						}}
						placeholder="my-server --port 8080"
					/>
				</Box>
				<Text> </Text>
				<Text dimColor>{'  ↵ confirm  esc back'}</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold>Configure ACP Server</Text>
			<Text> </Text>
			{presets.map((preset, i) => {
				const isSelected = i === selectedIndex;
				return (
					<Box key={preset.key}>
						<Text color={isSelected ? 'cyan' : undefined}>
							{isSelected ? '  ❯ ' : '    '}
						</Text>
						<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
							{preset.label.padEnd(20)}
						</Text>
						<Text dimColor>{preset.description}</Text>
					</Box>
				);
			})}
			<Text> </Text>
			<Text dimColor>{'  ↑↓ navigate  ↵ select  esc cancel'}</Text>
		</Box>
	);
}
