import { Text } from 'ink';
import React from 'react';

interface StreamingTextProps {
	readonly text: string;
}

export function StreamingText({ text }: StreamingTextProps) {
	if (!text) return null;
	return <Text>{text}</Text>;
}
