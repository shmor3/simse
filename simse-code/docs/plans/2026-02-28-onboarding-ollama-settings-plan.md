# Onboarding Wizard, Ollama Setup & Settings Explorer — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the readline-based first-boot setup with a full Ink-based onboarding wizard, add connection testing and model discovery to the Ollama setup flow, and create an interactive settings explorer for post-setup editing.

**Architecture:** Three new Ink components (OllamaWizard, OnboardingWizard, SettingsExplorer) plus supporting logic modules. The onboarding wizard is triggered when `hasACP` is false on boot. The ollama wizard is a sub-component used by both onboarding and `/setup ollama`. The settings explorer is a new `/settings` command that renders an interactive modal.

**Tech Stack:** React (Ink), TypeScript, fetch for connection testing, existing JSON file I/O patterns

---

## Task 1: Ollama Connection Testing Module

**Files:**
- Create: `simse-code/features/config/ollama-test.ts`
- Test: `simse-code/features/config/__tests__/ollama-test.test.ts`

**Step 1: Write the failing tests**

Create `simse-code/features/config/__tests__/ollama-test.test.ts`:

```typescript
import { describe, expect, it, mock } from 'bun:test';
import { listOllamaModels, testOllamaConnection } from '../ollama-test.js';

describe('testOllamaConnection', () => {
	it('returns success when ollama responds', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(() =>
			Promise.resolve(new Response(JSON.stringify({ models: [] }), { status: 200 })),
		);
		try {
			const result = await testOllamaConnection('http://localhost:11434');
			expect(result.ok).toBe(true);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it('returns failure on network error', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(() => Promise.reject(new Error('ECONNREFUSED')));
		try {
			const result = await testOllamaConnection('http://localhost:11434');
			expect(result.ok).toBe(false);
			expect(result.error).toContain('ECONNREFUSED');
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it('returns failure on non-200 status', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(() =>
			Promise.resolve(new Response('Not Found', { status: 404 })),
		);
		try {
			const result = await testOllamaConnection('http://localhost:11434');
			expect(result.ok).toBe(false);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it('returns failure on timeout', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(
			() => new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 100)),
		);
		try {
			const result = await testOllamaConnection('http://localhost:11434', 50);
			expect(result.ok).toBe(false);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});
});

describe('listOllamaModels', () => {
	it('returns model names from /api/tags response', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(() =>
			Promise.resolve(
				new Response(
					JSON.stringify({
						models: [
							{ name: 'llama3.2:latest', size: 2_000_000_000 },
							{ name: 'mistral:7b', size: 4_000_000_000 },
						],
					}),
					{ status: 200 },
				),
			),
		);
		try {
			const models = await listOllamaModels('http://localhost:11434');
			expect(models).toEqual([
				{ name: 'llama3.2:latest', size: '1.9 GB' },
				{ name: 'mistral:7b', size: '3.7 GB' },
			]);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it('returns empty array on failure', async () => {
		const originalFetch = globalThis.fetch;
		globalThis.fetch = mock(() => Promise.reject(new Error('fail')));
		try {
			const models = await listOllamaModels('http://localhost:11434');
			expect(models).toEqual([]);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-code && bun test features/config/__tests__/ollama-test.test.ts`
Expected: FAIL — module `../ollama-test.js` not found

**Step 3: Write the implementation**

Create `simse-code/features/config/ollama-test.ts`:

```typescript
/**
 * Ollama connection testing and model discovery.
 *
 * Used by both the onboarding wizard and /setup ollama to verify
 * the Ollama server is reachable and list available models.
 */

export interface OllamaConnectionResult {
	readonly ok: boolean;
	readonly error?: string;
	readonly version?: string;
}

export interface OllamaModelInfo {
	readonly name: string;
	readonly size: string;
}

function formatBytes(bytes: number): string {
	if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
	if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`;
	return `${(bytes / 1_000).toFixed(1)} KB`;
}

/**
 * Test whether an Ollama server is reachable at the given URL.
 * Hits GET /api/tags with a timeout.
 */
