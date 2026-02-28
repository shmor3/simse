# Librarian Explorer & "Notes" → "Volumes" Rename — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an interactive `/librarians` command for managing librarian definitions, and rename all user-facing "notes" to "volumes" to match the library analogy.

**Architecture:** New `LibrarianExplorer` component following the `SettingsExplorer` modal pattern (Promise-based dialog in app-ink.tsx). Rename is a mechanical find-and-replace across simse-code.

**Tech Stack:** React/Ink, bun:test, TypeScript, JSON file I/O

---

### Task 1: Rename "notes" → "volumes" in library commands

**Files:**
- Modify: `simse-code/features/library/commands.ts`

**Step 1: Update command definitions**

Change the `/notes` command to `/volumes` and update all "note" references in descriptions:

```typescript
// Line 5-7: Change description
{
	name: 'add',
	usage: '/add <topic> <text>',
	description: 'Add a volume to a topic',
	// ...
}

// Line 53-61: Rename command
{
	name: 'volumes',
	aliases: ['ls'],
	usage: '/volumes [topic]',
	description: 'List volumes (optionally filtered by topic)',
	// ...
}

// Line 63-73: Change description
{
	name: 'get',
	usage: '/get <id>',
	description: 'Get a volume by ID',
	// ...
}

// Line 75-87: Change description
{
	name: 'delete',
	aliases: ['rm'],
	usage: '/delete <id>',
	description: 'Delete a volume by ID',
	// ...
}
```

**Step 2: Run tests**

Run: `bun test simse-code/tests/features-all.test.ts -v`
Expected: PASS (command names still unique, categories correct)

**Step 3: Commit**

```bash
git add simse-code/features/library/commands.ts
git commit -m "refactor: rename /notes to /volumes, update command descriptions"
```

---

### Task 2: Rename "notes" → "volumes" in UI components

**Files:**
- Modify: `simse-code/features/library/components.tsx`
- Modify: `simse-code/features/library/index.ts`

**Step 1: Rename NoteList to VolumeList**

In `components.tsx`:
- Rename `NoteListProps` → `VolumeListProps` (line 45)
- Rename `NoteList` → `VolumeList` (line 55)
- Change string `'No notes in'` → `'No volumes in'` (line 59)
- Change string `'No notes'` → `'No volumes'` (line 59)
- Change string `' note'` → `' volume'` (line 67)

In `index.ts`:
- Change `NoteList` → `VolumeList` in the export

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/features/library/components.tsx simse-code/features/library/index.ts
git commit -m "refactor: rename NoteList to VolumeList"
```

---

### Task 3: Rename "notes" → "volumes" in app.ts view types and interface

**Files:**
- Modify: `simse-code/app.ts`

**Step 1: Rename types and interface methods**

This is a large mechanical rename. Change these throughout the file:
- `NoteView` → `VolumeView` (type definition at line 34, all references)
- `SearchResultView.note` → `SearchResultView.volume` (line 43)
- `TopicView.noteCount` → `TopicView.volumeCount` (line 49)
- `storedNoteId` → `storedVolumeId` (line 75, and all internal uses)
- `addNote` → `addVolume` (line 153, implementation at 326, export at 710)
- `deleteNote` → `deleteVolume` (line 158, implementation at 336, export at 711)
- `getNote` → `getVolume` (line 159, implementation at 338, export at 712)
- `getAllNotes` → `getAllVolumes` (line 160, implementation at 343, export at 713)
- `getNotesByTopic` → `getVolumesByTopic` (line 172, implementation at 385, export at 717)
- `noteCount` → `volumeCount` (line 222, getter at 729)
- `toNoteView` → `toVolumeView` (line 229, all call sites)
- Comment `// Notes` → `// Volumes` (line 152)
- String `'existing notes'` → `'existing volumes'` (line 284)
- String `'oldest notes'` → `'oldest volumes'` (line 316)
- All `r.note.` → `r.volume.` in formatting strings (lines 416, 511)
- All `Object.freeze({ note: toNoteView(...)` → `Object.freeze({ volume: toVolumeView(...)` (lines 358, 372, 409, 504)
- Log strings with `noteId` → `volumeId` (lines 455, 551)

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: May show errors in files that import NoteView/use note property — those are fixed in next tasks.

**Step 3: Commit**

```bash
git add simse-code/app.ts
git commit -m "refactor: rename Note to Volume in app types and interface"
```

---

### Task 4: Rename "notes" → "volumes" in file-mentions, prompt tips, config, and tests

