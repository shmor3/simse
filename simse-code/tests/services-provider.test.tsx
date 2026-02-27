import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { ServicesProvider, useServices } from '../providers/services-provider.js';

function TestConsumer() {
	const { dataDir } = useServices();
	return <Text>{dataDir}</Text>;
}

describe('ServicesProvider', () => {
	test('provides services to children', () => {
		const services = {
			app: {} as any,
			acpClient: {} as any,
			vfs: {} as any,
			disk: {} as any,
			toolRegistry: {} as any,
			skillRegistry: {} as any,
			configResult: {} as any,
			dataDir: '/test/data',
		};

		const { lastFrame } = render(
			<ServicesProvider value={services}>
				<TestConsumer />
			</ServicesProvider>,
		);
		expect(lastFrame()).toContain('/test/data');
	});

	test('useServices throws when called outside React', () => {
		expect(() => useServices()).toThrow();
	});
});