export async function testOllamaConnection(
	url: string,
	timeoutMs = 5000,
): Promise<OllamaConnectionResult> {
	try {
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), timeoutMs);

		const response = await fetch(`${url.replace(/\/+$/, '')}/api/tags`, {
			signal: controller.signal,
		});
		clearTimeout(timer);

		if (!response.ok) {
			return { ok: false, error: `HTTP ${response.status}: ${response.statusText}` };
		}

		// Try to extract version from headers if available
		const version = response.headers.get('x-ollama-version') ?? undefined;
		return { ok: true, version };
	} catch (err) {
		const message = err instanceof Error ? err.message : 'Unknown error';
		return { ok: false, error: message };
	}
}

/**
 * List available models on an Ollama server.
 * Returns empty array on failure.
 */
export async function listOllamaModels(
	url: string,
	timeoutMs = 5000,
): Promise<readonly OllamaModelInfo[]> {
	try {
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), timeoutMs);

		const response = await fetch(`${url.replace(/\/+$/, '')}/api/tags`, {
			signal: controller.signal,
		});
		clearTimeout(timer);

		if (!response.ok) return [];

		const data = (await response.json()) as {
			models?: readonly { name: string; size?: number }[];
		};

		return (data.models ?? []).map((m) => ({
			name: m.name,
			size: m.size ? formatBytes(m.size) : 'unknown',
		}));
	} catch {
		return [];
	}
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-code && bun test features/config/__tests__/ollama-test.test.ts`
Expected: All 5 tests PASS

**Step 5: Run lint and typecheck**

Run: `cd simse-code && bun run lint && bun run typecheck`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-code/features/config/ollama-test.ts simse-code/features/config/__tests__/ollama-test.test.ts
git commit -m "feat: add ollama connection testing and model discovery module"
```

---

## Task 2: Ollama Wizard Component

**Files:**
- Create: `simse-code/components/input/ollama-wizard.tsx`

This is a multi-step Ink component that handles URL input, connection testing (with spinner), and model selection from discovered models.

**Step 1: Create the component**

Create `simse-code/components/input/ollama-wizard.tsx`:

```tsx
import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useState } from 'react';
import {
	type OllamaModelInfo,
	listOllamaModels,
	testOllamaConnection,
} from '../../features/config/ollama-test.js';
import { TextInput } from './text-input.js';

export interface OllamaWizardResult {
	readonly url: string;
	readonly model: string;
}

interface OllamaWizardProps {
	readonly onComplete: (result: OllamaWizardResult) => void;
	readonly onDismiss: () => void;
	readonly defaultUrl?: string;
}

type WizardStep = 'url-input' | 'testing' | 'test-failed' | 'model-select' | 'model-manual';

const SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

function Spinner() {
	const [frame, setFrame] = useState(0);
	useEffect(() => {
		const timer = setInterval(() => setFrame((f) => (f + 1) % SPINNER_FRAMES.length), 80);
		return () => clearInterval(timer);
	}, []);
	return <Text color="magenta">{SPINNER_FRAMES[frame]} </Text>;
}

export function OllamaWizard({
	onComplete,
	onDismiss,
	defaultUrl = 'http://127.0.0.1:11434',
}: OllamaWizardProps) {
	const [step, setStep] = useState<WizardStep>('url-input');
	const [url, setUrl] = useState(defaultUrl);
	const [error, setError] = useState('');
	const [models, setModels] = useState<readonly OllamaModelInfo[]>([]);
	const [selectedModelIndex, setSelectedModelIndex] = useState(0);
	const [manualModel, setManualModel] = useState('llama3.2');
	const [failedSelectedIndex, setFailedSelectedIndex] = useState(0);

	const failedOptions = ['Retry', 'Change URL', 'Ignore & continue'] as const;

	const runConnectionTest = useCallback(
		async (testUrl: string) => {
			setStep('testing');
			setError('');

			const result = await testOllamaConnection(testUrl);

			if (result.ok) {
				const modelList = await listOllamaModels(testUrl);
				if (modelList.length > 0) {
					setModels(modelList);
					setSelectedModelIndex(0);
					setStep('model-select');
				} else {
					setStep('model-manual');
				}
			} else {
				setError(result.error ?? 'Connection failed');
				setFailedSelectedIndex(0);
				setStep('test-failed');
			}
		},
		[],
	);

	// Handle URL submit
	const handleUrlSubmit = useCallback(
		(value: string) => {
			const trimmed = value.trim() || defaultUrl;
			setUrl(trimmed);
			runConnectionTest(trimmed);
		},
		[defaultUrl, runConnectionTest],
	);

	// Arrow key navigation for test-failed options
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setFailedSelectedIndex((i) => Math.max(0, i - 1));
			}
			if (key.downArrow) {
				setFailedSelectedIndex((i) => Math.min(failedOptions.length - 1, i + 1));
			}
			if (key.return) {
				if (failedSelectedIndex === 0) {
					// Retry
					runConnectionTest(url);
				} else if (failedSelectedIndex === 1) {
					// Change URL
					setStep('url-input');
				} else {
					// Ignore & continue — fall back to manual model input
					setStep('model-manual');
				}
			}
		},
		{ isActive: step === 'test-failed' },
	);

	// Arrow key navigation for model selection
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setSelectedModelIndex((i) => Math.max(0, i - 1));
			}
			if (key.downArrow) {
				setSelectedModelIndex((i) => Math.min(models.length - 1, i + 1));
			}
			if (key.return) {
				const model = models[selectedModelIndex];
				if (model) {
					onComplete({ url, model: model.name });
				}
			}
		},
		{ isActive: step === 'model-select' },
	);

	// Escape for url-input (dismisses wizard)
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
			}
		},
		{ isActive: step === 'url-input' || step === 'model-manual' },
	);

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Text bold>Configure Ollama</Text>
			<Text> </Text>

			{/* Step 1: URL input */}
			{step === 'url-input' && (
				<>
					<Text>Ollama server URL:</Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={url}
							onChange={setUrl}
							onSubmit={handleUrlSubmit}
							placeholder={defaultUrl}
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  ↵ test connection  esc cancel'}</Text>
				</>
			)}

			{/* Step 2: Testing connection */}
			{step === 'testing' && (
				<Box>
					<Spinner />
					<Text>Testing connection to {url}...</Text>
				</Box>
			)}

			{/* Step 3a: Connection failed */}
			{step === 'test-failed' && (
				<>
					<Text color="red">  Connection failed: {error}</Text>
					<Text> </Text>
					{failedOptions.map((opt, i) => {
						const isSelected = i === failedSelectedIndex;
						return (
							<Box key={opt}>
								<Text color={isSelected ? 'cyan' : undefined}>
									{isSelected ? '  ❯ ' : '    '}
								</Text>
								<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
									{opt}
								</Text>
							</Box>
						);
					})}
					<Text> </Text>
					<Text dimColor>{'  ↑↓ navigate  ↵ select  esc cancel'}</Text>
				</>
			)}

			{/* Step 3b: Model selection from discovered models */}
			{step === 'model-select' && (
				<>
					<Text color="green">  Connected to Ollama at {url}</Text>
					<Text> </Text>
					<Text>Select a model:</Text>
					<Text> </Text>
					{models.map((m, i) => {
						const isSelected = i === selectedModelIndex;
						return (
							<Box key={m.name}>
								<Text color={isSelected ? 'cyan' : undefined}>
									{isSelected ? '  ❯ ' : '    '}
								</Text>
								<Text bold={isSelected} color={isSelected ? 'cyan' : undefined}>
									{m.name.padEnd(30)}
								</Text>
								<Text dimColor>{m.size}</Text>
							</Box>
						);
					})}
					<Text> </Text>
					<Text dimColor>{'  ↑↓ navigate  ↵ select  esc cancel'}</Text>
				</>
			)}

			{/* Step 3c: Manual model input (when no models found or ignore chosen) */}
			{step === 'model-manual' && (
				<>
					<Text>Enter model name:</Text>
					<Box paddingLeft={2}>
						<Text dimColor>{'> '}</Text>
						<TextInput
							value={manualModel}
							onChange={setManualModel}
							onSubmit={(val) => {
								const model = val.trim() || 'llama3.2';
								onComplete({ url, model });
							}}
							placeholder="llama3.2"
						/>
					</Box>
					<Text> </Text>
					<Text dimColor>{'  ↵ confirm  esc cancel'}</Text>
				</>
			)}
		</Box>
	);
}
```

**Step 2: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS (fix any formatting issues)

**Step 3: Commit**

```bash
git add simse-code/components/input/ollama-wizard.tsx
git commit -m "feat: add OllamaWizard component with connection testing and model discovery"
```

---

## Task 3: Integrate Ollama Wizard into `/setup` Command

