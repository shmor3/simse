import { Box, Text, useInput } from 'ink';
import { useCallback, useState } from 'react';
import {
	writeOnboardingFiles,
	type OnboardingResult,
} from '../../features/config/onboarding.js';
import { OllamaWizard } from './ollama-wizard.js';
import { TextInput } from './text-input.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface OnboardingWizardProps {
	readonly dataDir: string;
	readonly onComplete: (filesCreated: readonly string[]) => void;
	readonly onDismiss: () => void;
}

interface ACPConfig {
	readonly name: string;
	readonly command: string;
	readonly args?: readonly string[];
}

type SummarizeChoice =
	| 'same'
	| 'skip'
	| {
			readonly name: string;
			readonly command: string;
			readonly args?: readonly string[];
	  };

type EmbedChoice =
	| { readonly kind: 'local'; readonly model: string }
	| { readonly kind: 'tei'; readonly url: string };

interface LibrarySettings {
	readonly enabled?: boolean;
	readonly similarityThreshold?: number;
	readonly maxResults?: number;
	readonly autoSummarizeThreshold?: number;
}

interface WizardData {
	acp?: ACPConfig;
	summarize?: SummarizeChoice;
	embed?: EmbedChoice;
	library?: LibrarySettings;
	logLevel?: string;
}

// ---------------------------------------------------------------------------
// Option definitions
// ---------------------------------------------------------------------------

interface SelectOption {
	readonly label: string;
	readonly description?: string;
	readonly recommended?: boolean;
}

const ACP_OPTIONS: readonly SelectOption[] = [
	{ label: 'simse-engine', description: 'Built-in simse engine' },
	{ label: 'Ollama', description: 'Local AI via Ollama' },
	{ label: 'Claude Code', description: 'Anthropic Claude', recommended: true },
	{ label: 'GitHub Copilot', description: 'GitHub Copilot CLI' },
	{ label: 'Custom', description: 'Enter a custom command' },
];

const SUMMARIZE_OPTIONS: readonly SelectOption[] = [
	{
		label: 'Same as main',
		description: 'Use the same ACP provider',
		recommended: true,
	},
	{ label: 'Different provider', description: 'Configure a separate server' },
	{ label: 'Skip', description: 'Disable summarization' },
];

const EMBED_OPTIONS: readonly SelectOption[] = [
	{
		label: 'Small: snowflake-arctic-embed-xs',
		description: '22M params',
	},
	{
		label: 'Medium: nomic-embed-text-v1.5',
		description: '137M params',
		recommended: true,
	},
	{
		label: 'Large: snowflake-arctic-embed-l',
		description: '335M params',
	},
	{ label: 'TEI server', description: 'Text Embeddings Inference URL' },
];

const LIBRARY_OPTIONS: readonly SelectOption[] = [
	{
		label: 'Use recommended defaults',
		description: 'enabled, threshold=0.7, max=10, summarize=20',
		recommended: true,
	},
	{ label: 'Customize', description: 'Set each value individually' },
];

const LOG_LEVEL_OPTIONS: readonly SelectOption[] = [
	{ label: 'debug' },
	{ label: 'info' },
	{ label: 'warn', recommended: true },
	{ label: 'error' },
	{ label: 'none' },
];

const STEP_LABELS: readonly string[] = [
	'ACP Provider',
	'Summarization',
	'Embedding Model',
	'Library Settings',
	'Log Level',
	'Review & Confirm',
];

// ---------------------------------------------------------------------------
// Sub-step types
// ---------------------------------------------------------------------------

type Step1Sub = 'select' | 'ollama-wizard' | 'custom-input';
type Step2Sub = 'select' | 'provider-select' | 'provider-ollama' | 'provider-custom';
type Step3Sub = 'select' | 'tei-input';
type Step4Sub = 'select' | 'field-enabled' | 'field-threshold' | 'field-max' | 'field-summarize';

// ---------------------------------------------------------------------------
// ArrowSelect helper component
// ---------------------------------------------------------------------------

