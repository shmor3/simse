import { Box, Text, useInput } from 'ink';
import { useState } from 'react';
import { TextInput } from './text-input.js';

interface ConfirmDialogProps {
	readonly message: string;
	readonly onConfirm: () => void;
	readonly onCancel: () => void;
}

export function ConfirmDialog({
	message,
	onConfirm,
	onCancel,
}: ConfirmDialogProps) {
	const [selectedIndex, setSelectedIndex] = useState(0);
	const [confirmValue, setConfirmValue] = useState('');

	// Navigation when "No" is focused
	useInput(
		(_input, key) => {
			if (key.escape) {
				onCancel();
				return;
			}
			if (key.downArrow) {
				setSelectedIndex(1);
				return;
			}
			if (key.return) {
				onCancel();
			}
		},
		{ isActive: selectedIndex === 0 },
	);

	// Navigation when "Yes" is focused (escape + arrow up to go back)
	useInput(
		(_input, key) => {
			if (key.escape) {
				onCancel();
				return;
			}
			if (key.upArrow) {
				setSelectedIndex(0);
				setConfirmValue('');
			}
		},
		{ isActive: selectedIndex === 1 },
	);

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Box>
				<Text color="red">{'⚠  '}</Text>
				<Text bold>{message}</Text>
			</Box>
			<Text> </Text>

			{/* Option 0: No (default) */}
			<Box>
				<Text color={selectedIndex === 0 ? 'cyan' : undefined}>
					{selectedIndex === 0 ? '  ❯ ' : '    '}
				</Text>
				<Text bold={selectedIndex === 0} color={selectedIndex === 0 ? 'cyan' : undefined}>
					No, cancel
				</Text>
			</Box>

			{/* Option 1: Yes */}
			<Box>
				<Text color={selectedIndex === 1 ? 'red' : undefined}>
					{selectedIndex === 1 ? '  ❯ ' : '    '}
				</Text>
				<Text bold={selectedIndex === 1} color={selectedIndex === 1 ? 'red' : undefined}>
					Yes, delete everything
				</Text>
			</Box>

			{/* Confirmation input - only when Yes is focused */}
			{selectedIndex === 1 && (
				<>
					<Text> </Text>
					<Box paddingLeft={4}>
						<Text dimColor>{'Type "yes" to confirm: '}</Text>
						<TextInput
							value={confirmValue}
							onChange={setConfirmValue}
							onSubmit={(val) => {
								if (val.trim().toLowerCase() === 'yes') {
									onConfirm();
								}
							}}
							placeholder="yes"
						/>
					</Box>
				</>
			)}

			<Text> </Text>
			<Text dimColor>{'  ↑↓ navigate  ↵ select  esc cancel'}</Text>
		</Box>
	);
}
