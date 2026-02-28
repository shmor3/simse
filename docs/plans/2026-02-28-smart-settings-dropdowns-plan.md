# Smart Settings Dropdowns Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Convert 10 plain text-input settings fields into populated dropdowns with dynamic data resolution and inline sub-flows.

**Architecture:** Extend `FieldSchema` with `presets` and `resolve` properties. Add three sync resolver functions to `settings-schema.ts` that read local JSON files. Modify `settings-explorer.tsx` to use resolvers when building dropdown options, and add a `'setup-flow'` edit mode that renders `SetupSelector` inline.

**Tech Stack:** TypeScript, React (Ink), Bun test runner, Biome linter

---

### Task 1: Extend FieldSchema with `presets` and `resolve`

**Files:**
- Modify: `simse-code/features/config/settings-schema.ts:16-24`
- Test: `simse-code/tests/settings-schema.test.ts`

**Step 1: Write failing tests for new schema properties**

Add to `simse-code/tests/settings-schema.test.ts`:

```typescript
describe('field presets and resolve', () => {
	it('should have presets on similarityThreshold', () => {
		const schema = getConfigSchema('memory.json')!;
		const field = schema.fields.find((f) => f.key === 'similarityThreshold')!;
		expect(field.presets).toBeDefined();
		expect(field.presets!.length).toBeGreaterThan(0);
	});

	it('should have presets on maxResults', () => {
		const schema = getConfigSchema('memory.json')!;
		const field = schema.fields.find((f) => f.key === 'maxResults')!;
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('10');
	});

	it('should have presets on autoSummarizeThreshold', () => {
		const schema = getConfigSchema('memory.json')!;
		const field = schema.fields.find((f) => f.key === 'autoSummarizeThreshold')!;
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('0');
	});

	it('should have presets on duplicateThreshold', () => {
		const schema = getConfigSchema('memory.json')!;
		const field = schema.fields.find((f) => f.key === 'duplicateThreshold')!;
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('0');
	});

	it('should have resolve on defaultServer in acp.json', () => {
		const schema = getConfigSchema('acp.json')!;
		const field = schema.fields.find((f) => f.key === 'defaultServer')!;
		expect(field.resolve).toBe('acp-servers');
	});

	it('should have resolve on defaultAgent in config.json', () => {
		const schema = getConfigSchema('config.json')!;
		const field = schema.fields.find((f) => f.key === 'defaultAgent')!;
		expect(field.resolve).toBe('agents');
	});

	it('should have resolve on embeddingModel in embed.json', () => {
		const schema = getConfigSchema('embed.json')!;
		const field = schema.fields.find((f) => f.key === 'embeddingModel')!;
		expect(field.resolve).toBe('embedding-models');
	});

	it('should have resolve on server in summarize.json', () => {
		const schema = getConfigSchema('summarize.json')!;
		const field = schema.fields.find((f) => f.key === 'server')!;
		expect(field.resolve).toBe('acp-servers');
	});

	it('should have resolve on agent in summarize.json', () => {
		const schema = getConfigSchema('summarize.json')!;
		const field = schema.fields.find((f) => f.key === 'agent')!;
		expect(field.resolve).toBe('agents');
	});

	it('should have resolve on defaultAgent in settings.json', () => {
		const schema = getConfigSchema('settings.json')!;
		const field = schema.fields.find((f) => f.key === 'defaultAgent')!;
		expect(field.resolve).toBe('agents');
	});

	it('should have resolve on defaultServer in settings.json', () => {
		const schema = getConfigSchema('settings.json')!;
		const field = schema.fields.find((f) => f.key === 'defaultServer')!;
		expect(field.resolve).toBe('acp-servers');
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test ./simse-code/tests/settings-schema.test.ts`
Expected: FAIL — `field.presets` and `field.resolve` are undefined

**Step 3: Add `presets` and `resolve` to FieldSchema type**

In `simse-code/features/config/settings-schema.ts`, change the `FieldSchema` interface (lines 18-24) to:

```typescript
export type ResolveType = 'acp-servers' | 'agents' | 'embedding-models';

export interface FieldSchema {
	readonly key: string;
	readonly type: FieldType;
	readonly description: string;
	readonly default?: unknown;
	readonly options?: readonly string[]; // for enum type
	readonly presets?: readonly string[]; // static preset values for dropdown
	readonly resolve?: ResolveType; // dynamic data source for dropdown
}
```

