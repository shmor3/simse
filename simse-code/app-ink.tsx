import { execFileSync } from 'node:child_process';
import chalk from 'chalk';
import { Box, Text, useInput } from 'ink';
import {
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
import { OnboardingWizard } from './components/input/onboarding-wizard.js';
import { PermissionDialog } from './components/input/permission-dialog.js';
import {
	PromptInput,
	type PromptMode,
} from './components/input/prompt-input.js';
import type { SetupPresetOption } from './components/input/setup-selector.js';
import { SetupSelector } from './components/input/setup-selector.js';
import { Banner } from './components/layout/banner.js';
import { MainLayout } from './components/layout/main-layout.js';
import { StatusBar } from './components/layout/status-bar.js';
import { ThinkingSpinner } from './components/shared/spinner.js';
import { createCLIConfig } from './config.js';
import type { Conversation } from './conversation.js';
import { createConversation } from './conversation.js';
import { aiCommands } from './features/ai/index.js';
import { configCommands } from './features/config/index.js';
import { createInitCommands } from './features/config/init.js';
import { createSetupCommands } from './features/config/setup.js';
import { filesCommands } from './features/files/index.js';
import { libraryCommands } from './features/library/index.js';
import type { MetaCommandContext } from './features/meta/index.js';
import { createMetaCommands } from './features/meta/index.js';
import type { SessionCommandContext } from './features/session/index.js';
import { createSessionCommands } from './features/session/index.js';
import { createToolsCommands } from './features/tools/index.js';
import {
	completeAtMention,
	formatMentionsAsContext,
	resolveFileMentions,
} from './file-mentions.js';
import { useAgenticLoop } from './hooks/use-agentic-loop.js';
import { useCommandDispatch } from './hooks/use-command-dispatch.js';
import { detectImages, formatImageIndicator } from './image-input.js';
import type { OutputItem } from './ink-types.js';
import type { PermissionManager } from './permission-manager.js';
import type { SessionStore } from './session-store.js';
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
	readonly sessionStore: SessionStore;
	readonly sessionId: string;
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
	sessionStore,
	sessionId: initialSessionId,
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
	const sessionIdRef = useRef(initialSessionId);
	const [promptMode, setPromptMode] = useState<PromptMode>('normal');
	const [ctrlCWarning, setCtrlCWarning] = useState(false);
	const [showOnboarding, setShowOnboarding] = useState(!initialHasACP);
	const [permissionModeLabel, setPermissionModeLabel] = useState(
		permissionManager.formatMode(),
	);

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

	// Interactive setup selector — Promise-based active-area dialog
	const [pendingSetup, setPendingSetup] = useState<{
		presets: SetupPresetOption[];
		resolve: (result: { presetKey: string; customArgs: string } | null) => void;
	} | null>(null);

	const handleShowSetupSelector = useCallback(
		(
			presets: SetupPresetOption[],
		): Promise<{ presetKey: string; customArgs: string } | null> => {
			return new Promise((resolve) => {
				setPendingSetup({ presets, resolve });
			});
		},
		[],
	);

	// Refs for meta command state access (commands are created once but execute later)
	const verboseRef = useRef(verbose);
	verboseRef.current = verbose;
	const planModeRef = useRef(planMode);
	planModeRef.current = planMode;
	const bannerRef = useRef(bannerElement);

	// Resume a session: load messages into conversation and rebuild items
	const resumeSession = useCallback(
		(id: string) => {
			const messages = sessionStore.load(id);
			if (messages.length === 0) return;

			conversationRef.current.loadMessages(messages);
			sessionIdRef.current = id;

			// Rebuild items from saved messages
			const restored: OutputItem[] = [
				{ kind: 'command-result', element: bannerRef.current },
			];
			for (const msg of messages) {
				if (msg.role === 'user') {
					restored.push({ kind: 'message', role: 'user', text: msg.content });
				} else if (msg.role === 'assistant') {
					restored.push({
						kind: 'message',
						role: 'assistant',
						text: msg.content,
					});
				}
			}
			setItems(restored);
		},
		[sessionStore],
	);

	const registry = useMemo(() => {
		const reg = createCommandRegistry();
		const metaCtx: MetaCommandContext = {
			getCommands: () => reg.getAll(),
			setVerbose: (on) => setVerbose(on),
			getVerbose: () => verboseRef.current,
			setPlanMode: (on) => setPlanMode(on),
			getPlanMode: () => planModeRef.current,
			clearConversation: () => {
				conversationRef.current.clear();
				setItems([{ kind: 'command-result', element: bannerRef.current }]);
			},
			getContextUsage: () => ({
				usedChars: conversationRef.current.estimatedChars,
				maxChars: 200_000,
			}),
		};
		const sessionCtx: SessionCommandContext = {
			sessionStore,
			getSessionId: () => sessionIdRef.current,
			getServerName: () => currentServerName,
			getModelName: () => currentModelName,
			resumeSession,
		};
		const meta = createMetaCommands(metaCtx);
		reg.registerAll(meta);
		reg.registerAll(libraryCommands);
		reg.registerAll(
			createToolsCommands({ getToolRegistry: () => toolRegistryRef.current }),
		);
		reg.registerAll(createSessionCommands(sessionCtx));
		reg.registerAll(filesCommands);
		reg.registerAll(configCommands);
		reg.registerAll(
			createSetupCommands(
				dataDir,
				handleSetupComplete,
				handleShowSetupSelector,
			),
		);
		reg.registerAll(
			createInitCommands({
				getAcpClient: () => acpClientRef.current,
				getServerName: () => currentServerName,
				hasACP: () => hasACP,
			}),
		);
		reg.registerAll(aiCommands);
		return reg;
	}, [
		dataDir,
		handleSetupComplete,
		handleShowSetupSelector,
		sessionStore,
		currentServerName,
		currentModelName,
		resumeSession,
		hasACP,
	]);

	const { dispatch, isCommand } = useCommandDispatch(registry);

	// biome-ignore lint/correctness/useExhaustiveDependencies: hasACP triggers ref swap — intentional
	const loopOptions = useMemo(
		() => ({
			acpClient: acpClientRef.current,
			conversation: conversationRef.current,
			toolRegistry: toolRegistryRef.current,
			serverName: currentServerName,
			permissionManager,
		}),
		[hasACP, currentServerName, permissionManager],
	);

	const {
		state: loopState,
		submit: submitToLoop,
		abort: abortLoop,
		pendingPermission,
		resolvePermission,
		tokenUsage,
	} = useAgenticLoop(loopOptions);

	// Escape key interrupts the agentic loop when processing
	// (but not when setup selector is active — it handles its own Escape)
	useInput(
		(_input, key) => {
			if (key.escape && !pendingSetup) {
				abortLoop();
				setIsProcessing(false);
				setItems((prev) => [...prev, { kind: 'info', text: 'Interrupted.' }]);
			}
		},
		{ isActive: isProcessing },
	);

	// Ctrl+L clears the screen (like /clear)
	useInput(
		(input, key) => {
			if (key.ctrl && input === 'l') {
				conversationRef.current.clear();
				setItems([{ kind: 'command-result', element: bannerRef.current }]);
			}
			// Shift+Tab cycles permission mode
			if (key.shift && key.tab) {
				permissionManager.cycleMode();
				const label = permissionManager.formatMode();
				setPermissionModeLabel(label);
				setItems((prev) => [
					...prev,
					{ kind: 'info', text: `Permission mode: ${label}` },
				]);
			}
		},
		{ isActive: !isProcessing },
	);

	const handleCompleteAtMention = useCallback(
		(partial: string): readonly string[] => completeAtMention(partial),
		[],
	);

	// Auto-save message to session (crash-safe, sync write)
	const saveMessage = useCallback(
		(role: 'user' | 'assistant', content: string) => {
			try {
				sessionStore.append(sessionIdRef.current, {
					role,
					content,
				});
			} catch {
				// Best-effort: don't break the UI if save fails
			}
		},
		[sessionStore],
	);

	const handleSubmit = useCallback(
		async (input: string) => {
			setIsProcessing(true);

			// Save user message to session immediately (crash-safe)
			saveMessage('user', input);

			setItems((prev) => [
				...prev,
				{ kind: 'message', role: 'user', text: input },
			]);

			if (input.startsWith('!') && input.length > 1) {
				// Bash mode: execute shell command directly
				const cmd = input.slice(1).trim();
				try {
					const shell = process.platform === 'win32' ? 'cmd.exe' : '/bin/sh';
					const shellArgs =
						process.platform === 'win32' ? ['/c', cmd] : ['-c', cmd];
					const output = execFileSync(shell, shellArgs, {
						encoding: 'utf-8',
						timeout: 30_000,
						stdio: ['pipe', 'pipe', 'pipe'],
					});
					setItems((prev) => [
						...prev,
						{ kind: 'info', text: output.trimEnd() || '(no output)' },
					]);
				} catch (err) {
					const e = err as { stderr?: string; message?: string };
					setItems((prev) => [
						...prev,
						{
							kind: 'error',
							message: e.stderr?.trimEnd() || e.message || 'Command failed',
						},
					]);
				}
			} else if (isCommand(input)) {
				const result = await dispatch(input);
				if (result?.text) {
					const text = result.text;
					setItems((prev) => [...prev, { kind: 'info', text }]);
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

				// Save assistant messages to session (crash-safe)
				for (const item of completedItems) {
					if (item.kind === 'message' && item.role === 'assistant') {
						saveMessage('assistant', item.text);
					}
				}

				setItems((prev) => [...prev, ...completedItems]);
			}

			setIsProcessing(false);
		},
		[dispatch, isCommand, submitToLoop, hasACP, saveMessage],
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

			{pendingSetup && (
				<SetupSelector
					presets={pendingSetup.presets}
					onSelect={(selection) => {
						pendingSetup.resolve(selection);
						setPendingSetup(null);
					}}
					onDismiss={() => {
						pendingSetup.resolve(null);
						setPendingSetup(null);
					}}
				/>
			)}

			{pendingPermission && (
				<PermissionDialog
					toolName={pendingPermission.call.name}
					args={pendingPermission.call.arguments as Record<string, unknown>}
					onAllow={() => resolvePermission('allow')}
					onDeny={() => resolvePermission('deny')}
					onAllowAlways={() => resolvePermission('allow', true)}
				/>
			)}

			{showOnboarding ? (
				<OnboardingWizard
					dataDir={dataDir}
					onComplete={async (filesCreated) => {
						setShowOnboarding(false);
						await handleSetupComplete();
						setItems((prev) => [
							...prev,
							{
								kind: 'info',
								text: `Setup complete! Files written: ${filesCreated.join(', ')}`,
							},
						]);
					}}
					onDismiss={() => {
						setShowOnboarding(false);
						setItems((prev) => [
							...prev,
							{
								kind: 'info',
								text: 'Setup skipped. Run /setup to configure later.',
							},
						]);
					}}
				/>
			) : (
				<>
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
							permissionMode={permissionModeLabel}
							totalTokens={tokenUsage.totalTokens}
							contextPercent={Math.round(
								(conversationRef.current.estimatedChars / 200_000) * 100,
							)}
						/>
					) : null}
				</>
			)}
		</MainLayout>
	);
}
