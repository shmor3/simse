import TextInput from 'ink-text-input';
import { Box, Text } from 'ink';
import React, { useState } from 'react';
import { Badge } from '../shared/badge.js';

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
				{'>'}
			</Text>
			{!disabled && (
				<TextInput
					value={value}
					onChange={setValue}
					onSubmit={handleSubmit}
				/>
			)}
		</Box>
	);
}
