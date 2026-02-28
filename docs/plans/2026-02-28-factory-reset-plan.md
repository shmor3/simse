# Factory Reset Commands Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `/factory-reset` and `/factory-reset-project` commands with a deliberate confirmation dialog (arrow-key list, "No" default, "Yes" requires typing `yes`).

**Architecture:** A reusable `<ConfirmDialog>` component follows the existing Promise-based modal pattern (like `SetupSelector`). Two commands in a new `reset.ts` factory call an `onConfirm` callback that shows the dialog and returns `Promise<boolean>`. `app-ink.tsx` wires the state + rendering.

**Tech Stack:** React/Ink, TypeScript, node:fs (`rmSync`), existing `TextInput` component

---

### Task 1: Create `<ConfirmDialog>` component

**Files:**
- Create: `simse-code/components/input/confirm-dialog.tsx`

**Step 1: Write the component**

```tsx
import { Box, Text, useInput } from 'ink';
import { useState } from 'react';
import { TextInput } from './text-input.js';

interface ConfirmDialogProps {
	readonly message: string;
	readonly onConfirm: () => void;
	readonly onCancel: () => void;
}

export function ConfirmDialog({
	message,
	onConfirm,
	onCancel,
}: ConfirmDialogProps) {
	const [selectedIndex, setSelectedIndex] = useState(0);
	const [confirmValue, setConfirmValue] = useState('');

	useInput(
		(_input, key) => {
			if (key.escape) {
				onCancel();
				return;
			}
			if (key.upArrow) {
				setSelectedIndex(0);
				setConfirmValue('');
				return;
			}
			if (key.downArrow) {
				setSelectedIndex(1);
				return;
			}
			if (key.return && selectedIndex === 0) {
				onCancel();
			}
		},
		{ isActive: selectedIndex === 0 },
	);

	return (
		<Box flexDirection="column" paddingLeft={2} marginY={1}>
			<Box>
				<Text color="red">{'⚠  '}</Text>
				<Text bold>{message}</Text>
			</Box>
			<Text> </Text>

			{/* Option 0: No (default) */}
			<Box>
				<Text color={selectedIndex === 0 ? 'cyan' : undefined}>
					{selectedIndex === 0 ? '  ❯ ' : '    '}
				</Text>
				<Text bold={selectedIndex === 0} color={selectedIndex === 0 ? 'cyan' : undefined}>
					No, cancel
				</Text>
			</Box>

			{/* Option 1: Yes */}
			<Box>
				<Text color={selectedIndex === 1 ? 'red' : undefined}>
					{selectedIndex === 1 ? '  ❯ ' : '    '}
				</Text>
				<Text bold={selectedIndex === 1} color={selectedIndex === 1 ? 'red' : undefined}>
					Yes, delete everything
				</Text>
			</Box>

			{/* Confirmation input - only when Yes is focused */}
			{selectedIndex === 1 && (
				<>
					<Text> </Text>
					<Box paddingLeft={4}>
						<Text dimColor>{'Type "yes" to confirm: '}</Text>
						<TextInput
							value={confirmValue}
							onChange={setConfirmValue}
							onSubmit={(val) => {
								if (val.trim().toLowerCase() === 'yes') {
									onConfirm();
								}
							}}
							placeholder="yes"
						/>
					</Box>
				</>
			)}

			<Text> </Text>
			<Text dimColor>{'  ↑↓ navigate  ↵ select  esc cancel'}</Text>
		</Box>
	);
}
```

**Step 2: Commit**

```bash
git add simse-code/components/input/confirm-dialog.tsx
git commit -m "feat(cli): add ConfirmDialog component with yes-type gate"
```

---

### Task 2: Create factory reset commands

**Files:**
- Create: `simse-code/features/config/reset.ts`
- Modify: `simse-code/features/config/index.ts`

**Step 1: Write the commands**