**Files:**
- Modify: `simse-code/file-mentions.ts` — rename `resolveNote` → `resolveVolume`, `completeNote` → `completeVolume`, `kind: 'note'` → `kind: 'volume'`, `isNoteIdPrefix` → `isVolumeIdPrefix`, `<note>` XML tags → `<volume>`, all related comments
- Modify: `simse-code/components/input/prompt-input.tsx` — update PLACEHOLDER_TIPS strings from "notes" to "volumes" (e.g., "add a note" → "add a volume")
- Modify: `simse-code/features/config/settings-schema.ts` — line 143: change `'Max notes per topic'` → `'Max volumes per topic'`
- Modify: `simse-code/tests/file-mentions.test.ts` — update all `resolveNote` → `resolveVolume`, `completeNote` → `completeVolume`, `kind: 'note'` → `kind: 'volume'`, `<note>` → `<volume>`, string literals like `'note content'` → `'volume content'`
- Modify: `simse-code/tests/tool-registry.test.ts` — line 149: change `'Stored note'` → `'Stored volume'`
- Modify: `simse-code/features/index.ts` — update any re-exported `NoteList` → `VolumeList`
- Any other files importing `NoteView` or `NoteList`

**Step 1: Do all renames**

Work through each file mechanically, replacing note→volume terminology.

**Step 2: Run full test suite and typecheck**

Run: `bun run typecheck && bun test simse-code/`
Expected: ALL PASS

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: complete notes-to-volumes rename across codebase"
```

---

### Task 5: Create LibrarianExplorer component — list panel

**Files:**
- Create: `simse-code/components/input/librarian-explorer.tsx`

**Step 1: Write the component with list panel**

Create a new component following the `SettingsExplorer` pattern. Start with just the list panel:

```typescript
import { Box, Text, useInput } from 'ink';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { useCallback, useEffect, useState } from 'react';
import { TextInput } from './text-input.js';

interface LibrarianEntry {
	readonly name: string;
	readonly description: string;
}

interface LibrarianExplorerProps {
	readonly librariansDir: string;
	readonly dataDir: string;
	readonly onDismiss: () => void;
}

type Panel = 'list' | 'detail';
type EditMode = 'none' | 'selecting' | 'text-input' | 'confirm-delete';

export function LibrarianExplorer({
	librariansDir,
	dataDir,
	onDismiss,
}: LibrarianExplorerProps) {
	const [panel, setPanel] = useState<Panel>('list');
	const [listIndex, setListIndex] = useState(0);
	const [librarians, setLibrarians] = useState<readonly LibrarianEntry[]>([]);

	// Load librarian definitions from disk
	const loadLibrarians = useCallback(() => {
		try {
			const files = readdirSync(librariansDir).filter((f) =>
				f.endsWith('.json'),
			);
			const entries: LibrarianEntry[] = [];
			for (const file of files) {
				try {
					const raw = readFileSync(join(librariansDir, file), 'utf-8');
					const def = JSON.parse(raw);
					entries.push({
						name: def.name ?? file.replace('.json', ''),
						description: def.description ?? '',
					});
				} catch {
					// Skip invalid files
				}
			}
			setLibrarians(Object.freeze(entries));
		} catch {
			setLibrarians([]);
		}
	}, [librariansDir]);

	useEffect(() => {
		loadLibrarians();
	}, [loadLibrarians]);

	// Items: librarians + "New librarian..." option
	const listItems = [...librarians, { name: '+ New librarian...', description: '' }];

	// List panel input
	useInput(
		(_input, key) => {
			if (key.escape) {
				onDismiss();
				return;
			}
			if (key.upArrow) {
				setListIndex((i) => Math.max(0, i - 1));
				return;
			}
			if (key.downArrow) {
				setListIndex((i) => Math.min(listItems.length - 1, i + 1));
				return;
			}
			if (key.return || key.rightArrow) {
				// Selected "New librarian..." or existing librarian
				setPanel('detail');
				return;
			}
		},
		{ isActive: panel === 'list' },
	);

	if (panel === 'list') {
		return (
			<Box flexDirection="column">
				<Box borderStyle="round" borderColor="cyan" paddingX={1}>
					<Text bold color="cyan">
						Librarians
					</Text>
				</Box>
				<Box flexDirection="column" paddingLeft={2}>
					{listItems.map((item, i) => (
						<Box key={item.name} gap={2}>
							<Text color={i === listIndex ? 'cyan' : undefined}>
								{i === listIndex ? '❯' : ' '}
							</Text>
							<Text
								bold={i === listIndex}
								color={i === listIndex ? 'cyan' : undefined}
								italic={item.name.startsWith('+')}
							>
								{item.name}
							</Text>
							{item.description && (
								<Text dimColor wrap="truncate-end">
									{item.description.slice(0, 40)}
								</Text>
							)}
						</Box>
					))}
				</Box>
				<Text dimColor> ↑↓ navigate ↵/→ select esc dismiss</Text>
			</Box>
		);
	}

	// Detail panel placeholder (implemented in next task)
	return (
		<Box flexDirection="column">
			<Text>Detail panel (TODO)</Text>
		</Box>
	);
}
```

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/components/input/librarian-explorer.tsx
git commit -m "feat: add LibrarianExplorer component with list panel"
```