**Files:**
- Modify: `simse-code/features/config/setup.ts` (lines 51-72 — ollama preset, and lines 165-291 — command factory)
- Modify: `simse-code/components/input/setup-selector.tsx` (lines 1-118 — add ollama wizard transition)
- Modify: `simse-code/app-ink.tsx` (lines 197-212 — pending setup state, lines 530-542 — setup selector rendering)

**Step 1: Update the setup selector to support sub-wizard transitions**

The `SetupSelector` needs a new mode for the ollama wizard. When user selects "Ollama", instead of immediately calling `onSelect`, it transitions to showing the `OllamaWizard`.

Modify `simse-code/components/input/setup-selector.tsx`:

Add `OllamaWizard` import and a new `'ollama-wizard'` mode. When mode is `'ollama-wizard'`, render `<OllamaWizard>` instead of the preset list. When `OllamaWizard` completes, call `onSelect` with the ollama preset key and the URL+model as custom args.

```tsx
// Add to imports
import { OllamaWizard } from './ollama-wizard.js';

// Change SelectorMode union
type SelectorMode = 'selecting' | 'custom-input' | 'ollama-wizard';

// In the useInput handler for 'selecting' mode, when key.return:
// Check if the selected preset key is 'ollama' — if so, setMode('ollama-wizard')
// instead of calling onSelect directly.

// Add useInput handler for 'ollama-wizard' mode (escape goes back to selecting)

// Add rendering branch for mode === 'ollama-wizard':
// <OllamaWizard
//   onComplete={({ url, model }) => onSelect({ presetKey: 'ollama', customArgs: `${url} ${model}` })}
//   onDismiss={() => { setMode('selecting'); }}
// />
```

**Step 2: Update the setup.ts ollama preset to accept URL+model from customArgs**

The ollama preset in `setup.ts` lines 55-71 already parses `args.split(/\s+/)` for URL and model. This should work as-is since the wizard passes `"<url> <model>"` as customArgs. Verify by reading the code — no changes needed here.

**Step 3: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 4: Manual test**

Run: `cd simse-code && bun run cli-ink.tsx`
Type: `/setup`
Select: Ollama
Expected: Ollama wizard appears with URL input, connection test, model selection

**Step 5: Commit**

```bash
git add simse-code/components/input/setup-selector.tsx
git commit -m "feat: integrate ollama wizard into /setup selector with connection testing"
```

---

## Task 4: Onboarding Wizard Component

**Files:**
- Create: `simse-code/components/input/onboarding-wizard.tsx`
- Create: `simse-code/features/config/onboarding.ts`

This is the multi-step onboarding wizard that replaces the readline-based `setup.ts` flow. It is shown when `hasACP` is false on boot.

**Step 1: Create the onboarding logic module**

Create `simse-code/features/config/onboarding.ts` — file writer that takes all wizard results and writes config files atomically:

```typescript
/**
 * Onboarding wizard logic — writes all config files atomically
 * after the full wizard flow completes.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export interface OnboardingResult {
	/** ACP provider preset key and config. */
	readonly acp: {
		readonly name: string;
		readonly command: string;
		readonly args?: readonly string[];
	};
	/** Summarization config: 'same' (reuse main), 'skip', or a separate server config. */
	readonly summarize:
		| 'same'
		| 'skip'
		| {
				readonly name: string;
				readonly command: string;
				readonly args?: readonly string[];
		  };
	/** Embedding config: model ID or TEI URL. */
	readonly embed:
		| { readonly kind: 'local'; readonly model: string }
		| { readonly kind: 'tei'; readonly url: string };
	/** Library settings (undefined means use defaults). */
	readonly library?: {
		readonly enabled?: boolean;
		readonly similarityThreshold?: number;
		readonly maxResults?: number;
		readonly autoSummarizeThreshold?: number;
	};
	/** Log level. */
	readonly logLevel: string;
}

export function writeOnboardingFiles(
	dataDir: string,
	result: OnboardingResult,
): readonly string[] {
	mkdirSync(dataDir, { recursive: true });
	const created: string[] = [];

	// acp.json — always write
	const acpConfig = {
		servers: [result.acp],
		defaultServer: result.acp.name,
	};
	writeFileSync(
		join(dataDir, 'acp.json'),
		JSON.stringify(acpConfig, null, '\t') + '\n',
		'utf-8',
	);
	created.push('acp.json');

	// summarize.json
	if (result.summarize === 'same') {
		const summarizeConfig = {
			server: result.acp.name,
			command: result.acp.command,
			...(result.acp.args && { args: result.acp.args }),
		};
		writeFileSync(
			join(dataDir, 'summarize.json'),
			JSON.stringify(summarizeConfig, null, '\t') + '\n',
			'utf-8',
		);
		created.push('summarize.json');
	} else if (result.summarize !== 'skip') {
		const summarizeConfig = {
			server: result.summarize.name,
			command: result.summarize.command,
			...(result.summarize.args && { args: result.summarize.args }),
		};
		writeFileSync(
			join(dataDir, 'summarize.json'),
			JSON.stringify(summarizeConfig, null, '\t') + '\n',
			'utf-8',
		);
		created.push('summarize.json');
	}

	// embed.json
	const embedConfig =
		result.embed.kind === 'local'
			? { embeddingModel: result.embed.model }
			: { teiUrl: result.embed.url };
	writeFileSync(
		join(dataDir, 'embed.json'),
		JSON.stringify(embedConfig, null, '\t') + '\n',
		'utf-8',
	);
	created.push('embed.json');

	// memory.json
	const libraryConfig = {
		enabled: result.library?.enabled ?? true,
		similarityThreshold: result.library?.similarityThreshold ?? 0.7,
		maxResults: result.library?.maxResults ?? 10,
		autoSummarizeThreshold: result.library?.autoSummarizeThreshold ?? 20,
	};
	writeFileSync(
		join(dataDir, 'memory.json'),
		JSON.stringify(libraryConfig, null, '\t') + '\n',
		'utf-8',
	);
	created.push('memory.json');

	// config.json — log level
	const configObj: Record<string, string> = {};
	if (result.logLevel !== 'warn') {
		configObj.logLevel = result.logLevel;
	}
	if (!existsSync(join(dataDir, 'config.json'))) {
		writeFileSync(
			join(dataDir, 'config.json'),
			JSON.stringify(configObj, null, '\t') + '\n',
			'utf-8',
		);
		created.push('config.json');
	}

	// mcp.json — empty servers array
	if (!existsSync(join(dataDir, 'mcp.json'))) {
		writeFileSync(
			join(dataDir, 'mcp.json'),
			JSON.stringify({ servers: [] }, null, '\t') + '\n',
			'utf-8',
		);
		created.push('mcp.json');
	}

	return created;
}
```

**Step 2: Create the onboarding wizard component**

Create `simse-code/components/input/onboarding-wizard.tsx`:

This is a large component with 6 steps. Each step is a sub-view rendered conditionally. State tracks the current step and accumulated results.

The component has these steps:
1. ACP Provider (arrow-select with sub-wizards for ollama/simse-engine/custom)
2. Summarization (3-option arrow-select)
3. Embedding (4-option arrow-select + TEI URL input)
4. Library Settings (2-option: defaults vs customize, then field editors)
5. Log Level (5-option arrow-select)
6. Review & Confirm (summary + enter to write)

Each step calls a shared `goNext()` to advance, stores results in a `wizardData` state object.

The OllamaWizard from Task 2 is reused as a sub-component in step 1.

**Important details:**
- Step indicator shows `(1/6)`, `(2/6)`, etc.
- "Recommended" label on the default option in each step
- Enter selects, Esc goes back one step (or cancels on step 1)
- Review step shows all selected values and Enter writes files + calls `onComplete`

I won't write the full 300+ line component here — the structure follows the exact same patterns as `SetupSelector` (useState for mode/index, useInput with isActive guards, arrow navigation, Box/Text rendering). Implement each step as a section in the component's return JSX, conditionally rendered based on `currentStep`.