```typescript
import { rmSync } from 'node:fs';
import { join } from 'node:path';
import type { CommandDefinition } from '../../ink-types.js';

export function createResetCommands(
	dataDir: string,
	workDir: string,
	onConfirm: (message: string) => Promise<boolean>,
): readonly CommandDefinition[] {
	return [
		{
			name: 'factory-reset',
			usage: '/factory-reset',
			description: 'Delete all global configs, sessions, and memories',
			category: 'config',
			execute: async () => {
				const confirmed = await onConfirm(
					`This will permanently delete everything in ${dataDir}`,
				);
				if (!confirmed) {
					return { text: 'Factory reset cancelled.' };
				}
				rmSync(dataDir, { recursive: true, force: true });
				return {
					text: `Factory reset complete. Deleted ${dataDir}\nRestart simse to begin fresh.`,
				};
			},
		},
		{
			name: 'factory-reset-project',
			usage: '/factory-reset-project',
			description: 'Delete project-level .simse/ config and SIMSE.md',
			category: 'config',
			execute: async () => {
				const simseDir = join(workDir, '.simse');
				const simseMd = join(workDir, 'SIMSE.md');
				const confirmed = await onConfirm(
					`This will permanently delete ${simseDir} and ${simseMd}`,
				);
				if (!confirmed) {
					return { text: 'Project reset cancelled.' };
				}
				rmSync(simseDir, { recursive: true, force: true });
				rmSync(simseMd, { force: true });
				return {
					text: `Project reset complete. Deleted .simse/ and SIMSE.md`,
				};
			},
		},
	];
}
```

**Step 2: Update the barrel export in `simse-code/features/config/index.ts`**

Add this line:

```typescript
export { createResetCommands } from './reset.js';
```

**Step 3: Commit**

```bash
git add simse-code/features/config/reset.ts simse-code/features/config/index.ts
git commit -m "feat(cli): add /factory-reset and /factory-reset-project commands"
```

---

### Task 3: Wire ConfirmDialog and reset commands into app-ink.tsx

**Files:**
- Modify: `simse-code/app-ink.tsx`

**Step 1: Add imports** (near top of file, around line 36):

Add these imports:

```typescript
import { ConfirmDialog } from './components/input/confirm-dialog.js';
import { createResetCommands } from './features/config/reset.js';
```

**Step 2: Add pendingConfirm state** (after line 213, after `handleShowSetupSelector`):

```typescript
// Confirm dialog — Promise-based active-area dialog (factory reset, etc.)
const [pendingConfirm, setPendingConfirm] = useState<{
	message: string;
	resolve: (confirmed: boolean) => void;
} | null>(null);

const handleConfirm = useCallback(
	(message: string): Promise<boolean> => {
		return new Promise((resolve) => {
			setPendingConfirm({ message, resolve });
		});
	},
	[],
);
```

**Step 3: Register reset commands in registry useMemo** (after `reg.registerAll(aiCommands)` around line 299):

```typescript
reg.registerAll(createResetCommands(dataDir, process.cwd(), handleConfirm));
```

Add `handleConfirm` to the `useMemo` dependency array.

**Step 4: Update Escape key handler** (line 339). Change `!pendingSetup` to `!pendingSetup && !pendingConfirm`:

```typescript
if (key.escape && !pendingSetup && !pendingConfirm) {
```

**Step 5: Render ConfirmDialog** (after the `SetupSelector` block around line 544, before `pendingPermission`):

```tsx
{pendingConfirm && (
	<ConfirmDialog
		message={pendingConfirm.message}
		onConfirm={() => {
			pendingConfirm.resolve(true);
			setPendingConfirm(null);
		}}
		onCancel={() => {
			pendingConfirm.resolve(false);
			setPendingConfirm(null);
		}}
	/>
)}
```

**Step 6: Commit**

```bash
git add simse-code/app-ink.tsx
git commit -m "feat(cli): wire ConfirmDialog and reset commands into app"
```

---

### Task 4: Verify

**Step 1: Run typecheck**

Run: `npx tsc --noEmit --project simse-code/tsconfig.json 2>&1 | grep -E 'confirm-dialog|reset\.ts|app-ink'`
Expected: No new errors from changed files.

**Step 2: Run lint**

Run: `npx @biomejs/biome check simse-code/components/input/confirm-dialog.tsx simse-code/features/config/reset.ts simse-code/app-ink.tsx`
Expected: No errors (warnings OK if pre-existing).
