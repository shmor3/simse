import { Box, Text } from 'ink';
import InkSpinner from 'ink-spinner';
import React, { useCallback, useMemo, useState } from 'react';
import type { ACPClient } from 'simse';
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
import { aiCommands } from './features/ai/index.js';
import type { Conversation } from './conversation.js';
import type { ToolRegistry } from './tool-registry.js';
import type { OutputItem } from './ink-types.js';

interface AppProps {
	readonly dataDir: string;
	readonly serverName?: string;
	readonly modelName?: string;
	readonly acpClient: ACPClient;
	readonly conversation: Conversation;
	readonly toolRegistry: ToolRegistry;
}

export function App({
	dataDir,
	serverName,
	modelName,
	acpClient,
	conversation,
	toolRegistry,
}: AppProps) {
	const [items, setItems] = useState<OutputItem[]>([]);
	const [isProcessing, setIsProcessing] = useState(false);
	const [planMode, setPlanMode] = useState(false);
	const [verbose, setVerbose] = useState(false);

	const registry = useMemo(() => {
		const reg = createCommandRegistry();
		const meta = createMetaCommands(() => reg.getAll());
		reg.registerAll(meta);
		reg.registerAll(libraryCommands);
		reg.registerAll(toolsCommands);
		reg.registerAll(sessionCommands);
		reg.registerAll(filesCommands);
		reg.registerAll(configCommands);
		reg.registerAll(aiCommands);
		return reg;
	}, []);

	const { dispatch, isCommand } = useCommandDispatch(registry);

	const loopOptions = useMemo(
		() => ({
			acpClient,
			conversation,
			toolRegistry,
			serverName,
		}),
		[acpClient, conversation, toolRegistry, serverName],
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
			} else {
				// Send to agentic loop
				const completedItems = await submitToLoop(input);
				setItems((prev) => [...prev, ...completedItems]);
			}

			setIsProcessing(false);
		},
		[dispatch, isCommand, submitToLoop],
	);

	return (
		<MainLayout>
			<Banner
				version="1.0.0"
				workDir={process.cwd()}
				dataDir={dataDir}
				server={serverName}
				model={modelName}
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
				server={serverName}
				model={modelName}
				planMode={planMode}
				verbose={verbose}
			/>
		</MainLayout>
	);
}