**Step 3: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-code/features/config/onboarding.ts simse-code/components/input/onboarding-wizard.tsx
git commit -m "feat: add onboarding wizard component and file writer"
```

---

## Task 5: Integrate Onboarding Wizard into App Boot

**Files:**
- Modify: `simse-code/app-ink.tsx` (lines 70-81 — AppProps, lines 91-93 — initial items, lines 504-582 — render)

**Step 1: Add onboarding state to App**

In `app-ink.tsx`, add state to track whether onboarding is active:

```typescript
// After line 98 (useState for promptMode)
const [showOnboarding, setShowOnboarding] = useState(!initialHasACP);
```

Add import for the `OnboardingWizard` component.

**Step 2: Render onboarding wizard when active**

In the render section (around line 504), when `showOnboarding` is true, render `<OnboardingWizard>` instead of the normal prompt area. When onboarding completes:
1. Call `handleSetupComplete()` to re-bootstrap ACP
2. Set `showOnboarding` to false
3. Add info item showing files created

```tsx
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
		{/* existing PromptInput, StatusBar, etc. */}
	</>
)}
```

**Step 3: Suppress "No ACP server" error during onboarding**

In `handleSubmit` (line 436-445), the error about no ACP server should not show when onboarding is active. The `showOnboarding` flag handles this since the prompt input is hidden during onboarding.

**Step 4: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 5: Manual test**

1. Delete `~/.simse/acp.json` temporarily
2. Run: `cd simse-code && bun run cli-ink.tsx`
3. Expected: Onboarding wizard appears instead of normal prompt
4. Walk through all 6 steps
5. On completion, normal prompt appears and ACP is connected
6. Restore `~/.simse/acp.json`

**Step 6: Commit**

```bash
git add simse-code/app-ink.tsx
git commit -m "feat: trigger onboarding wizard on first boot when no ACP config exists"
```

---

## Task 6: Settings Schema Module

**Files:**
- Create: `simse-code/features/config/settings-schema.ts`
- Test: `simse-code/features/config/__tests__/settings-schema.test.ts`

This module defines the schema for each config file — field names, types, descriptions, defaults, and allowed values. It's used by the settings explorer to know what fields exist and how to edit them.

**Step 1: Write the failing test**

Create `simse-code/features/config/__tests__/settings-schema.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { getConfigSchema, getFieldType, loadConfigFile, saveConfigField } from '../settings-schema.js';

describe('getConfigSchema', () => {
	it('returns schema for known config files', () => {
		const schema = getConfigSchema('config.json');
		expect(schema).toBeDefined();
		expect(schema!.fields.length).toBeGreaterThan(0);
	});

	it('returns undefined for unknown files', () => {
		expect(getConfigSchema('unknown.json')).toBeUndefined();
	});
});

