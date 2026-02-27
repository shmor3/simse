import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { SessionProvider, useSession } from '../providers/session-provider.js';

function TestConsumer() {
	const { serverName, permissionMode } = useSession();
	return <Text>{serverName ?? 'none'} {permissionMode}</Text>;
}

describe('SessionProvider', () => {
	test('provides session state to children', () => {
		const { lastFrame } = render(
			<SessionProvider initialServerName="test-server">
				<TestConsumer />
			</SessionProvider>,
		);
		expect(lastFrame()).toContain('test-server');
		expect(lastFrame()).toContain('default');
	});

	test('useSession throws when called outside React', () => {
		expect(() => useSession()).toThrow();
	});
});
