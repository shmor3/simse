import { Box, Text } from 'ink';
import InkSpinner from 'ink-spinner';
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { createACPClient, toError } from 'simse';
import type { ACPClient, ACPPermissionRequestInfo } from 'simse';
import { createCommandRegistry } from './command-registry.js';
import { Banner } from './components/layout/banner.js';
import { MainLayout } from './components/layout/main-layout.js';
import { StatusBar } from './components/layout/status-bar.js';
import { MessageList } from './components/chat/message-list.js';
import { ToolCallBox } from './components/chat/tool-call-box.js';
import { PromptInput } from './components/input/prompt-input.js';
import { useCommandDispatch } from './hooks/use-command-dispatch.js';
import { useAgenticLoop } from './hooks/use-agentic-loop.js';
import { createMetaCommands } from './features/meta/index.js';
import { libraryCommands } from './features/library/index.js';
import { toolsCommands } from './features/tools/index.js';
import { sessionCommands } from './features/session/index.js';
import { filesCommands } from './features/files/index.js';
import { configCommands } from './features/config/index.js';
import { createSetupCommands } from './features/config/setup.js';
import { aiCommands } from './features/ai/index.js';
import { createCLIConfig } from './config.js';
import { createConversation } from './conversation.js';
import type { Conversation } from './conversation.js';
import { createToolRegistry } from './tool-registry.js';
import type { ToolRegistry } from './tool-registry.js';
import type { OutputItem } from './ink-types.js';

interface AppProps {
	readonly dataDir: string;
	readonly serverName?: string;
	readonly modelName?: string;
	readonly acpClient: ACPClient;
	readonly conversation: Conversation;
	readonly toolRegistry: ToolRegistry;
	readonly hasACP?: boolean;
}

