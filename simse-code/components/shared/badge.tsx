import { Text } from 'ink';
import React from 'react';

interface BadgeProps {
	readonly label: string;
	readonly color?: string;
}

const BADGE_COLORS: Record<string, string> = {
	PLAN: 'blue',
	VERBOSE: 'yellow',
	YOLO: 'red',
	'AUTO-EDIT': 'green',
};

export function Badge({ label, color }: BadgeProps) {
	const resolvedColor = color ?? BADGE_COLORS[label] ?? 'gray';
	return (
		<Text color={resolvedColor} bold>
			[{label}]
		</Text>
	);
}
