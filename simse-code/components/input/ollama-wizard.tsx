import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useState } from 'react';
import type { OllamaModelInfo } from '../../features/config/ollama-test.js';
import {
	listOllamaModels,
	testOllamaConnection,
} from '../../features/config/ollama-test.js';
import { TextInput } from './text-input.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface OllamaWizardResult {
	readonly url: string;
	readonly model: string;
}

interface OllamaWizardProps {
	readonly onComplete: (result: OllamaWizardResult) => void;
	readonly onDismiss: () => void;
	readonly defaultUrl?: string;
}

type WizardStep =
	| 'url-input'
	| 'testing'
	| 'model-select'
	| 'model-manual'
	| 'test-failed';

// ---------------------------------------------------------------------------
// Spinner frames
// ---------------------------------------------------------------------------

const SPINNER_FRAMES = [
	'\u280B',
	'\u2819',
	'\u2839',
	'\u2838',
	'\u283C',
	'\u2834',
	'\u2826',
	'\u2827',
	'\u2807',
	'\u280F',
];

const DEFAULT_OLLAMA_URL = 'http://127.0.0.1:11434';

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function OllamaWizard({
	onComplete,
	onDismiss,
	defaultUrl = DEFAULT_OLLAMA_URL,
}: OllamaWizardProps) {
	const [step, setStep] = useState<WizardStep>('url-input');
	const [url, setUrl] = useState(defaultUrl);
	const [models, setModels] = useState<readonly OllamaModelInfo[]>([]);
	const [selectedIndex, setSelectedIndex] = useState(0);
	const [errorMessage, setErrorMessage] = useState('');
	const [failOptionIndex, setFailOptionIndex] = useState(0);
	const [manualModel, setManualModel] = useState('');
	const [spinnerFrame, setSpinnerFrame] = useState(0);

	// -----------------------------------------------------------------------
	// Spinner animation
	// -----------------------------------------------------------------------

	useEffect(() => {
		if (step !== 'testing') return;

		const timer = setInterval(() => {
			setSpinnerFrame((prev) => (prev + 1) % SPINNER_FRAMES.length);
		}, 80);

		return () => clearInterval(timer);
	}, [step]);

	// -----------------------------------------------------------------------
	// Connection test
	// -----------------------------------------------------------------------

	const runConnectionTest = useCallback(
		(targetUrl: string) => {
			setStep('testing');
			setSpinnerFrame(0);

			(async () => {
				const result = await testOllamaConnection(targetUrl);

				if (!result.ok) {
					setErrorMessage(result.error);
					setFailOptionIndex(0);
					setStep('test-failed');
					return;
				}

				const discovered = await listOllamaModels(targetUrl);

				if (discovered.length > 0) {
					setModels(discovered);
					setSelectedIndex(0);
					setStep('model-select');
				} else {
					setStep('model-manual');
				}
			})();
		},
		[],
	);

	// -----------------------------------------------------------------------
	// Input handlers per step
	// -----------------------------------------------------------------------

	// URL input step
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
			}
		},
		{ isActive: step === 'url-input' },
	);

	// Test failed step
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setFailOptionIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setFailOptionIndex((prev) => (prev < 2 ? prev + 1 : prev));
				return;
			}
			if (key.return) {
				if (failOptionIndex === 0) {
					// Retry
					runConnectionTest(url);
				} else if (failOptionIndex === 1) {
					// Change URL
					setStep('url-input');
				} else {
					// Ignore & continue
					setStep('model-manual');
				}
			}
		},
		{ isActive: step === 'test-failed' },
	);

	// Model select step
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setSelectedIndex((prev) =>
					prev < models.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				const model = models[selectedIndex];
				if (model) {
					onComplete({ url, model: model.name });
				}
			}
		},
		{ isActive: step === 'model-select' },
	);

	// Model manual step
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
			}
		},
		{ isActive: step === 'model-manual' },
	);

	// -----------------------------------------------------------------------
	// Render
	// -----------------------------------------------------------------------

	if (step === 'url-input') {
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Ollama Server URL</Text>
				<Text> </Text>
				<Box paddingLeft={2}>
					<Text dimColor>{'> '}</Text>
					<TextInput
						value={url}
						onChange={setUrl}
						onSubmit={(val) => {
							const trimmed = val.trim();
							if (trimmed) {
								setUrl(trimmed);
								runConnectionTest(trimmed);
							}
						}}
						placeholder={DEFAULT_OLLAMA_URL}
					/>
				</Box>
				<Text> </Text>
				<Text dimColor>{'  \u21B5 connect  esc cancel'}</Text>
			</Box>
		);
	}

	if (step === 'testing') {
		const frame = SPINNER_FRAMES[spinnerFrame] ?? SPINNER_FRAMES[0];
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Ollama Server URL</Text>
				<Text> </Text>
				<Box paddingLeft={2}>
					<Text color="cyan">{frame} </Text>
					<Text>Testing connection to </Text>
					<Text bold>{url}</Text>
					<Text>...</Text>
				</Box>
			</Box>
		);
	}

	if (step === 'test-failed') {
		const failOptions = ['Retry', 'Change URL', 'Ignore & continue'];
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Ollama Server URL</Text>
				<Text> </Text>
				<Box paddingLeft={2}>
					<Text color="red">{'\u2718'} </Text>
					<Text>Connection failed: </Text>
					<Text dimColor>{errorMessage}</Text>
				</Box>
				<Text> </Text>
				{failOptions.map((option, i) => {
					const isSelected = i === failOptionIndex;
					return (
						<Box key={option} paddingLeft={2}>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '\u276F ' : '  '}
							</Text>
							<Text
								bold={isSelected}
								color={isSelected ? 'cyan' : undefined}
							>
								{option}
							</Text>
						</Box>
					);
				})}
				<Text> </Text>
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5 select  esc cancel'}
				</Text>
			</Box>
		);
	}

	if (step === 'model-select') {
		return (
			<Box flexDirection="column" paddingLeft={2} marginY={1}>
				<Text bold>Select Model</Text>
				<Text> </Text>
				{models.map((model, i) => {
					const isSelected = i === selectedIndex;
					return (
						<Box key={model.name} paddingLeft={2}>
							<Text color={isSelected ? 'cyan' : undefined}>
								{isSelected ? '\u276F ' : '  '}
							</Text>
							<Text
								bold={isSelected}
								color={isSelected ? 'cyan' : undefined}
							>
								{model.name}
							</Text>
							<Text dimColor> ({model.size})</Text>
						</Box>
					);
				})}
				<Text> </Text>
				<Text dimColor>
					{'  \u2191\u2193 navigate  \u21B5 select  esc cancel'}
				</Text>
			</Box>
		);
	}

	// model-manual
	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold>Enter Model Name</Text>
			<Text> </Text>
			<Box paddingLeft={2}>
				<Text dimColor>{'> '}</Text>
				<TextInput
					value={manualModel}
					onChange={setManualModel}
					onSubmit={(val) => {
						const trimmed = val.trim();
						if (trimmed) {
							onComplete({ url, model: trimmed });
						}
					}}
					placeholder="llama3.2:latest"
				/>
			</Box>
			<Text> </Text>
			<Text dimColor>{'  \u21B5 confirm  esc cancel'}</Text>
		</Box>
	);
}