export function App({
	dataDir,
	serverName: initialServerName,
	modelName: initialModelName,
	acpClient: initialAcpClient,
	conversation: initialConversation,
	toolRegistry: initialToolRegistry,
	hasACP: initialHasACP = true,
}: AppProps) {
	const [items, setItems] = useState<OutputItem[]>([]);
	const [isProcessing, setIsProcessing] = useState(false);
	const [planMode, setPlanMode] = useState(false);
	const [verbose, setVerbose] = useState(false);

	// Mutable service refs — swapped on /setup reload
	const [hasACP, setHasACP] = useState(initialHasACP);
	const [currentServerName, setCurrentServerName] = useState(initialServerName);
	const [currentModelName, setCurrentModelName] = useState(initialModelName);
	const acpClientRef = useRef(initialAcpClient);
	const conversationRef = useRef(initialConversation);
	const toolRegistryRef = useRef(initialToolRegistry);

	// Re-bootstrap services after /setup writes new config files
	const handleSetupComplete = useCallback(async () => {
		try {
			const configResult = createCLIConfig({ dataDir });
			const { config, logger } = configResult;

			if (config.acp.servers.length === 0) return;

			const newServerName =
				config.acp.defaultServer ?? config.acp.servers[0]?.name;

			const newClient = createACPClient(config.acp, {
				logger,
				onPermissionRequest: async (
					info: ACPPermissionRequestInfo,
				): Promise<string | undefined> => {
					const allowOption = info.options.find(
						(o) => o.kind === 'allow_once',
					);
					return allowOption?.optionId;
				},
			});

			await newClient.initialize();

			// Get model info
			let newModelName: string | undefined;
			if (newServerName) {
				try {
					const modelInfo =
						await newClient.getServerModelInfo(newServerName);
					newModelName = modelInfo?.currentModelId;
				} catch {
					// optional
				}
			}

			const newConversation = createConversation();
			if (configResult.workspacePrompt) {
				newConversation.setSystemPrompt(configResult.workspacePrompt);
			}

			const newToolRegistry = createToolRegistry({ logger });
			await newToolRegistry.discover();

			// Swap refs and update state
			acpClientRef.current = newClient;
			conversationRef.current = newConversation;
			toolRegistryRef.current = newToolRegistry;
			setHasACP(true);
			setCurrentServerName(newServerName);
			setCurrentModelName(newModelName);
		} catch (err) {
			setItems((prev) => [
				...prev,
				{
					kind: 'error',
					message: `Failed to connect: ${toError(err).message}. Check your config and restart.`,
				},
			]);
		}
	}, [dataDir]);

	const registry = useMemo(() => {
		const reg = createCommandRegistry();
		const meta = createMetaCommands(() => reg.getAll());
		reg.registerAll(meta);
		reg.registerAll(libraryCommands);
		reg.registerAll(toolsCommands);
		reg.registerAll(sessionCommands);
		reg.registerAll(filesCommands);
		reg.registerAll(configCommands);
		reg.registerAll(createSetupCommands(dataDir, handleSetupComplete));
		reg.registerAll(aiCommands);
		return reg;
	}, [dataDir, handleSetupComplete]);

	const { dispatch, isCommand } = useCommandDispatch(registry);

	const loopOptions = useMemo(
		() => ({
			acpClient: acpClientRef.current,
			conversation: conversationRef.current,
			toolRegistry: toolRegistryRef.current,
			serverName: currentServerName,
		}),
		// Re-create when hasACP changes (services were swapped)
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[hasACP, currentServerName],
	);

	const { state: loopState, submit: submitToLoop } =
		useAgenticLoop(loopOptions);

	const handleSubmit = useCallback(
		async (input: string) => {
			setIsProcessing(true);

			setItems((prev) => [
				...prev,
				{ kind: 'message', role: 'user', text: input },
			]);

			if (isCommand(input)) {
				const result = await dispatch(input);
				if (result?.text) {
					setItems((prev) => [
						...prev,
						{ kind: 'info', text: result.text! },
					]);
				} else if (result?.element) {
					setItems((prev) => [
						...prev,
						{ kind: 'command-result', element: result.element },
					]);
				}
			} else if (!hasACP) {
				setItems((prev) => [
					...prev,
					{
						kind: 'error',
						message:
							'No ACP server configured. Run /setup to configure one.\n' +
							'  Examples: /setup claude-code, /setup ollama, /setup copilot',
					},
				]);
			} else {
				// Send to agentic loop
				const completedItems = await submitToLoop(input);
				setItems((prev) => [...prev, ...completedItems]);
			}

			setIsProcessing(false);
		},
		[dispatch, isCommand, submitToLoop, hasACP],
	);

	return (
		<MainLayout>
			<Banner
				version="1.0.0"
				workDir={process.cwd()}
				dataDir={dataDir}
				server={currentServerName}
				model={currentModelName}
			/>
			<MessageList items={items} />

			{/* Active area: streaming text and active tool calls */}
			{loopState.status !== 'idle' && (
				<Box flexDirection="column" paddingLeft={2}>
					{loopState.streamText && (
						<Box>
							<Text>
								<Text color="magenta">{'● '}</Text>
								{loopState.streamText}
							</Text>
						</Box>
					)}
					{loopState.activeToolCalls.map((tc) => (
						<ToolCallBox
							key={tc.id}
							name={tc.name}
							args={tc.args}
							status="active"
						/>
					))}
					{loopState.status === 'streaming' && !loopState.streamText && (
						<Box gap={1}>
							<Text color="cyan">
								<InkSpinner type="dots" />
							</Text>
							<Text dimColor>Thinking...</Text>
						</Box>
					)}
				</Box>
			)}

			<Box flexDirection="column">
				<PromptInput
					onSubmit={handleSubmit}
					disabled={isProcessing}
					planMode={planMode}
					verbose={verbose}
				/>
			</Box>
			<StatusBar
				server={currentServerName}
				model={currentModelName}
				planMode={planMode}
				verbose={verbose}
			/>
		</MainLayout>
	);
}