---

### Task 6: Create LibrarianExplorer — detail panel with field rendering

**Files:**
- Modify: `simse-code/components/input/librarian-explorer.tsx`

**Step 1: Add detail panel state and field definitions**

Add the detail panel that loads a full `LibrarianDefinition` from disk and renders all fields as editable rows. Define the field schema inline:

```typescript
interface FieldDef {
	readonly key: string;
	readonly label: string;
	readonly path: readonly string[]; // e.g. ['permissions', 'add']
	readonly type: 'string' | 'boolean' | 'number' | 'action';
	readonly presets?: readonly string[];
	readonly resolve?: 'acp-commands';
}

const FIELD_DEFS: readonly FieldDef[] = [
	{ key: 'name', label: 'name', path: ['name'], type: 'string' },
	{ key: 'description', label: 'description', path: ['description'], type: 'string' },
	{ key: 'purpose', label: 'purpose', path: ['purpose'], type: 'string' },
	{ key: 'topics', label: 'topics', path: ['topics'], type: 'string' },
	{ key: 'permissions.add', label: '  add', path: ['permissions', 'add'], type: 'boolean' },
	{ key: 'permissions.delete', label: '  delete', path: ['permissions', 'delete'], type: 'boolean' },
	{ key: 'permissions.reorganize', label: '  reorganize', path: ['permissions', 'reorganize'], type: 'boolean' },
	{ key: 'thresholds.topicComplexity', label: '  topicComplexity', path: ['thresholds', 'topicComplexity'], type: 'number', presets: ['25', '50', '100', '200'] },
	{ key: 'thresholds.escalateAt', label: '  escalateAt', path: ['thresholds', 'escalateAt'], type: 'number', presets: ['100', '250', '500', '1000'] },
	{ key: 'acp.command', label: '  command', path: ['acp', 'command'], type: 'string', resolve: 'acp-commands' },
	{ key: 'acp.args', label: '  args', path: ['acp', 'args'], type: 'string' },
	{ key: 'acp.agentId', label: '  agentId', path: ['acp', 'agentId'], type: 'string' },
	{ key: '_delete', label: '⚠ Delete librarian', path: [], type: 'action' },
];
```

Implement:
- Load full definition JSON on detail panel enter
- Render each field with current value (use `getNestedValue(def, path)` helper)
- Show section headers ("permissions", "thresholds", "acp") as non-selectable dimmed text
- Navigate with up/down, press Enter to edit, escape/left to go back to list

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/components/input/librarian-explorer.tsx
git commit -m "feat: add LibrarianExplorer detail panel with field rendering"
```

---

### Task 7: Add field editing — dropdowns and text input

**Files:**
- Modify: `simse-code/components/input/librarian-explorer.tsx`

**Step 1: Implement edit modes**

Add dropdown and text-input editing following the settings-explorer pattern:

**Dropdown mode** (for booleans, number presets, resolved fields):
- Build options array based on field type:
  - Boolean: `['true', 'false']`
  - Number with presets: `[...presets, 'Custom value...']`
  - String with resolve: dynamically read from disk (e.g., ACP server commands from `acp.json`)
- Render inline below the selected field
- Up/down to navigate, Enter to select, Escape to cancel
- `'Custom value...'` switches to text-input mode
- `'(unset)'` clears the value

**Text-input mode** (for strings, custom values):
- Uses `TextInput` component
- Enter to save, Escape to cancel
- For `topics` field: accept comma-separated globs, store as array
- For `acp.args` field: accept comma-separated strings, store as array
- For `name` field: validate kebab-case (`/^[a-z0-9]+(-[a-z0-9]+)*$/`)

**Persistence:**
- After each edit, update the in-memory definition object
- Write to `{librariansDir}/{name}.json` with `JSON.stringify(def, null, '\t')`
- Show "Saved" indicator for 1.5 seconds (same pattern as settings-explorer)
- For name rename: delete old file, write new file

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/components/input/librarian-explorer.tsx
git commit -m "feat: add dropdown and text-input editing to LibrarianExplorer"
```

---

### Task 8: Add create and delete flows

**Files:**
- Modify: `simse-code/components/input/librarian-explorer.tsx`

**Step 1: Implement "New librarian" flow**

