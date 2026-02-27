import { Box, Text } from 'ink';
import React, { useState } from 'react';
import { Badge } from '../shared/badge.js';
import { TextInput } from './text-input.js';

interface PromptInputProps {
	readonly onSubmit: (value: string) => void;
	readonly disabled?: boolean;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
}

export function PromptInput({
	onSubmit,
	disabled = false,
	planMode,
	verbose,
}: PromptInputProps) {
	const [value, setValue] = useState('');

	const handleSubmit = (input: string) => {
		if (!input.trim()) return;
		onSubmit(input);
		setValue('');
	};

	return (
		<Box gap={1}>
			{planMode && <Badge label="PLAN" />}
			{verbose && <Badge label="VERBOSE" />}
			<Text bold color="cyan">
				{'❯'}
			</Text>
			<TextInput
				value={value}
				onChange={setValue}
				onSubmit={handleSubmit}
				isActive={!disabled}
				placeholder="Send a message..."
			/>
		</Box>
	);
}