**Step 4: Add presets to number fields in memoryJsonSchema**

In `simse-code/features/config/settings-schema.ts`, update the memory.json field definitions (lines 112-137):

```typescript
Object.freeze({
	key: 'similarityThreshold',
	type: 'number' as FieldType,
	description: 'Similarity threshold for library search (0-1)',
	default: 0.7,
	presets: Object.freeze(['0.5', '0.6', '0.7', '0.8', '0.9']),
}),
Object.freeze({
	key: 'maxResults',
	type: 'number' as FieldType,
	description: 'Maximum library search results',
	default: 10,
	presets: Object.freeze(['5', '10', '20', '50']),
}),
Object.freeze({
	key: 'autoSummarizeThreshold',
	type: 'number' as FieldType,
	description:
		'Max notes per topic before auto-summarizing oldest entries (0 = disabled)',
	default: 20,
	presets: Object.freeze(['0', '10', '20', '50']),
}),
Object.freeze({
	key: 'duplicateThreshold',
	type: 'number' as FieldType,
	description:
		'Cosine similarity threshold for duplicate detection (0-1, 0 = disabled)',
	default: 0,
	presets: Object.freeze(['0', '0.8', '0.85', '0.9', '0.95']),
}),
```

**Step 5: Add `resolve` to all server/agent/model fields**

Update these field definitions across multiple schemas:

In `acpJsonSchema` (line 70), add `resolve`:
```typescript
Object.freeze({
	key: 'defaultServer',
	type: 'string' as FieldType,
	description: 'Default ACP server name',
	resolve: 'acp-servers' as ResolveType,
}),
```

In `configJsonSchema` `defaultAgent` (line 48), add `resolve`:
```typescript
Object.freeze({
	key: 'defaultAgent',
	type: 'string' as FieldType,
	description: 'Default agent ID for generation',
	resolve: 'agents' as ResolveType,
}),
```

In `embedJsonSchema` `embeddingModel` (line 82), add `resolve`:
```typescript
Object.freeze({
	key: 'embeddingModel',
	type: 'string' as FieldType,
	description: 'Hugging Face model ID for in-process embeddings',
	default: 'nomic-ai/nomic-embed-text-v1.5',
	resolve: 'embedding-models' as ResolveType,
}),
```

In `summarizeJsonSchema` `server` (line 153), add `resolve`:
```typescript
Object.freeze({
	key: 'server',
	type: 'string' as FieldType,
	description: 'ACP server name to use for summarization',
	resolve: 'acp-servers' as ResolveType,
}),
```

In `summarizeJsonSchema` `agent` (line 163), add `resolve`:
```typescript
Object.freeze({
	key: 'agent',
	type: 'string' as FieldType,
	description: 'Agent ID for the summarization ACP server',
	resolve: 'agents' as ResolveType,
}),
```

In `settingsJsonSchema` `defaultAgent` (line 175), add `resolve`:
```typescript
Object.freeze({
	key: 'defaultAgent',
	type: 'string' as FieldType,
	description: 'Default agent ID',
	resolve: 'agents' as ResolveType,
}),
```

In `settingsJsonSchema` `defaultServer` (line 191), add `resolve`:
```typescript
Object.freeze({
	key: 'defaultServer',
	type: 'string' as FieldType,
	description: 'ACP server name override',
	resolve: 'acp-servers' as ResolveType,
}),
```

**Step 6: Run tests to verify they pass**

Run: `bun test ./simse-code/tests/settings-schema.test.ts`
Expected: All pass

**Step 7: Run typecheck and lint**

Run: `bun run typecheck && bun x biome check ./simse-code/features/config/settings-schema.ts ./simse-code/tests/settings-schema.test.ts`
Expected: Clean

**Step 8: Commit**

```bash
git add simse-code/features/config/settings-schema.ts simse-code/tests/settings-schema.test.ts
git commit -m "feat(settings): add presets and resolve properties to FieldSchema"
```

---

### Task 2: Add resolver functions

**Files:**
- Modify: `simse-code/features/config/settings-schema.ts`
- Test: `simse-code/tests/settings-schema.test.ts`

**Step 1: Write failing tests for resolvers**

Add to `simse-code/tests/settings-schema.test.ts`:

```typescript
import {
	getAllConfigSchemas,
	getConfigSchema,
	loadConfigFile,
	resolveFieldOptions,
	saveConfigField,
} from '../features/config/settings-schema.js';
```

