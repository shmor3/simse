import { Box } from 'ink';
import React, { useCallback, useMemo, useState } from 'react';
import { createCommandRegistry } from './command-registry.js';
import { Banner } from './components/layout/banner.js';
import { MainLayout } from './components/layout/main-layout.js';
import { StatusBar } from './components/layout/status-bar.js';
import { MessageList } from './components/chat/message-list.js';
import { PromptInput } from './components/input/prompt-input.js';
import { useCommandDispatch } from './hooks/use-command-dispatch.js';
import { createMetaCommands } from './features/meta/index.js';
import { libraryCommands } from './features/library/index.js';
import { toolsCommands } from './features/tools/index.js';
import { sessionCommands } from './features/session/index.js';
import { filesCommands } from './features/files/index.js';
import { configCommands } from './features/config/index.js';
import { aiCommands } from './features/ai/index.js';
import type { OutputItem } from './ink-types.js';

interface AppProps {
	readonly dataDir: string;
	readonly serverName?: string;
	readonly modelName?: string;
}

export function App({ dataDir, serverName, modelName }: AppProps) {
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

	const handleSubmit = useCallback(
		async (input: string) => {
			setIsProcessing(true);

			setItems((prev) => [...prev, { kind: 'message', role: 'user', text: input }]);

			if (isCommand(input)) {
				const result = await dispatch(input);
				if (result?.text) {
					setItems((prev) => [...prev, { kind: 'info', text: result.text! }]);
				} else if (result?.element) {
					setItems((prev) => [...prev, { kind: 'command-result', element: result.element }]);
				}
			} else {
				setItems((prev) => [
					...prev,
					{ kind: 'message', role: 'assistant', text: `(Agentic loop not yet wired) You said: ${input}` },
				]);
			}

			setIsProcessing(false);
		},
		[dispatch, isCommand],
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
