import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { ThemeProvider, useTheme } from '../providers/theme-provider.js';

function TestConsumer() {
	const { colors } = useTheme();
	return <Text>{colors.enabled ? 'colors-on' : 'colors-off'}</Text>;
}

describe('ThemeProvider', () => {
	test('provides colors to children', () => {
		const { lastFrame } = render(
			<ThemeProvider>
				<TestConsumer />
			</ThemeProvider>,
		);
		// In test environment, colors are disabled (no TTY)
		expect(lastFrame()).toContain('colors-off');
	});

	test('useTheme throws when called outside React', () => {
		expect(() => useTheme()).toThrow();
	});
});
