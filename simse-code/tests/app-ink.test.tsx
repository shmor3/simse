import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import React from 'react';
import { App } from '../app-ink.js';
import { createConversation } from '../conversation.js';
import type { PermissionManager } from '../permission-manager.js';

// Minimal mock ACPClient for testing
function createMockACPClient() {
	return {
		initialize: async () => {},
		dispose: async () => {},
		listAgents: async () => [],
		getAgent: async () => ({ id: 'test', name: 'test', capabilities: {} }),
		generate: async () => ({ text: '', usage: undefined }),
		chat: async () => ({ text: '', usage: undefined }),
		generateStream: async function* () {},
		embed: async () => ({ embeddings: [] }),
		isAvailable: async () => false,
		setPermissionPolicy: () => {},
		listSessions: async () => [],
		loadSession: async () => ({ id: 'test', messages: [] }),
		deleteSession: async () => {},
		setSessionMode: async () => {},
		setSessionModel: async () => {},
		getSessionModes: async () => undefined,
		getServerHealth: () => undefined,
		getServerModelInfo: async () => undefined,
		getServerStatuses: async () => [],
		serverNames: [],
		serverCount: 0,
		defaultServerName: undefined,
		defaultAgent: undefined,
	} as any;
}

function createMockToolRegistry() {
	return {
		discover: async () => {},
		getToolDefinitions: () => [],
		formatForSystemPrompt: () => '',
		execute: async () => ({
			id: 'test',
			name: 'test',
			output: '',
			isError: false,
		}),
		toolCount: 0,
	} as any;
}

function createMockPermissionManager(): PermissionManager {
	return {
		check: () => 'allow',
		getMode: () => 'default',
		setMode: () => {},
		cycleMode: () => 'default',
		addRule: () => {},
		removeRule: () => {},
		getRules: () => [],
		save: () => {},
		load: () => {},
		formatMode: () => 'Default — Ask for writes & bash',
	};
}

describe('App', () => {
	test('renders without crashing', () => {
		const { lastFrame } = render(
			<App
				dataDir="/test"
				acpClient={createMockACPClient()}
				conversation={createConversation()}
				toolRegistry={createMockToolRegistry()}
				permissionManager={createMockPermissionManager()}
			/>,
		);
		expect(lastFrame()).toBeDefined();
	});

	test('renders the prompt character', () => {
		const { lastFrame } = render(
			<App
				dataDir="/test"
				acpClient={createMockACPClient()}
				conversation={createConversation()}
				toolRegistry={createMockToolRegistry()}
				permissionManager={createMockPermissionManager()}
			/>,
		);
		expect(lastFrame()).toContain('>');
	});

	test('renders the banner', () => {
		const { lastFrame } = render(
			<App
				dataDir="/test"
				serverName="claude"
				acpClient={createMockACPClient()}
				conversation={createConversation()}
				toolRegistry={createMockToolRegistry()}
				permissionManager={createMockPermissionManager()}
			/>,
		);
		expect(lastFrame()).toContain('simse');
	});
});