```typescript
describe('resolveFieldOptions', () => {
	let tempDir: string;

	beforeEach(() => {
		tempDir = makeTempDir();
	});

	afterEach(() => {
		rmSync(tempDir, { recursive: true, force: true });
	});

	it('should return acp server names from acp.json', () => {
		const acpData = {
			servers: [
				{ name: 'claude', command: 'bunx', args: ['claude-code-acp'] },
				{ name: 'ollama', command: 'bun', args: ['acp-ollama-bridge.ts'] },
			],
			defaultServer: 'claude',
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const options = resolveFieldOptions('acp-servers', tempDir);
		expect(options).toContain('claude');
		expect(options).toContain('ollama');
		expect(options).toContain('(unset)');
		expect(options).toContain('Add new server...');
		expect(options[options.length - 1]).toBe('Add new server...');
	});

	it('should return empty + Add new server when acp.json missing', () => {
		const options = resolveFieldOptions('acp-servers', tempDir);
		expect(options).toContain('(unset)');
		expect(options).toContain('Add new server...');
	});

	it('should return agent IDs from acp.json servers', () => {
		const acpData = {
			servers: [
				{ name: 'claude', command: 'bunx', defaultAgent: 'claude-agent' },
				{ name: 'ollama', command: 'bun' },
			],
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const options = resolveFieldOptions('agents', tempDir);
		expect(options).toContain('claude-agent');
		expect(options).toContain('ollama');
		expect(options).toContain('(unset)');
		expect(options).toContain('Custom value...');
	});

	it('should return agent IDs from .simse/agents/*.md files', () => {
		const agentsDir = join(tempDir, '.simse', 'agents');
		mkdirSync(agentsDir, { recursive: true });
		writeFileSync(join(agentsDir, 'researcher.md'), '# Researcher');
		writeFileSync(join(agentsDir, 'coder.md'), '# Coder');

		const options = resolveFieldOptions('agents', tempDir, tempDir);
		expect(options).toContain('researcher');
		expect(options).toContain('coder');
	});

	it('should return embedding model presets', () => {
		const options = resolveFieldOptions('embedding-models', tempDir);
		expect(options).toContain('Snowflake/snowflake-arctic-embed-xs');
		expect(options).toContain('nomic-ai/nomic-embed-text-v1.5');
		expect(options).toContain('Snowflake/snowflake-arctic-embed-l');
		expect(options).toContain('(unset)');
		expect(options).toContain('Custom model...');
	});

	it('should deduplicate agent names from servers and agent files', () => {
		const acpData = {
			servers: [{ name: 'researcher', command: 'bunx' }],
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const agentsDir = join(tempDir, '.simse', 'agents');
		mkdirSync(agentsDir, { recursive: true });
		writeFileSync(join(agentsDir, 'researcher.md'), '# Researcher');

		const options = resolveFieldOptions('agents', tempDir, tempDir);
		const researcherCount = options.filter((o) => o === 'researcher').length;
		expect(researcherCount).toBe(1);
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test ./simse-code/tests/settings-schema.test.ts`
Expected: FAIL — `resolveFieldOptions` does not exist

**Step 3: Implement resolveFieldOptions**

Add to `simse-code/features/config/settings-schema.ts`, after the existing imports, add `readdirSync` to the fs import:

```typescript
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
```

Then add the resolver function before the Public API section (before line ~227):