describe('getFieldType', () => {
	it('identifies string fields', () => {
		const schema = getConfigSchema('config.json');
		const logLevel = schema!.fields.find((f) => f.key === 'logLevel');
		expect(logLevel?.type).toBe('enum');
	});

	it('identifies number fields', () => {
		const schema = getConfigSchema('memory.json');
		const threshold = schema!.fields.find((f) => f.key === 'similarityThreshold');
		expect(threshold?.type).toBe('number');
	});

	it('identifies boolean fields', () => {
		const schema = getConfigSchema('memory.json');
		const enabled = schema!.fields.find((f) => f.key === 'enabled');
		expect(enabled?.type).toBe('boolean');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test features/config/__tests__/settings-schema.test.ts`
Expected: FAIL — module not found

**Step 3: Write the implementation**

Create `simse-code/features/config/settings-schema.ts`:

Define a `ConfigFileSchema` type with an array of `FieldSchema` entries. Each entry has: `key`, `type` (string | number | boolean | enum), `description`, `default`, `options?` (for enums). Create schema definitions for all 7 config files.

Also export `loadConfigFile(dataDir, filename)` and `saveConfigField(dataDir, filename, key, value)` — read JSON, update field, write back.

**Step 4: Run tests to verify they pass**

Run: `cd simse-code && bun test features/config/__tests__/settings-schema.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/features/config/settings-schema.ts simse-code/features/config/__tests__/settings-schema.test.ts
git commit -m "feat: add settings schema definitions for all config files"
```

---

## Task 7: Settings Explorer Component

**Files:**
- Create: `simse-code/components/input/settings-explorer.tsx`

An interactive Ink component for browsing and editing all config files. File navigation on the left, field editing on the right.

**Step 1: Create the component**

Create `simse-code/components/input/settings-explorer.tsx`:

The component has two panels:
- Left: list of config files (arrow-key navigable)
- Right: fields of selected file with current values

States:
- `selectedFileIndex` — which file is focused
- `selectedFieldIndex` — which field within the file is focused
- `editingField` — the field currently being edited (null if not editing)
- `editValue` — current edit buffer
- `panel` — 'files' | 'fields' (which panel has focus)

Navigation:
- Up/Down arrows move within current panel
- Right arrow (or Enter on a file) moves to fields panel
- Left arrow (or Esc in fields) moves back to files panel
- Enter on a field starts editing
- Enter during editing saves, Esc cancels
- Boolean fields toggle on Enter (no text input)
- Enum fields cycle through options on Enter

Render layout:
- Horizontal Box with two vertical sub-boxes
- File list uses cyan highlight for selected item
- Field list shows `key: value` pairs with types
- Active edit shows TextInput inline

**Step 2: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/components/input/settings-explorer.tsx
git commit -m "feat: add interactive settings explorer component"
```

---

## Task 8: Integrate Settings Explorer into `/settings` Command

**Files:**
- Modify: `simse-code/features/config/commands.ts` (lines 1-22 — replace stub)
- Modify: `simse-code/app-ink.tsx` (add pending settings state, render explorer)

**Step 1: Update commands.ts to trigger settings explorer**

Modify `simse-code/features/config/commands.ts`:

The `/settings` command should work like `/setup` — it calls an `onShowSettingsExplorer` callback that returns a Promise. The command factory receives this callback.

Change from static `configCommands` array to `createSettingsCommands(dataDir, onShowSettingsExplorer?)` factory function.

**Step 2: Add settings explorer modal state to app-ink.tsx**

Following the same Promise-based modal pattern as `pendingSetup`:

```typescript
const [pendingSettings, setPendingSettings] = useState<{
	resolve: () => void;
} | null>(null);

const handleShowSettingsExplorer = useCallback((): Promise<void> => {
	return new Promise((resolve) => {
		setPendingSettings({ resolve });
	});
}, []);
```

Render `<SettingsExplorer>` when `pendingSettings` is non-null. On dismiss, call `resolve()` and clear state.

**Step 3: Update imports in features/config/index.ts**

Export the new `createSettingsCommands` instead of `configCommands`.

**Step 4: Update command registration in app-ink.tsx**

Replace `reg.registerAll(configCommands)` with `reg.registerAll(createSettingsCommands(dataDir, handleShowSettingsExplorer))`.

**Step 5: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 6: Manual test**

Run: `cd simse-code && bun run cli-ink.tsx`
Type: `/settings`
Expected: Settings explorer appears, can navigate files and fields, edit values

**Step 7: Commit**

```bash
git add simse-code/features/config/commands.ts simse-code/features/config/index.ts simse-code/app-ink.tsx
git commit -m "feat: integrate settings explorer into /settings command"
```

---

## Task 9: Add Connection Testing to readline-based Ollama Setup

**Files:**
- Modify: `simse-code/setup.ts` (lines 192-214 — Ollama preset build function)

The global `setup.ts` readline flow is still used as a fallback. Add connection testing to the Ollama preset's `build()` function.

**Step 1: Import and use testOllamaConnection**

After the URL input, call `testOllamaConnection(url)`. Show result. If failed, ask whether to retry, change URL, or ignore.

If successful, call `listOllamaModels(url)` and display models as a numbered list for selection (readline-style, since Ink isn't available here).

**Step 2: Run lint and typecheck**

Run: `cd simse-code && bun run lint:fix && bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/setup.ts
git commit -m "feat: add connection testing to readline-based ollama setup"
```

---

## Task 10: Final Integration Test & Cleanup

**Files:**
- Modify: `simse-code/features/config/index.ts` — ensure all exports are correct
- Run all tests and lint

**Step 1: Run full test suite**

Run: `cd simse-code && bun test`
Expected: All tests pass

**Step 2: Run lint and typecheck**

Run: `cd simse-code && bun run lint && bun run typecheck`
Expected: PASS with no warnings

**Step 3: Manual end-to-end test**

1. Delete `~/.simse/` (back up first!)
2. Run `cd simse-code && bun run cli-ink.tsx`
3. Onboarding wizard should appear
4. Select Ollama → connection test → model selection
5. Complete all steps → config files written → normal prompt
6. Type `/settings` → settings explorer → edit a value → verify file updated
7. Type `/setup` → should show preset selector with ollama wizard
8. Restore `~/.simse/` backup

**Step 4: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final cleanup and integration fixes for onboarding and settings"
```
