import { Box } from 'ink';
import React from 'react';
import type { ReactNode } from 'react';

interface MainLayoutProps {
	readonly children: ReactNode;
}

export function MainLayout({ children }: MainLayoutProps) {
	return (
		<Box flexDirection="column" flexGrow={1}>
			{children}
		</Box>
	);
}