```typescript
// ---------------------------------------------------------------------------
// Dynamic option resolvers
// ---------------------------------------------------------------------------

const EMBEDDING_PRESETS: readonly string[] = Object.freeze([
	'Snowflake/snowflake-arctic-embed-xs',
	'nomic-ai/nomic-embed-text-v1.5',
	'Snowflake/snowflake-arctic-embed-l',
]);

/**
 * Resolves dynamic dropdown options for a field based on its resolve type.
 * Reads local JSON files and directories — no API calls.
 */
export function resolveFieldOptions(
	resolve: ResolveType,
	dataDir: string,
	workDir?: string,
): string[] {
	switch (resolve) {
		case 'acp-servers': {
			const acpData = loadConfigFile(dataDir, 'acp.json');
			const servers = Array.isArray(acpData.servers) ? acpData.servers : [];
			const names = servers
				.map((s: unknown) => {
					if (typeof s === 'object' && s !== null && 'name' in s) {
						return String((s as { name: unknown }).name);
					}
					return null;
				})
				.filter((n): n is string => n !== null);
			return [...names, '(unset)', 'Add new server...'];
		}
		case 'agents': {
			const acpData = loadConfigFile(dataDir, 'acp.json');
			const servers = Array.isArray(acpData.servers) ? acpData.servers : [];
			const agentIds = new Set<string>();

			// Derive agent IDs from ACP server entries
			for (const s of servers) {
				if (typeof s === 'object' && s !== null) {
					const entry = s as { name?: unknown; defaultAgent?: unknown };
					const id = entry.defaultAgent ?? entry.name;
					if (typeof id === 'string' && id) {
						agentIds.add(id);
					}
				}
			}

			// Scan .simse/agents/*.md in workDir
			const agentsDir = join(workDir ?? process.cwd(), '.simse', 'agents');
			if (existsSync(agentsDir)) {
				try {
					const files = readdirSync(agentsDir).filter((f) =>
						f.endsWith('.md'),
					);
					for (const file of files) {
						agentIds.add(file.replace(/\.md$/, ''));
					}
				} catch {
					// Ignore read errors
				}
			}

			return [...agentIds, '(unset)', 'Custom value...'];
		}
		case 'embedding-models': {
			return [...EMBEDDING_PRESETS, '(unset)', 'Custom model...'];
		}
	}
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test ./simse-code/tests/settings-schema.test.ts`
Expected: All pass

**Step 5: Run typecheck and lint**

Run: `bun run typecheck && bun x biome check ./simse-code/features/config/settings-schema.ts ./simse-code/tests/settings-schema.test.ts`
Expected: Clean

**Step 6: Commit**

```bash
git add simse-code/features/config/settings-schema.ts simse-code/tests/settings-schema.test.ts
git commit -m "feat(settings): add resolveFieldOptions for dynamic dropdowns"
```

---

### Task 3: Update settings explorer to use dynamic dropdowns

**Files:**
- Modify: `simse-code/components/input/settings-explorer.tsx`

This task modifies the settings explorer to use `presets` and `resolve` when building dropdown options, so that fields with dynamic data show populated dropdowns instead of text inputs.

**Step 1: Update imports**

In `simse-code/components/input/settings-explorer.tsx`, add `resolveFieldOptions` and `ResolveType` to the imports (lines 4-12):

```typescript
import {
	getAllConfigSchemas,
	loadConfigFile,
	resolveFieldOptions,
	saveConfigField,
} from '../../features/config/settings-schema.js';
import type {
	ConfigFileSchema,
	FieldSchema,
	ResolveType,
} from '../../features/config/settings-schema.js';
```

**Step 2: Modify buildDropdownOptions to handle presets and resolve**

Replace the `buildDropdownOptions` function (lines 63-71) with a version that accepts dataDir/workDir and checks for presets/resolve:

```typescript
/**
 * Builds the dropdown options list for a field.
 * Priority: resolve (dynamic) > presets (static) > enum options > boolean.
 * Returns empty array for plain text-input fields.
 */
function buildDropdownOptions(
	field: FieldSchema,
	dataDir: string,
	workDir?: string,
): readonly string[] {
	// Dynamic resolution from on-disk data
	if (field.resolve) {
		return resolveFieldOptions(field.resolve, dataDir, workDir);
	}
	// Static presets (number fields with common values)
	if (field.presets) {
		return [...field.presets, '(unset)', 'Custom value...'];
	}
	// Existing enum/boolean handling
	if (field.type === 'boolean') {
		return ['true', 'false', '(unset)'];
	}
	if (field.type === 'enum' && field.options) {
		return [...field.options, '(unset)', 'Custom value...'];
	}
	return [];
}
```

**Step 3: Update all callsites of buildDropdownOptions**

In the fields panel input handler (line 198), update the call to pass dataDir and workDir:

```typescript
const options = buildDropdownOptions(field, dataDir, workDir);
```

**Step 4: Update the enter-edit logic to prefer dropdown for presets/resolve fields**

In the fields panel input handler (lines 192-214), change the condition for opening a dropdown. Currently it only opens for `boolean` and `enum` types. Now it should also open for any field with `presets` or `resolve`:

```typescript
if (key.return) {
	const field = fields[fieldIndex];
	if (!field) return;

	const options = buildDropdownOptions(field, dataDir, workDir);
	if (options.length > 0) {
		// Open dropdown selector
		setDropdownOptions(options);
		setDropdownIndex(
			findCurrentIndex(fileData[field.key], options),
		);
		setEditMode('selecting');
		return;
	}
	// Fallback: plain text input for fields without dropdown options
	const current = fileData[field.key];
	setEditValue(
		current !== undefined && current !== null
			? String(current)
			: '',
	);
	setEditMode('text-input');
}
```

**Step 5: Update dropdown selection handler for number coercion**

In the dropdown selector input handler (lines 236-261), the `key.return` block needs to handle number coercion when a preset is selected for a number field. Update the save logic:

```typescript
if (key.return) {
	if (!selectedSchema) return;
	const field = selectedSchema.fields[fieldIndex];
	if (!field) return;

	const selected = dropdownOptions[dropdownIndex];
	if (
		selected === 'Custom value...' ||
		selected === 'Custom model...'
	) {
		// Switch to text input mode
		const current = fileData[field.key];
		setEditValue(
			current !== undefined && current !== null
				? String(current)
				: '',
		);
		setEditMode('text-input');
		return;
	}
	if (selected === 'Add new server...') {
		setEditMode('setup-flow');
		return;
	}
	if (selected === '(unset)') {
		saveField(selectedSchema, field.key, undefined);
	} else if (field.type === 'boolean') {
		saveField(selectedSchema, field.key, selected === 'true');
	} else if (field.type === 'number') {
		const num = Number(selected);
		if (!Number.isNaN(num)) {
			saveField(selectedSchema, field.key, num);
		}
	} else {
		saveField(selectedSchema, field.key, selected);
	}
	setEditMode('none');
}
```

**Step 6: Update the dropdown item rendering for "Custom model..." and "Add new server..."**

In the dropdown rendering JSX (lines 439-446), update the italic/dimColor conditions to also handle the new special options:

```typescript
dimColor={
	(opt === '(unset)') &&
	!isOptSelected
}
italic={
	opt === 'Custom value...' ||
	opt === 'Custom model...' ||
	opt === 'Add new server...'
}
```

**Step 7: Run typecheck and lint**

Run: `bun run typecheck && bun x biome check ./simse-code/components/input/settings-explorer.tsx`
Expected: Clean

**Step 8: Commit**

```bash
git add simse-code/components/input/settings-explorer.tsx
git commit -m "feat(settings): use dynamic dropdowns for server, agent, and model fields"
```

---

### Task 4: Add inline SetupSelector sub-flow

**Files:**
- Modify: `simse-code/components/input/settings-explorer.tsx`

This task adds the `'setup-flow'` edit mode to the settings explorer, rendering the SetupSelector inline when "Add new server..." is selected.

**Step 1: Update EditMode type and add imports**

In `simse-code/components/input/settings-explorer.tsx`:

Update the EditMode type (line 26):
```typescript
type EditMode = 'none' | 'selecting' | 'text-input' | 'setup-flow';
```

Add SetupSelector and setup-related imports:
```typescript
import type { SetupPresetOption } from './setup-selector.js';
import { SetupSelector } from './setup-selector.js';
```

**Step 2: Add the setup presets constant**

Add after the helper functions, before the component:

```typescript
const SETUP_PRESETS: readonly SetupPresetOption[] = Object.freeze([
	Object.freeze({
		key: 'claude-code',
		label: 'Claude Code',
		description: 'Anthropic Claude via claude-code-acp',
		needsInput: false,
	}),
	Object.freeze({
		key: 'ollama',
		label: 'Ollama',
		description: 'Local AI via Ollama + ACP bridge',
		needsInput: false,
	}),
	Object.freeze({
		key: 'copilot',
		label: 'GitHub Copilot',
		description: 'GitHub Copilot CLI',
		needsInput: false,
	}),
	Object.freeze({
		key: 'custom',
		label: 'Custom',
		description: 'Enter a custom ACP server command',
		needsInput: true,
	}),
]);
```

**Step 3: Add the setup-flow completion handler**

Inside the component, add a callback that writes the new server to `acp.json` and reloads:

```typescript
import { writeSetupFiles } from '../../features/config/setup.js';
```

Wait — `writeSetupFiles` is not exported from `setup.ts`. Instead, we need to import the setup presets' build function. Actually, the simplest approach is to use `saveConfigField` to update acp.json. But the setup flow creates a full server entry with command/args.

Better approach: import the setup PRESETS from setup.ts, or duplicate the build logic. Since setup.ts exports `createSetupCommands` but not the PRESETS directly, we should export them.

