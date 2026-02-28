import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { render } from 'ink-testing-library';
import { MainLayout } from '../components/layout/main-layout.js';

describe('MainLayout', () => {
	test('renders children', () => {
		const { lastFrame } = render(
			<MainLayout>
				<Text>Hello</Text>
			</MainLayout>,
		);
		expect(lastFrame()).toContain('Hello');
	});
});