function ArrowSelect({
	options,
	selectedIndex,
	title,
}: {
	readonly options: readonly SelectOption[];
	readonly selectedIndex: number;
	readonly title: string;
}) {
	return (
		<Box flexDirection="column">
			<Text bold>{title}</Text>
			<Text> </Text>
			{options.map((opt, i) => {
				const isSelected = i === selectedIndex;
				return (
					<Box key={opt.label} paddingLeft={2}>
						<Text color={isSelected ? 'cyan' : undefined}>
							{isSelected ? '\u276F ' : '  '}
						</Text>
						<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
							{opt.label}
						</Text>
						{opt.recommended ? (
							<Text color="green"> (Recommended)</Text>
						) : null}
						{opt.description ? (
							<Text dimColor> {opt.description}</Text>
						) : null}
					</Box>
				);
			})}
			<Text> </Text>
			<Text dimColor>
				{'  \u2191\u2193 navigate  \u21B5 select  esc back'}
			</Text>
		</Box>
	);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function OnboardingWizard({
	dataDir,
	onComplete,
	onDismiss,
}: OnboardingWizardProps) {
	const [currentStep, setCurrentStep] = useState(1);
	const [wizardData, setWizardData] = useState<WizardData>({});

	// Selection indexes per step
	const [step1Index, setStep1Index] = useState(0);
	const [step1Sub, setStep1Sub] = useState<Step1Sub>('select');
	const [customCommand, setCustomCommand] = useState('');

	const [step2Index, setStep2Index] = useState(0);
	const [step2Sub, setStep2Sub] = useState<Step2Sub>('select');
	const [step2ProviderIndex, setStep2ProviderIndex] = useState(0);
	const [step2CustomCommand, setStep2CustomCommand] = useState('');

	const [step3Index, setStep3Index] = useState(1); // Pre-select Medium
	const [step3Sub, setStep3Sub] = useState<Step3Sub>('select');
	const [teiUrl, setTeiUrl] = useState('');

	const [step4Index, setStep4Index] = useState(0);
	const [step4Sub, setStep4Sub] = useState<Step4Sub>('select');
	const [libEnabled, setLibEnabled] = useState('y');
	const [libThreshold, setLibThreshold] = useState('0.7');
	const [libMax, setLibMax] = useState('10');
	const [libSummarize, setLibSummarize] = useState('20');

	const [step5Index, setStep5Index] = useState(2); // Pre-select warn

	// -----------------------------------------------------------------------
	// Navigation helpers
	// -----------------------------------------------------------------------

	const goBack = useCallback(() => {
		if (currentStep <= 1) {
			onDismiss();
		} else {
			setCurrentStep((s) => s - 1);
		}
	}, [currentStep, onDismiss]);

	const goNext = useCallback(() => {
		setCurrentStep((s) => s + 1);
	}, []);

	// -----------------------------------------------------------------------
	// Step 1: ACP Provider
	// -----------------------------------------------------------------------

	const handleStep1Select = useCallback(
		(acp: ACPConfig) => {
			setWizardData((d) => ({ ...d, acp }));
			setStep1Sub('select');
			goNext();
		},
		[goNext],
	);

	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setStep1Index((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep1Index((prev) =>
					prev < ACP_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				switch (step1Index) {
					case 0: // simse-engine
						handleStep1Select({
							name: 'simse-engine',
							command: 'simse-engine',
						});
						break;
					case 1: // Ollama
						setStep1Sub('ollama-wizard');
						break;
					case 2: // Claude Code
						handleStep1Select({
							name: 'claude',
							command: 'bunx',
							args: ['claude-code-acp'],
						});
						break;
					case 3: // GitHub Copilot
						handleStep1Select({
							name: 'copilot',
							command: 'copilot',
							args: ['--acp'],
						});
						break;
					case 4: // Custom
						setStep1Sub('custom-input');
						break;
				}
			}
		},
		{ isActive: currentStep === 1 && step1Sub === 'select' },
	);

	// Step 1 custom input escape
	useInput(
		(_input, key) => {
			if (key.escape) {
				setStep1Sub('select');
				setCustomCommand('');
			}
		},
		{ isActive: currentStep === 1 && step1Sub === 'custom-input' },
	);

	// -----------------------------------------------------------------------
	// Step 2: Summarization
	// -----------------------------------------------------------------------

	// Step 2 provider sub-select options (reuse ACP_OPTIONS)
	const STEP2_PROVIDER_OPTIONS = ACP_OPTIONS;

	const handleStep2ProviderSelect = useCallback(
		(config: ACPConfig) => {
			setWizardData((d) => ({ ...d, summarize: config }));
			setStep2Sub('select');
			goNext();
		},
		[goNext],
	);

	useInput(
		(_input, key) => {
			if (key.escape) {
				goBack();
				return;
			}
			if (key.upArrow) {
				setStep2Index((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep2Index((prev) =>
					prev < SUMMARIZE_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				switch (step2Index) {
					case 0: // Same as main
						setWizardData((d) => ({ ...d, summarize: 'same' }));
						goNext();
						break;
					case 1: // Different provider
						setStep2Sub('provider-select');
						setStep2ProviderIndex(0);
						break;
					case 2: // Skip
						setWizardData((d) => ({ ...d, summarize: 'skip' }));
						goNext();
						break;
				}
			}
		},
		{ isActive: currentStep === 2 && step2Sub === 'select' },
	);

	// Step 2 provider sub-select
	useInput(
		(_input, key) => {
			if (key.escape) {
				setStep2Sub('select');
				return;
			}
			if (key.upArrow) {
				setStep2ProviderIndex((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep2ProviderIndex((prev) =>
					prev < STEP2_PROVIDER_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				switch (step2ProviderIndex) {
					case 0: // simse-engine
						handleStep2ProviderSelect({
							name: 'simse-engine',
							command: 'simse-engine',
						});
						break;
					case 1: // Ollama
						setStep2Sub('provider-ollama');
						break;
					case 2: // Claude Code
						handleStep2ProviderSelect({
							name: 'claude',
							command: 'bunx',
							args: ['claude-code-acp'],
						});
						break;
					case 3: // GitHub Copilot
						handleStep2ProviderSelect({
							name: 'copilot',
							command: 'copilot',
							args: ['--acp'],
						});
						break;
					case 4: // Custom
						setStep2Sub('provider-custom');
						setStep2CustomCommand('');
						break;
				}
			}
		},
		{ isActive: currentStep === 2 && step2Sub === 'provider-select' },
	);

	// Step 2 provider custom input escape
	useInput(
		(_input, key) => {
			if (key.escape) {
				setStep2Sub('provider-select');
				setStep2CustomCommand('');
			}
		},
		{ isActive: currentStep === 2 && step2Sub === 'provider-custom' },
	);

	// -----------------------------------------------------------------------
	// Step 3: Embedding Model
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				goBack();
				return;
			}
			if (key.upArrow) {
				setStep3Index((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep3Index((prev) =>
					prev < EMBED_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				const embedModels = [
					'Snowflake/snowflake-arctic-embed-xs',
					'nomic-ai/nomic-embed-text-v1.5',
					'Snowflake/snowflake-arctic-embed-l',
				];
				if (step3Index < 3) {
					const model = embedModels[step3Index];
					if (model) {
						setWizardData((d) => ({
							...d,
							embed: { kind: 'local', model },
						}));
						goNext();
					}
				} else {
					// TEI server
					setStep3Sub('tei-input');
					setTeiUrl('');
				}
			}
		},
		{ isActive: currentStep === 3 && step3Sub === 'select' },
	);

	// Step 3 TEI input escape
	useInput(
		(_input, key) => {
			if (key.escape) {
				setStep3Sub('select');
				setTeiUrl('');
			}
		},
		{ isActive: currentStep === 3 && step3Sub === 'tei-input' },
	);

	// -----------------------------------------------------------------------
	// Step 4: Library Settings
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				goBack();
				return;
			}
			if (key.upArrow) {
				setStep4Index((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep4Index((prev) =>
					prev < LIBRARY_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				if (step4Index === 0) {
					// Use defaults
					setWizardData((d) => ({ ...d, library: undefined }));
					goNext();
				} else {
					// Customize
					setStep4Sub('field-enabled');
					setLibEnabled('y');
				}
			}
		},
		{ isActive: currentStep === 4 && step4Sub === 'select' },
	);

	// Step 4 field inputs — escape goes back to select
	useInput(
		(_input, key) => {
			if (key.escape) {
				setStep4Sub('select');
			}
		},
		{
			isActive:
				currentStep === 4 &&
				(step4Sub === 'field-enabled' ||
					step4Sub === 'field-threshold' ||
					step4Sub === 'field-max' ||
					step4Sub === 'field-summarize'),
		},
	);

	// -----------------------------------------------------------------------
	// Step 5: Log Level
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				goBack();
				return;
			}
			if (key.upArrow) {
				setStep5Index((prev) => (prev > 0 ? prev - 1 : prev));
				return;
			}
			if (key.downArrow) {
				setStep5Index((prev) =>
					prev < LOG_LEVEL_OPTIONS.length - 1 ? prev + 1 : prev,
				);
				return;
			}
			if (key.return) {
				const level = LOG_LEVEL_OPTIONS[step5Index];
				if (level) {
					setWizardData((d) => ({ ...d, logLevel: level.label }));
					goNext();
				}
			}
		},
		{ isActive: currentStep === 5 },
	);

	// -----------------------------------------------------------------------
	// Step 6: Review & Confirm
	// -----------------------------------------------------------------------

	useInput(
		(_input, key) => {
			if (key.escape) {
				goBack();
				return;
			}
			if (key.return) {
				// Write files and complete
				const result: OnboardingResult = {
					acp: wizardData.acp ?? {
						name: 'simse-engine',
						command: 'simse-engine',
					},
					summarize: wizardData.summarize ?? 'same',
					embed: wizardData.embed ?? {
						kind: 'local',
						model: 'nomic-ai/nomic-embed-text-v1.5',
					},
					library: wizardData.library,
					logLevel: wizardData.logLevel ?? 'warn',
				};
				const filesCreated = writeOnboardingFiles(dataDir, result);
				onComplete(filesCreated);
			}
		},
		{ isActive: currentStep === 6 },
	);

	// -----------------------------------------------------------------------
	// Step indicator
	// -----------------------------------------------------------------------

	const stepLabel = STEP_LABELS[currentStep - 1] ?? '';
	const stepIndicator = `(${currentStep}/6) ${stepLabel}`;

	// -----------------------------------------------------------------------
	// Render
	// -----------------------------------------------------------------------

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold color="cyan">
				{stepIndicator}
			</Text>
			<Text> </Text>
			{currentStep === 1 && renderStep1()}
			{currentStep === 2 && renderStep2()}
			{currentStep === 3 && renderStep3()}
			{currentStep === 4 && renderStep4()}
			{currentStep === 5 && renderStep5()}
			{currentStep === 6 && renderStep6()}
		</Box>
	);

	// -----------------------------------------------------------------------
	// Step renderers
	// -----------------------------------------------------------------------

	function renderStep1() {
		if (step1Sub === 'ollama-wizard') {
			return (
				<OllamaWizard
					onComplete={({ url, model }) => {
						handleStep1Select({
							name: 'ollama',
							command: 'bun',
							args: [
								'run',
								'acp-ollama-bridge.ts',
								'--ollama',
								url,
								'--model',
								model,
							],
						});
					}}
					onDismiss={() => {
						setStep1Sub('select');
					}}
				/>
			);
		}

		if (step1Sub === 'custom-input') {
			return (
				<Box flexDirection="column">
					<Text bold>Enter custom ACP server command:</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={customCommand}
							onChange={setCustomCommand}
							onSubmit={(val) => {
								const trimmed = val.trim();
								if (trimmed) {
									const parts = trimmed.split(/\s+/).filter(Boolean);
									const command = parts[0] as string;
									const args = parts.slice(1);
									const name =
										command.replace(/[^a-zA-Z0-9-]/g, '').toLowerCase() ||
										'custom';
									handleStep1Select({
										name,
										command,
										...(args.length > 0 && { args }),
									});
								}
							}}
							placeholder="my-server --port 8080"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		return (
			<ArrowSelect
				options={ACP_OPTIONS}
				selectedIndex={step1Index}
				title="Select ACP Provider"
			/>
		);
	}

	function renderStep2() {
		if (step2Sub === 'provider-ollama') {
			return (
				<OllamaWizard
					onComplete={({ url, model }) => {
						handleStep2ProviderSelect({
							name: 'ollama',
							command: 'bun',
							args: [
								'run',
								'acp-ollama-bridge.ts',
								'--ollama',
								url,
								'--model',
								model,
							],
						});
					}}
					onDismiss={() => {
						setStep2Sub('provider-select');
					}}
				/>
			);
		}

		if (step2Sub === 'provider-custom') {
			return (
				<Box flexDirection="column">
					<Text bold>Enter summarization server command:</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={step2CustomCommand}
							onChange={setStep2CustomCommand}
							onSubmit={(val) => {
								const trimmed = val.trim();
								if (trimmed) {
									const parts = trimmed.split(/\s+/).filter(Boolean);
									const command = parts[0] as string;
									const args = parts.slice(1);
									const name =
										command.replace(/[^a-zA-Z0-9-]/g, '').toLowerCase() ||
										'custom';
									handleStep2ProviderSelect({
										name,
										command,
										...(args.length > 0 && { args }),
									});
								}
							}}
							placeholder="my-server --port 8080"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		if (step2Sub === 'provider-select') {
			return (
				<ArrowSelect
					options={STEP2_PROVIDER_OPTIONS}
					selectedIndex={step2ProviderIndex}
					title="Select Summarization Provider"
				/>
			);
		}

		return (
			<ArrowSelect
				options={SUMMARIZE_OPTIONS}
				selectedIndex={step2Index}
				title="Configure Summarization"
			/>
		);
	}

	function renderStep3() {
		if (step3Sub === 'tei-input') {
			return (
				<Box flexDirection="column">
					<Text bold>Enter TEI server URL:</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={teiUrl}
							onChange={setTeiUrl}
							onSubmit={(val) => {
								const trimmed = val.trim();
								if (trimmed) {
									setWizardData((d) => ({
										...d,
										embed: { kind: 'tei', url: trimmed },
									}));
									setStep3Sub('select');
									goNext();
								}
							}}
							placeholder="http://localhost:8080"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		return (
			<ArrowSelect
				options={EMBED_OPTIONS}
				selectedIndex={step3Index}
				title="Select Embedding Model"
			/>
		);
	}

	function renderStep4() {
		if (step4Sub === 'field-enabled') {
			return (
				<Box flexDirection="column">
					<Text bold>Library enabled? (y/n)</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={libEnabled}
							onChange={setLibEnabled}
							onSubmit={(val) => {
								const trimmed = val.trim().toLowerCase();
								if (trimmed === 'y' || trimmed === 'n') {
									setLibEnabled(trimmed);
									setStep4Sub('field-threshold');
								}
							}}
							placeholder="y"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		if (step4Sub === 'field-threshold') {
			return (
				<Box flexDirection="column">
					<Text bold>Similarity threshold (0-1):</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={libThreshold}
							onChange={setLibThreshold}
							onSubmit={(val) => {
								const num = Number.parseFloat(val.trim());
								if (!Number.isNaN(num) && num >= 0 && num <= 1) {
									setLibThreshold(val.trim());
									setStep4Sub('field-max');
								}
							}}
							placeholder="0.7"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		if (step4Sub === 'field-max') {
			return (
				<Box flexDirection="column">
					<Text bold>Max results:</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={libMax}
							onChange={setLibMax}
							onSubmit={(val) => {
								const num = Number.parseInt(val.trim(), 10);
								if (!Number.isNaN(num) && num > 0) {
									setLibMax(val.trim());
									setStep4Sub('field-summarize');
								}
							}}
							placeholder="10"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		if (step4Sub === 'field-summarize') {
			return (
				<Box flexDirection="column">
					<Text bold>Auto-summarize threshold:</Text>
					<Text> </Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={libSummarize}
							onChange={setLibSummarize}
							onSubmit={(val) => {
								const num = Number.parseInt(val.trim(), 10);
								if (!Number.isNaN(num) && num > 0) {
									setLibSummarize(val.trim());
									setWizardData((d) => ({
										...d,
										library: {
											enabled: libEnabled === 'y',
											similarityThreshold: Number.parseFloat(libThreshold),
											maxResults: Number.parseInt(libMax, 10),
											autoSummarizeThreshold: num,
										},
									}));
									setStep4Sub('select');
									goNext();
								}
							}}
							placeholder="20"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
				</Box>
			);
		}

		return (
			<ArrowSelect
				options={LIBRARY_OPTIONS}
				selectedIndex={step4Index}
				title="Library Settings"
			/>
		);
	}

	function renderStep5() {
		return (
			<ArrowSelect
				options={LOG_LEVEL_OPTIONS}
				selectedIndex={step5Index}
				title="Select Log Level"
			/>
		);
	}

	function renderStep6() {
		const acp = wizardData.acp ?? {
			name: 'simse-engine',
			command: 'simse-engine',
		};
		const summarize = wizardData.summarize ?? 'same';
		const embed = wizardData.embed ?? {
			kind: 'local' as const,
			model: 'nomic-ai/nomic-embed-text-v1.5',
		};
		const library = wizardData.library;
		const logLevel = wizardData.logLevel ?? 'warn';

		const summarizeLabel =
			summarize === 'same'
				? 'Same as main provider'
				: summarize === 'skip'
					? 'Disabled'
					: `${summarize.name} (${summarize.command})`;

		const embedLabel =
			embed.kind === 'local' ? embed.model : `TEI: ${embed.url}`;

		return (
			<Box flexDirection="column">
				<Text bold>Review Configuration</Text>
				<Text> </Text>
				<Box paddingLeft={2} flexDirection="column">
					<Box>
						<Text bold>{'Provider:      '}</Text>
						<Text>
							{acp.name} ({acp.command}
							{acp.args ? ` ${acp.args.join(' ')}` : ''})
						</Text>
					</Box>
					<Box>
						<Text bold>{'Summarization: '}</Text>
						<Text>{summarizeLabel}</Text>
					</Box>
					<Box>
						<Text bold>{'Embedding:     '}</Text>
						<Text>{embedLabel}</Text>
					</Box>
					<Box>
						<Text bold>{'Library:       '}</Text>
						<Text>
							{library
								? `enabled=${String(library.enabled ?? true)}, threshold=${String(library.similarityThreshold ?? 0.7)}, max=${String(library.maxResults ?? 10)}, summarize=${String(library.autoSummarizeThreshold ?? 20)}`
								: 'Recommended defaults'}
						</Text>
					</Box>
					<Box>
						<Text bold>{'Log level:     '}</Text>
						<Text>{logLevel}</Text>
					</Box>
				</Box>
				<Text> </Text>
				<Box paddingLeft={2} flexDirection="column">
					<Text dimColor>Files will be written to: {dataDir}</Text>
				</Box>
				<Text> </Text>
				<Text dimColor>{'  \u21B5 confirm  esc back'}</Text>
			</Box>
		);
	}
}