**Step 3a: Export PRESETS build functions from setup.ts**

In `simse-code/features/config/setup.ts`, export a helper function (add after the PRESETS definition, around line 104):

```typescript
/**
 * Builds an ACP server entry from a preset key and optional args.
 * Used by the settings explorer inline setup flow.
 */
export function buildPresetServer(
	presetKey: string,
	customArgs: string,
): ACPServerEntry | null {
	const preset = PRESETS[presetKey];
	if (!preset) return null;
	try {
		return preset.build(customArgs);
	} catch {
		return null;
	}
}

/**
 * Writes ACP setup files and returns the list of created filenames.
 */
export function writeSetupToDataDir(
	dataDir: string,
	server: ACPServerEntry,
): string[] {
	return writeSetupFiles(dataDir, server);
}
```

Also export the `ACPServerEntry` type by adding `export` before the interface (line 21):

```typescript
export interface ACPServerEntry {
```

**Step 3b: Add setup-flow completion handler in settings-explorer.tsx**

```typescript
import {
	buildPresetServer,
	writeSetupToDataDir,
} from '../../features/config/setup.js';
```

Inside the component, add the handler:

```typescript
const handleSetupComplete = useCallback(
	(selection: { presetKey: string; customArgs: string }) => {
		const server = buildPresetServer(
			selection.presetKey,
			selection.customArgs,
		);
		if (server) {
			writeSetupToDataDir(dataDir, server);
			// Reload the current file's data to pick up changes
			if (selectedSchema) {
				loadFile(selectedSchema);
			}
			// Auto-select the new server name as the field value
			if (selectedSchema) {
				const field = selectedSchema.fields[fieldIndex];
				if (field) {
					saveField(selectedSchema, field.key, server.name);
				}
			}
		}
		setEditMode('none');
	},
	[dataDir, selectedSchema, fieldIndex, loadFile, saveField],
);
```

**Step 4: Add SetupSelector rendering in the fields panel JSX**

In the fields panel render section, add a check for `editMode === 'setup-flow'` before the existing fields render. Insert before the `{fields.map(...)}` block:

```typescript
{editMode === 'setup-flow' && (
	<SetupSelector
		presets={[...SETUP_PRESETS]}
		onSelect={handleSetupComplete}
		onDismiss={() => setEditMode('none')}
	/>
)}
{editMode !== 'setup-flow' && (
	<>
		{fields.map((field, i) => {
			// ... existing field rendering
		})}
		<Text> </Text>
		{/* ... existing hint text */}
	</>
)}
```

This replaces the field list with the SetupSelector when in setup-flow mode, and shows the field list otherwise.

**Step 5: Run typecheck and lint**

Run: `bun run typecheck && bun x biome check ./simse-code/components/input/settings-explorer.tsx ./simse-code/features/config/setup.ts`
Expected: Clean

**Step 6: Run full test suite**

Run: `bun test`
Expected: All previously-passing tests still pass

**Step 7: Commit**

```bash
git add simse-code/components/input/settings-explorer.tsx simse-code/features/config/setup.ts
git commit -m "feat(settings): add inline SetupSelector for 'Add new server...' option"
```

---

### Task 5: Final verification and cleanup

**Files:**
- All modified files

**Step 1: Run full test suite**

Run: `bun test`
Expected: Same pass/fail count as before (1677 pass / 7 fail, all pre-existing E2E failures)

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: Clean exit

**Step 3: Run lint on all changed files**

Run: `bun x biome check ./simse-code/features/config/settings-schema.ts ./simse-code/components/input/settings-explorer.tsx ./simse-code/features/config/setup.ts ./simse-code/tests/settings-schema.test.ts`
Expected: Clean

**Step 4: Verify field count — only 7 text inputs remain**

Manually verify by reading `settings-schema.ts` that:
- All 4 number fields have `presets`
- All `defaultServer` fields have `resolve: 'acp-servers'`
- All `defaultAgent` fields have `resolve: 'agents'`
- `embeddingModel` has `resolve: 'embedding-models'`
- `summarize.json` `server` has `resolve: 'acp-servers'`
- `summarize.json` `agent` has `resolve: 'agents'`
- Only these 7 fields remain as plain text inputs: `perplexityApiKey`, `githubToken`, `teiUrl`, `command`, `systemPrompt`, `conversationTopic`, `chainTopic`