When `+ New librarian...` is selected and Enter pressed:
- Create a new definition with defaults:
  ```typescript
  const NEW_DEFAULTS = {
    name: '',
    description: '',
    purpose: '',
    topics: ['**'],
    permissions: { add: true, delete: true, reorganize: true },
    thresholds: { topicComplexity: 100, escalateAt: 500 },
  };
  ```
- Open detail panel in create mode (track via `isCreating` state)
- First edit to `name` creates the file on disk
- Back/escape returns to list and reloads

**Step 2: Implement delete flow**

When `⚠ Delete librarian` is selected:
- Switch to `confirm-delete` edit mode
- Render inline: `Are you sure? ▶ Yes  No`
- Yes: delete `{librariansDir}/{name}.json`, return to list
- No: cancel, stay in detail
- Hide delete option for `default` librarian

**Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-code/components/input/librarian-explorer.tsx
git commit -m "feat: add create and delete flows to LibrarianExplorer"
```

---

### Task 9: Wire LibrarianExplorer into app-ink.tsx

**Files:**
- Create: `simse-code/features/library/librarian-commands.ts`
- Modify: `simse-code/features/library/index.ts`
- Modify: `simse-code/app-ink.tsx`

**Step 1: Create the /librarians command**

In `simse-code/features/library/librarian-commands.ts`:

```typescript
import type { CommandDefinition } from '../../ink-types.js';

export function createLibrarianCommands(
	onShowLibrarianExplorer: () => Promise<void>,
): readonly CommandDefinition[] {
	return [
		{
			name: 'librarians',
			aliases: ['libs'],
			usage: '/librarians',
			description: 'Browse and manage librarians interactively',
			category: 'library',
			execute: async () => {
				await onShowLibrarianExplorer();
				return { text: '' };
			},
		},
	];
}
```

**Step 2: Export from library index**

Add to `simse-code/features/library/index.ts`:
```typescript
export { createLibrarianCommands } from './librarian-commands.js';
```

**Step 3: Wire into app-ink.tsx**

Follow the exact pattern used for SettingsExplorer:

1. Add import for `LibrarianExplorer` and `createLibrarianCommands`
2. Add `pendingLibrarians` state (same shape as `pendingSettings`)
3. Add `handleShowLibrarianExplorer` callback (same pattern)
4. Register commands: `reg.registerAll(createLibrarianCommands(handleShowLibrarianExplorer))`
5. Add conditional render block after the SettingsExplorer block:
   ```tsx
   {pendingLibrarians && (
     <LibrarianExplorer
       librariansDir={join(dataDir, 'librarians')}
       dataDir={dataDir}
       onDismiss={() => {
         pendingLibrarians.resolve();
         setPendingLibrarians(null);
       }}
     />
   )}
   ```
6. Add `pendingLibrarians` to the escape-key guard (alongside `pendingSetup`, `pendingSettings`, `pendingConfirm`)

**Step 4: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/features/library/librarian-commands.ts simse-code/features/library/index.ts simse-code/app-ink.tsx
git commit -m "feat: wire /librarians command and LibrarianExplorer modal"
```

---

### Task 10: Write tests for librarian commands

**Files:**
- Modify: `simse-code/tests/features-all.test.ts`

**Step 1: Add librarian commands to the all-features test**

Import and add to the duplicate-name check:

```typescript
import { createLibrarianCommands } from '../features/library/index.js';

const librarianCommands = createLibrarianCommands(async () => {});

// Add to existing tests:
test('librarian module exports commands with correct category', () => {
	expect(librarianCommands.length).toBeGreaterThan(0);
	for (const cmd of librarianCommands) expect(cmd.category).toBe('library');
});

// Update the no-duplicate-names test to include librarianCommands
test('no duplicate command names across modules', () => {
	const allNames = [
		...libraryCommands,
		...librarianCommands,
		...toolsCommands,
		// ... rest
	].map((c) => c.name);
	const unique = new Set(allNames);
	expect(unique.size).toBe(allNames.length);
});
```

**Step 2: Run tests**

Run: `bun test simse-code/tests/features-all.test.ts -v`
Expected: ALL PASS

**Step 3: Commit**

```bash
git add simse-code/tests/features-all.test.ts
git commit -m "test: add librarian commands to all-features test"
```

---

### Task 11: Run full test suite and lint

**Files:** None (verification only)

**Step 1: Run typecheck**

Run: `bun run typecheck`
Expected: PASS with no errors

**Step 2: Run all tests**

Run: `bun test`
Expected: ALL PASS

**Step 3: Run linter**

Run: `bun run lint`
Expected: PASS (or fix any formatting issues with `bun run lint:fix`)

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: lint and test fixes"
```

**Step 5: Push**

```bash
git push
```
