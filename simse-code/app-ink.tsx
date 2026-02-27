import chalk from 'chalk';
import { Box, Text, useInput } from 'ink';
import React, {
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from 'react';
import type { ACPClient, ACPPermissionRequestInfo } from 'simse';
import { createACPClient, toError } from 'simse';
import { createCommandRegistry } from './command-registry.js';
import { Markdown } from './components/chat/markdown.js';
import { MessageList } from './components/chat/message-list.js';
import { ToolCallBox } from './components/chat/tool-call-box.js';
import {
	PromptInput,
	type PromptMode,
} from './components/input/prompt-input.js';
import { Banner } from './components/layout/banner.js';
import { MainLayout } from './components/layout/main-layout.js';
import { StatusBar } from './components/layout/status-bar.js';
import { PermissionDialog } from './components/input/permission-dialog.js';
import { ThinkingSpinner } from './components/shared/spinner.js';
import { createCLIConfig } from './config.js';
import type { Conversation } from './conversation.js';
import { createConversation } from './conversation.js';
import { aiCommands } from './features/ai/index.js';
import { configCommands } from './features/config/index.js';
import { createSetupCommands } from './features/config/setup.js';
import { filesCommands } from './features/files/index.js';
import { libraryCommands } from './features/library/index.js';
import { createMetaCommands } from './features/meta/index.js';
import { sessionCommands } from './features/session/index.js';
import { toolsCommands } from './features/tools/index.js';
import { useAgenticLoop } from './hooks/use-agentic-loop.js';
import { useCommandDispatch } from './hooks/use-command-dispatch.js';
import {
	completeAtMention,
	formatMentionsAsContext,
	resolveFileMentions,
} from './file-mentions.js';
import { detectImages, formatImageIndicator } from './image-input.js';
import type { OutputItem } from './ink-types.js';
import type { PermissionManager } from './permission-manager.js';
import type { ToolRegistry } from './tool-registry.js';
import { createToolRegistry } from './tool-registry.js';

interface AppProps {
	readonly dataDir: string;
	readonly serverName?: string;
	readonly modelName?: string;
	readonly acpClient: ACPClient;
	readonly conversation: Conversation;
	readonly toolRegistry: ToolRegistry;
	readonly permissionManager: PermissionManager;
	readonly hasACP?: boolean;
}

export function App({
	dataDir,
	serverName: initialServerName,
	modelName: initialModelName,
	acpClient: initialAcpClient,
	conversation: initialConversation,
	toolRegistry: initialToolRegistry,
	permissionManager,
	hasACP: initialHasACP = true,
}: AppProps) {
	const bannerElement: ReactNode = (
		<Banner
			version="1.0.0"
			workDir={process.cwd()}
			dataDir={dataDir}
			server={initialServerName}
			model={initialModelName}
		/>
	);
	const [items, setItems] = useState<OutputItem[]>([
		{ kind: 'command-result', element: bannerElement },
	]);
	const [isProcessing, setIsProcessing] = useState(false);
	const [planMode, setPlanMode] = useState(false);
	const [verbose, setVerbose] = useState(false);
	const [promptMode, setPromptMode] = useState<PromptMode>('normal');
	const [ctrlCWarning, setCtrlCWarning] = useState(false);

	// Ctrl+C double-press guard: first press shows warning, second exits
	const ctrlCRef = useRef(false);
	useEffect(() => {
		const handler = () => {
			if (ctrlCRef.current) {
				process.exit(0);
			}
			ctrlCRef.current = true;
			setCtrlCWarning(true);
			const timer = setTimeout(() => {
				ctrlCRef.current = false;
				setCtrlCWarning(false);
			}, 2000);
			timer.unref?.();
		};
		process.on('SIGINT', handler);
		return () => {
			process.off('SIGINT', handler);
		};
	}, []);

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
					const pick =
						info.options.find((o) => o.kind === 'allow_always') ??
						info.options.find((o) => o.kind === 'allow_once') ??
						info.options[0];
					return pick?.optionId;
				},
			});

			newClient.setPermissionPolicy('auto-approve');
			await newClient.initialize();

			// Get model info
			let newModelName: string | undefined;
			if (newServerName) {
				try {
					const modelInfo = await newClient.getServerModelInfo(newServerName);
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
			permissionManager,
		}),
		// Re-create when hasACP changes (services were swapped)
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[hasACP, currentServerName, permissionManager],
	);

	const {
		state: loopState,
		submit: submitToLoop,
		abort: abortLoop,
		pendingPermission,
		resolvePermission,
	} = useAgenticLoop(loopOptions);

	// Escape key interrupts the agentic loop when processing
	useInput(
		(_input, key) => {
			if (key.escape) {
				abortLoop();
				setIsProcessing(false);
				setItems((prev) => [...prev, { kind: 'info', text: 'Interrupted.' }]);
			}
		},
		{ isActive: isProcessing },
	);

	const handleCompleteAtMention = useCallback(
		(partial: string): readonly string[] => completeAtMention(partial),
		[],
	);

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
					setItems((prev) => [...prev, { kind: 'info', text: result.text! }]);
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
				// Detect images in user input
				const { cleanInput: imageClean, images } = detectImages(input);

				// Show image indicators
				if (images.length > 0) {
					const indicators = images.map((img) =>
						formatImageIndicator(img, {
							dim: chalk.dim,
							cyan: chalk.cyan,
						}),
					);
					setItems((prev) => [
						...prev,
						{ kind: 'info', text: indicators.join('\n') },
					]);
				}

				// Resolve @-mentions
				const afterImages = images.length > 0 ? imageClean : input;
				const mentionResult = resolveFileMentions(afterImages);
				let processedInput = mentionResult.cleanInput || afterImages;

				if (mentionResult.mentions.length > 0) {
					const mentionContext = formatMentionsAsContext(
						mentionResult.mentions,
					);
					processedInput = `${mentionContext}\n\n${processedInput}`;
					setItems((prev) => [
						...prev,
						...mentionResult.mentions.map((m) => ({
							kind: 'info' as const,
							text: `  @${m.path} (${m.size} bytes)`,
						})),
					]);
				}

				// Send to agentic loop
				const completedItems = await submitToLoop(
					processedInput,
					images.length > 0 ? images : undefined,
				);
				setItems((prev) => [...prev, ...completedItems]);
			}

			setIsProcessing(false);
		},
		[dispatch, isCommand, submitToLoop, hasACP],
	);

	return (
		<MainLayout>
			<MessageList items={items} />

			{/* Active area: streaming text and active tool calls */}
			{loopState.status !== 'idle' && (
				<Box flexDirection="column">
					{loopState.streamText && (
						<Box paddingLeft={2}>
							<Markdown text={loopState.streamText} />
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
						<ThinkingSpinner />
					)}
				</Box>
			)}

			{pendingPermission && (
				<PermissionDialog
					toolName={pendingPermission.call.name}
					args={
						pendingPermission.call.arguments as Record<string, unknown>
					}
					onAllow={() => resolvePermission('allow')}
					onDeny={() => resolvePermission('deny')}
					onAllowAlways={() => resolvePermission('allow', true)}
				/>
			)}

			<Box flexDirection="column">
				<PromptInput
					onSubmit={handleSubmit}
					disabled={isProcessing}
					planMode={planMode}
					commands={registry.getAll()}
					onModeChange={setPromptMode}
					onCompleteAtMention={handleCompleteAtMention}
				/>
			</Box>
			{ctrlCWarning ? (
				<Box paddingX={1}>
					<Text dimColor>Press Ctrl-C again to exit</Text>
				</Box>
			) : promptMode === 'normal' ? (
				<StatusBar
					isProcessing={isProcessing}
					planMode={planMode}
					verbose={verbose}
				/>
			) : null}
		</MainLayout>
	);
}
