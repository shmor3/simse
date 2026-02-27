# Ink CLI Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite simse-code's monolithic cli.ts (~4,300 lines) to use Ink (React for terminals) with feature module architecture, React context providers, and Claude Code-style bordered tool call UI.

**Architecture:** Feature module architecture where each module (library, ai, tools, session, files, config, meta) owns its commands, React components, and hooks. State management via React context providers (Theme, Services, Session, Input) replacing the monolithic AppContext. Ink's built-in TextInput for prompt input. `<Static>` for completed messages, active area for streaming, fixed StatusBar footer.

**Tech Stack:** Ink 5, React 18, ink-text-input, ink-spinner, ink-select-input, ink-testing-library, Bun, TypeScript with JSX.

---

### Task 1: Install Dependencies & Configure JSX

**Files:**
- Modify: `simse-code/package.json`
- Modify: `simse-code/tsconfig.json`

**Step 1: Install Ink dependencies**

Run:
```bash
cd simse-code && bun add ink ink-text-input ink-spinner ink-select-input react && bun add -d ink-testing-library @types/react
```

**Step 2: Update tsconfig.json for JSX**

Add `jsx` and `jsxImportSource` to `simse-code/tsconfig.json`:

```json
{
  "compilerOptions": {
    "lib": ["ESNext"],
    "target": "ESNext",
    "module": "ESNext",
    "moduleDetection": "force",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "noEmit": true,
    "strict": true,
    "skipLibCheck": true,
    "noFallthroughCasesInSwitch": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noPropertyAccessFromIndexSignature": false,
    "allowJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "jsx": "react-jsx",
    "jsxImportSource": "react",
    "types": ["bun-types", "@types/react"],
    "paths": {
      "simse": ["../src/lib.ts"]
    }
  },
  "include": ["./**/*", "../src/**/*"]
}
```

**Step 3: Verify TypeScript compiles**

Run: `cd simse-code && bun x tsc --noEmit`
Expected: No errors

**Step 4: Commit**

```bash
git add simse-code/package.json simse-code/tsconfig.json
git commit -m "chore: install Ink dependencies and configure JSX"
```

---

### Task 2: Create Shared Types

**Files:**
- Create: `simse-code/types.ts` (new shared types for the Ink CLI)

Note: `simse-code/types.ts` does not currently exist. The existing `app-context.ts` defines the old types (AppContext, Command, SessionState, etc.) which we'll eventually replace.

**Step 1: Write the shared types file**

```typescript
// simse-code/types.ts
/**
 * Shared types for the Ink-based CLI.
 */

import type { ReactNode } from 'react';

// ---------------------------------------------------------------------------
// Command System
// ---------------------------------------------------------------------------

export type CommandCategory =
	| 'ai'
	| 'library'
	| 'tools'
	| 'session'
	| 'files'
	| 'config'
	| 'meta';

export interface CommandResult {
	/** React element to render in the output area. */
	readonly element?: ReactNode;
	/** Plain text output (rendered as <Text>). */
	readonly text?: string;
}

export interface CommandDefinition {
	readonly name: string;
	readonly aliases?: readonly string[];
	readonly usage: string;
	readonly description: string;
	readonly category: CommandCategory;
	/** Execute the command. Return result to render or undefined for no output. */
	readonly execute: (args: string) => CommandResult | Promise<CommandResult> | undefined;
}

// ---------------------------------------------------------------------------
// Output Items (rendered in <Static> after completion)
// ---------------------------------------------------------------------------

export type OutputItem =
	| { readonly kind: 'message'; readonly role: 'user' | 'assistant'; readonly text: string }
	| { readonly kind: 'tool-call'; readonly name: string; readonly args: string; readonly status: 'completed' | 'failed'; readonly duration?: number; readonly summary?: string; readonly error?: string; readonly diff?: string }
	| { readonly kind: 'command-result'; readonly element: ReactNode }
	| { readonly kind: 'error'; readonly message: string }
	| { readonly kind: 'info'; readonly text: string };

// ---------------------------------------------------------------------------
// Tool Call State (for active area rendering)
// ---------------------------------------------------------------------------

export interface ToolCallState {
	readonly id: string;
	readonly name: string;
	readonly args: string;
	readonly status: 'active' | 'completed' | 'failed';
	readonly startedAt: number;
	readonly duration?: number;
	readonly summary?: string;
	readonly error?: string;
	readonly diff?: string;
}

// ---------------------------------------------------------------------------
// Permission Dialog
// ---------------------------------------------------------------------------

export interface PermissionRequest {
	readonly id: string;
	readonly toolName: string;
	readonly args: Record<string, unknown>;
	readonly options: readonly PermissionOption[];
}

export interface PermissionOption {
	readonly id: string;
	readonly label: string;
}

export type PermissionMode = 'default' | 'acceptEdits' | 'plan' | 'dontAsk';
```

**Step 2: Verify it compiles**

Run: `cd simse-code && bun x tsc --noEmit`
Expected: No errors

**Step 3: Commit**

```bash
git add simse-code/types.ts
git commit -m "feat(ink): add shared types for Ink CLI (CommandDefinition, OutputItem, ToolCallState)"
```

---

### Task 3: Create Theme Provider

**Files:**
- Create: `simse-code/providers/theme-provider.tsx`
- Create: `simse-code/tests/theme-provider.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/theme-provider.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { ThemeProvider, useTheme } from '../providers/theme-provider.js';

function TestConsumer() {
	const { colors } = useTheme();
	return <Text>{colors.enabled ? 'colors-on' : 'colors-off'}</Text>;
}

describe('ThemeProvider', () => {
	test('provides colors to children', () => {
		const { lastFrame } = render(
			<ThemeProvider>
				<TestConsumer />
			</ThemeProvider>,
		);
		// In test environment, colors are disabled (no TTY)
		expect(lastFrame()).toContain('colors-off');
	});

	test('throws when useTheme is used outside provider', () => {
		expect(() => render(<TestConsumer />)).toThrow();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/theme-provider.test.tsx`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```tsx
// simse-code/providers/theme-provider.tsx
import { createContext, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { createColors, createMarkdownRenderer } from '../ui.js';
import type { MarkdownRenderer, TermColors } from '../ui.js';

interface ThemeContextValue {
	readonly colors: TermColors;
	readonly md: MarkdownRenderer;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
	const ctx = useContext(ThemeContext);
	if (!ctx) throw new Error('useTheme must be used within a ThemeProvider');
	return ctx;
}

interface ThemeProviderProps {
	readonly children: ReactNode;
	readonly forceColors?: boolean;
}

export function ThemeProvider({ children, forceColors }: ThemeProviderProps) {
	const value = useMemo(() => {
		const colors = createColors({ enabled: forceColors });
		const md = createMarkdownRenderer(colors);
		return { colors, md } as const;
	}, [forceColors]);

	return (
		<ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/theme-provider.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/providers/theme-provider.tsx simse-code/tests/theme-provider.test.tsx
git commit -m "feat(ink): add ThemeProvider with useTheme hook"
```

---

### Task 4: Create Services Provider

**Files:**
- Create: `simse-code/providers/services-provider.tsx`
- Create: `simse-code/tests/services-provider.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/services-provider.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { ServicesProvider, useServices } from '../providers/services-provider.js';

function TestConsumer() {
	const { dataDir } = useServices();
	return <Text>{dataDir}</Text>;
}

describe('ServicesProvider', () => {
	test('provides services to children', () => {
		const services = {
			app: {} as any,
			acpClient: {} as any,
			vfs: {} as any,
			disk: {} as any,
			toolRegistry: {} as any,
			skillRegistry: {} as any,
			configResult: {} as any,
			dataDir: '/test/data',
		};

		const { lastFrame } = render(
			<ServicesProvider value={services}>
				<TestConsumer />
			</ServicesProvider>,
		);
		expect(lastFrame()).toContain('/test/data');
	});

	test('throws when useServices is used outside provider', () => {
		expect(() => render(<TestConsumer />)).toThrow();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/services-provider.test.tsx`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```tsx
// simse-code/providers/services-provider.tsx
import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import type { ACPClient, VFSDisk, VirtualFS } from 'simse';
import type { KnowledgeBaseApp } from '../app.js';
import type { CLIConfigResult } from '../config.js';
import type { SkillRegistry } from '../skills.js';
import type { ToolRegistry } from '../tool-registry.js';

export interface ServicesContextValue {
	readonly app: KnowledgeBaseApp;
	readonly acpClient: ACPClient;
	readonly vfs: VirtualFS;
	readonly disk: VFSDisk;
	readonly toolRegistry: ToolRegistry;
	readonly skillRegistry: SkillRegistry;
	readonly configResult: CLIConfigResult;
	readonly dataDir: string;
}

const ServicesContext = createContext<ServicesContextValue | null>(null);

export function useServices(): ServicesContextValue {
	const ctx = useContext(ServicesContext);
	if (!ctx) throw new Error('useServices must be used within a ServicesProvider');
	return ctx;
}

interface ServicesProviderProps {
	readonly value: ServicesContextValue;
	readonly children: ReactNode;
}

export function ServicesProvider({ value, children }: ServicesProviderProps) {
	return (
		<ServicesContext.Provider value={value}>
			{children}
		</ServicesContext.Provider>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/services-provider.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/providers/services-provider.tsx simse-code/tests/services-provider.test.tsx
git commit -m "feat(ink): add ServicesProvider with useServices hook"
```

---

### Task 5: Create Session Provider

**Files:**
- Create: `simse-code/providers/session-provider.tsx`
- Create: `simse-code/tests/session-provider.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/session-provider.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { SessionProvider, useSession } from '../providers/session-provider.js';

function TestConsumer() {
	const { serverName, permissionMode } = useSession();
	return <Text>{serverName ?? 'none'} {permissionMode}</Text>;
}

describe('SessionProvider', () => {
	test('provides session state to children', () => {
		const { lastFrame } = render(
			<SessionProvider initialServerName="test-server">
				<TestConsumer />
			</SessionProvider>,
		);
		expect(lastFrame()).toContain('test-server');
		expect(lastFrame()).toContain('default');
	});

	test('throws when useSession is used outside provider', () => {
		expect(() => render(<TestConsumer />)).toThrow();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/session-provider.test.tsx`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```tsx
// simse-code/providers/session-provider.tsx
import { createContext, useCallback, useContext, useState } from 'react';
import type { ReactNode } from 'react';
import type { PermissionMode } from '../types.js';
import type { Conversation } from '../conversation.js';

interface SessionContextValue {
	readonly serverName: string | undefined;
	readonly agentName: string | undefined;
	readonly libraryEnabled: boolean;
	readonly bypassPermissions: boolean;
	readonly maxTurns: number;
	readonly totalTurns: number;
	readonly permissionMode: PermissionMode;
	readonly planMode: boolean;
	readonly verbose: boolean;
	readonly conversation: Conversation | undefined;
	readonly abortController: AbortController | undefined;

	// Setters
	readonly setServerName: (name: string | undefined) => void;
	readonly setAgentName: (name: string | undefined) => void;
	readonly setLibraryEnabled: (enabled: boolean) => void;
	readonly setBypassPermissions: (bypass: boolean) => void;
	readonly setMaxTurns: (turns: number) => void;
	readonly incrementTurns: () => void;
	readonly setPermissionMode: (mode: PermissionMode) => void;
	readonly setPlanMode: (active: boolean) => void;
	readonly setVerbose: (verbose: boolean) => void;
	readonly setConversation: (conv: Conversation) => void;
	readonly setAbortController: (ctrl: AbortController | undefined) => void;
}

const SessionContext = createContext<SessionContextValue | null>(null);

export function useSession(): SessionContextValue {
	const ctx = useContext(SessionContext);
	if (!ctx) throw new Error('useSession must be used within a SessionProvider');
	return ctx;
}

interface SessionProviderProps {
	readonly children: ReactNode;
	readonly initialServerName?: string;
	readonly initialAgentName?: string;
	readonly initialLibraryEnabled?: boolean;
	readonly initialBypassPermissions?: boolean;
	readonly initialMaxTurns?: number;
	readonly initialConversation?: Conversation;
}

export function SessionProvider({
	children,
	initialServerName,
	initialAgentName,
	initialLibraryEnabled = true,
	initialBypassPermissions = false,
	initialMaxTurns = 10,
	initialConversation,
}: SessionProviderProps) {
	const [serverName, setServerName] = useState(initialServerName);
	const [agentName, setAgentName] = useState(initialAgentName);
	const [libraryEnabled, setLibraryEnabled] = useState(initialLibraryEnabled);
	const [bypassPermissions, setBypassPermissions] = useState(initialBypassPermissions);
	const [maxTurns, setMaxTurns] = useState(initialMaxTurns);
	const [totalTurns, setTotalTurns] = useState(0);
	const [permissionMode, setPermissionMode] = useState<PermissionMode>('default');
	const [planMode, setPlanMode] = useState(false);
	const [verbose, setVerbose] = useState(false);
	const [conversation, setConversation] = useState<Conversation | undefined>(initialConversation);
	const [abortController, setAbortController] = useState<AbortController | undefined>();

	const incrementTurns = useCallback(() => setTotalTurns((t) => t + 1), []);

	const value: SessionContextValue = {
		serverName, agentName, libraryEnabled, bypassPermissions,
		maxTurns, totalTurns, permissionMode, planMode, verbose,
		conversation, abortController,
		setServerName, setAgentName, setLibraryEnabled, setBypassPermissions,
		setMaxTurns, incrementTurns, setPermissionMode, setPlanMode,
		setVerbose, setConversation, setAbortController,
	};

	return (
		<SessionContext.Provider value={value}>{children}</SessionContext.Provider>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/session-provider.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/providers/session-provider.tsx simse-code/tests/session-provider.test.tsx
git commit -m "feat(ink): add SessionProvider with useSession hook"
```

---

### Task 6: Create Shared Components (Spinner, ErrorBox, Badge)

**Files:**
- Create: `simse-code/components/shared/spinner.tsx`
- Create: `simse-code/components/shared/error-box.tsx`
- Create: `simse-code/components/shared/badge.tsx`
- Create: `simse-code/tests/shared-components.test.tsx`

**Step 1: Write the failing tests**

```typescript
// simse-code/tests/shared-components.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { ThinkingSpinner } from '../components/shared/spinner.js';
import { ErrorBox } from '../components/shared/error-box.js';
import { Badge } from '../components/shared/badge.js';

describe('ThinkingSpinner', () => {
	test('renders with default label', () => {
		const { lastFrame } = render(<ThinkingSpinner />);
		expect(lastFrame()).toBeDefined();
	});

	test('renders with custom label', () => {
		const { lastFrame } = render(<ThinkingSpinner label="Searching..." />);
		expect(lastFrame()).toContain('Searching...');
	});
});

describe('ErrorBox', () => {
	test('renders error message', () => {
		const { lastFrame } = render(<ErrorBox message="Something went wrong" />);
		expect(lastFrame()).toContain('Something went wrong');
	});
});

describe('Badge', () => {
	test('renders PLAN badge', () => {
		const { lastFrame } = render(<Badge label="PLAN" />);
		expect(lastFrame()).toContain('PLAN');
	});

	test('renders VERBOSE badge', () => {
		const { lastFrame } = render(<Badge label="VERBOSE" />);
		expect(lastFrame()).toContain('VERBOSE');
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-code && bun test tests/shared-components.test.tsx`
Expected: FAIL — modules not found

**Step 3: Write implementations**

```tsx
// simse-code/components/shared/spinner.tsx
import InkSpinner from 'ink-spinner';
import { Box, Text } from 'ink';
import React from 'react';

interface ThinkingSpinnerProps {
	readonly label?: string;
	readonly tokens?: number;
	readonly server?: string;
	readonly elapsed?: number;
}

export function ThinkingSpinner({
	label = 'Thinking',
	tokens,
	server,
	elapsed,
}: ThinkingSpinnerProps) {
	const parts: string[] = [];
	if (elapsed !== undefined) parts.push(`${(elapsed / 1000).toFixed(1)}s`);
	if (tokens !== undefined) parts.push(`↓ ${tokens}`);
	if (server) parts.push(server);

	const suffix = parts.length > 0 ? ` (${parts.join(' · ')})` : '';

	return (
		<Box>
			<Text color="cyan">
				<InkSpinner type="dots" />
			</Text>
			<Text dimColor> {label}{suffix}</Text>
		</Box>
	);
}
```

```tsx
// simse-code/components/shared/error-box.tsx
import { Box, Text } from 'ink';
import React from 'react';

interface ErrorBoxProps {
	readonly message: string;
}

export function ErrorBox({ message }: ErrorBoxProps) {
	return (
		<Box paddingLeft={2}>
			<Text color="red">● {message}</Text>
		</Box>
	);
}
```

```tsx
// simse-code/components/shared/badge.tsx
import { Text } from 'ink';
import React from 'react';

interface BadgeProps {
	readonly label: string;
	readonly color?: string;
}

const BADGE_COLORS: Record<string, string> = {
	PLAN: 'blue',
	VERBOSE: 'yellow',
	YOLO: 'red',
	'AUTO-EDIT': 'green',
};

export function Badge({ label, color }: BadgeProps) {
	const resolvedColor = color ?? BADGE_COLORS[label] ?? 'gray';
	return (
		<Text color={resolvedColor} bold>
			[{label}]
		</Text>
	);
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-code && bun test tests/shared-components.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/shared/ simse-code/tests/shared-components.test.tsx
git commit -m "feat(ink): add shared components (ThinkingSpinner, ErrorBox, Badge)"
```

---

### Task 7: Create ToolCallBox Component

**Files:**
- Create: `simse-code/components/chat/tool-call-box.tsx`
- Create: `simse-code/tests/tool-call-box.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/tool-call-box.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { ToolCallBox } from '../components/chat/tool-call-box.js';

describe('ToolCallBox', () => {
	test('renders active tool call with spinner', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/main.ts"}'
				status="active"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_read');
		expect(frame).toContain('/src/main.ts');
	});

	test('renders completed tool call with check and duration', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_read"
				args='{"path": "/src/main.ts"}'
				status="completed"
				duration={125}
				summary="150 lines"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_read');
		expect(frame).toContain('125ms');
		expect(frame).toContain('150 lines');
	});

	test('renders failed tool call with error', () => {
		const { lastFrame } = render(
			<ToolCallBox
				name="vfs_write"
				args='{"path": "/src/main.ts"}'
				status="failed"
				error="Permission denied"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_write');
		expect(frame).toContain('Permission denied');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/tool-call-box.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/chat/tool-call-box.tsx
import InkSpinner from 'ink-spinner';
import { Box, Text } from 'ink';
import React from 'react';

interface ToolCallBoxProps {
	readonly name: string;
	readonly args: string;
	readonly status: 'active' | 'completed' | 'failed';
	readonly duration?: number;
	readonly summary?: string;
	readonly error?: string;
	readonly diff?: string;
}

function formatArgs(argsStr: string): string {
	try {
		const parsed = JSON.parse(argsStr);
		if (typeof parsed === 'object' && parsed !== null) {
			return Object.entries(parsed)
				.map(([k, v]) => `${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`)
				.join(', ');
		}
	} catch {
		// fallback to raw string
	}
	return argsStr.length > 200 ? `${argsStr.slice(0, 200)}...` : argsStr;
}

function StatusIcon({ status }: { status: ToolCallBoxProps['status'] }) {
	switch (status) {
		case 'active':
			return (
				<Text color="cyan">
					<InkSpinner type="dots" />
				</Text>
			);
		case 'completed':
			return <Text color="green">✓</Text>;
		case 'failed':
			return <Text color="red">✗</Text>;
	}
}

function borderColor(status: ToolCallBoxProps['status']): string {
	switch (status) {
		case 'active':
			return 'cyan';
		case 'completed':
			return 'green';
		case 'failed':
			return 'red';
	}
}

export function ToolCallBox({
	name,
	args,
	status,
	duration,
	summary,
	error,
	diff,
}: ToolCallBoxProps) {
	const meta: string[] = [];
	if (duration !== undefined) meta.push(`${duration}ms`);
	if (summary) meta.push(summary);

	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={borderColor(status)}
			paddingX={1}
			marginLeft={2}
		>
			{/* Header: icon + tool name + meta */}
			<Box gap={1}>
				<StatusIcon status={status} />
				<Text bold>{name}</Text>
				{meta.length > 0 && <Text dimColor>({meta.join(', ')})</Text>}
			</Box>

			{/* Args */}
			<Text dimColor>{formatArgs(args)}</Text>

			{/* Diff (if present) */}
			{diff && (
				<Box marginTop={1}>
					<Text>{diff}</Text>
				</Box>
			)}

			{/* Error (if failed) */}
			{error && (
				<Box marginTop={1}>
					<Text color="red">{error}</Text>
				</Box>
			)}
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/tool-call-box.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/chat/tool-call-box.tsx simse-code/tests/tool-call-box.test.tsx
git commit -m "feat(ink): add ToolCallBox component with active/completed/failed states"
```

---

### Task 8: Create InlineDiff Component

**Files:**
- Create: `simse-code/components/chat/inline-diff.tsx`
- Create: `simse-code/tests/inline-diff.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/inline-diff.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { InlineDiff } from '../components/chat/inline-diff.js';

describe('InlineDiff', () => {
	test('renders additions in green and removals in red', () => {
		const lines = [
			{ type: 'context' as const, content: 'const x = 1;', oldLineNumber: 1, newLineNumber: 1 },
			{ type: 'remove' as const, content: 'const y = 2;', oldLineNumber: 2 },
			{ type: 'add' as const, content: 'const y = 3;', newLineNumber: 2 },
		];

		const { lastFrame } = render(
			<InlineDiff path="/src/main.ts" lines={lines} />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('/src/main.ts');
		expect(frame).toContain('const y = 2;');
		expect(frame).toContain('const y = 3;');
	});

	test('renders empty diff gracefully', () => {
		const { lastFrame } = render(
			<InlineDiff path="/src/main.ts" lines={[]} />,
		);
		expect(lastFrame()).toContain('No changes');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/inline-diff.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/chat/inline-diff.tsx
import { Box, Text } from 'ink';
import React from 'react';

export interface DiffLine {
	readonly type: 'add' | 'remove' | 'context';
	readonly content: string;
	readonly oldLineNumber?: number;
	readonly newLineNumber?: number;
}

interface InlineDiffProps {
	readonly path: string;
	readonly lines: readonly DiffLine[];
	readonly maxLines?: number;
}

function lineColor(type: DiffLine['type']): string | undefined {
	switch (type) {
		case 'add':
			return 'green';
		case 'remove':
			return 'red';
		default:
			return undefined;
	}
}

function linePrefix(type: DiffLine['type']): string {
	switch (type) {
		case 'add':
			return '+';
		case 'remove':
			return '-';
		default:
			return ' ';
	}
}

export function InlineDiff({ path, lines, maxLines = 50 }: InlineDiffProps) {
	if (lines.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>{path}: No changes</Text>
			</Box>
		);
	}

	const visible = lines.slice(0, maxLines);
	const truncated = lines.length > maxLines ? lines.length - maxLines : 0;

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold dimColor>{path}</Text>
			{visible.map((line, i) => (
				<Text key={i} color={lineColor(line.type)}>
					{linePrefix(line.type)} {line.content}
				</Text>
			))}
			{truncated > 0 && (
				<Text dimColor>  ... {truncated} more lines</Text>
			)}
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/inline-diff.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/chat/inline-diff.tsx simse-code/tests/inline-diff.test.tsx
git commit -m "feat(ink): add InlineDiff component for unified diff rendering"
```

---

### Task 9: Create StreamingText Component

**Files:**
- Create: `simse-code/components/chat/streaming-text.tsx`
- Create: `simse-code/tests/streaming-text.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/streaming-text.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { StreamingText } from '../components/chat/streaming-text.js';

describe('StreamingText', () => {
	test('renders accumulated text', () => {
		const { lastFrame } = render(
			<StreamingText text="Hello, world!" />,
		);
		expect(lastFrame()).toContain('Hello, world!');
	});

	test('renders empty text without crashing', () => {
		const { lastFrame } = render(
			<StreamingText text="" />,
		);
		expect(lastFrame()).toBeDefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/streaming-text.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/chat/streaming-text.tsx
import { Text } from 'ink';
import React from 'react';

interface StreamingTextProps {
	readonly text: string;
}

export function StreamingText({ text }: StreamingTextProps) {
	if (!text) return null;
	return <Text>{text}</Text>;
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/streaming-text.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/chat/streaming-text.tsx simse-code/tests/streaming-text.test.tsx
git commit -m "feat(ink): add StreamingText component for live token display"
```

---

### Task 10: Create MessageList Component (uses `<Static>`)

**Files:**
- Create: `simse-code/components/chat/message-list.tsx`
- Create: `simse-code/tests/message-list.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/message-list.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { MessageList } from '../components/chat/message-list.js';
import type { OutputItem } from '../types.js';

describe('MessageList', () => {
	test('renders user and assistant messages', () => {
		const items: OutputItem[] = [
			{ kind: 'message', role: 'user', text: 'Hello' },
			{ kind: 'message', role: 'assistant', text: 'Hi there!' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		const frame = lastFrame()!;
		expect(frame).toContain('Hello');
		expect(frame).toContain('Hi there!');
	});

	test('renders error items', () => {
		const items: OutputItem[] = [
			{ kind: 'error', message: 'Something broke' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Something broke');
	});

	test('renders info items', () => {
		const items: OutputItem[] = [
			{ kind: 'info', text: 'Library enabled' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Library enabled');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/message-list.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/chat/message-list.tsx
import { Box, Static, Text } from 'ink';
import React from 'react';
import type { OutputItem } from '../../types.js';
import { ErrorBox } from '../shared/error-box.js';
import { ToolCallBox } from './tool-call-box.js';

interface MessageListProps {
	readonly items: readonly OutputItem[];
}

function renderItem(item: OutputItem, index: number): React.ReactNode {
	switch (item.kind) {
		case 'message':
			return (
				<Box key={index} paddingLeft={item.role === 'user' ? 0 : 2}>
					<Text
						bold={item.role === 'user'}
						color={item.role === 'user' ? 'white' : undefined}
					>
						{item.text}
					</Text>
				</Box>
			);
		case 'tool-call':
			return (
				<ToolCallBox
					key={index}
					name={item.name}
					args={item.args}
					status={item.status}
					duration={item.duration}
					summary={item.summary}
					error={item.error}
					diff={item.diff}
				/>
			);
		case 'command-result':
			return <Box key={index}>{item.element}</Box>;
		case 'error':
			return <ErrorBox key={index} message={item.message} />;
		case 'info':
			return (
				<Box key={index} paddingLeft={2}>
					<Text dimColor>{item.text}</Text>
				</Box>
			);
	}
}

export function MessageList({ items }: MessageListProps) {
	return (
		<Static items={items.map((item, i) => ({ ...item, key: i }))}>
			{(item, index) => renderItem(item as OutputItem, index)}
		</Static>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/message-list.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/chat/message-list.tsx simse-code/tests/message-list.test.tsx
git commit -m "feat(ink): add MessageList component with Static rendering"
```

---

### Task 11: Create PermissionDialog Component

**Files:**
- Create: `simse-code/components/input/permission-dialog.tsx`
- Create: `simse-code/tests/permission-dialog.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/permission-dialog.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { PermissionDialog } from '../components/input/permission-dialog.js';

describe('PermissionDialog', () => {
	test('renders tool name and args', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="vfs_write"
				args={{ path: '/src/main.ts' }}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('vfs_write');
		expect(frame).toContain('/src/main.ts');
	});

	test('shows action keys', () => {
		const { lastFrame } = render(
			<PermissionDialog
				toolName="vfs_write"
				args={{}}
				onAllow={() => {}}
				onDeny={() => {}}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Allow');
		expect(frame).toContain('Deny');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/permission-dialog.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/input/permission-dialog.tsx
import { Box, Text, useInput } from 'ink';
import React from 'react';

interface PermissionDialogProps {
	readonly toolName: string;
	readonly args: Record<string, unknown>;
	readonly onAllow: () => void;
	readonly onDeny: () => void;
	readonly onAllowAlways?: () => void;
}

export function PermissionDialog({
	toolName,
	args,
	onAllow,
	onDeny,
	onAllowAlways,
}: PermissionDialogProps) {
	useInput((input) => {
		if (input === 'y') onAllow();
		else if (input === 'n') onDeny();
		else if (input === 'a' && onAllowAlways) onAllowAlways();
	});

	const argsStr = JSON.stringify(args, null, 2);
	const truncated =
		argsStr.length > 500 ? `${argsStr.slice(0, 500)}...` : argsStr;

	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor="yellow"
			paddingX={1}
			marginLeft={2}
		>
			<Text bold color="yellow">
				⚠ Permission requested
			</Text>
			<Box marginTop={1}>
				<Text>
					Allow <Text bold>{toolName}</Text>?
				</Text>
			</Box>
			<Box marginTop={1}>
				<Text dimColor>{truncated}</Text>
			</Box>
			<Box marginTop={1} gap={2}>
				<Text color="green">[y] Allow</Text>
				<Text color="red">[n] Deny</Text>
				{onAllowAlways && <Text color="blue">[a] Always</Text>}
			</Box>
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/permission-dialog.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/input/permission-dialog.tsx simse-code/tests/permission-dialog.test.tsx
git commit -m "feat(ink): add PermissionDialog component with keyboard input"
```

---

### Task 12: Create StatusBar Component

**Files:**
- Create: `simse-code/components/layout/status-bar.tsx`
- Create: `simse-code/tests/status-bar.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/status-bar.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { StatusBar } from '../components/layout/status-bar.js';

describe('StatusBar', () => {
	test('renders server and model', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={1234}
				cost="$0.03"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('claude');
		expect(frame).toContain('opus-4');
	});

	test('renders token count', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={1234}
				cost="$0.03"
			/>,
		);
		expect(lastFrame()).toContain('1234');
	});

	test('renders badges when modes active', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={0}
				cost="$0.00"
				planMode
				verbose
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('PLAN');
		expect(frame).toContain('VERBOSE');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/status-bar.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/layout/status-bar.tsx
import { Box, Text } from 'ink';
import React from 'react';
import { Badge } from '../shared/badge.js';

interface StatusBarProps {
	readonly server?: string;
	readonly model?: string;
	readonly tokens?: number;
	readonly cost?: string;
	readonly additions?: number;
	readonly deletions?: number;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
	readonly permissionMode?: string;
}

export function StatusBar({
	server,
	model,
	tokens = 0,
	cost,
	additions,
	deletions,
	planMode,
	verbose,
	permissionMode,
}: StatusBarProps) {
	const parts: string[] = [];
	if (server && model) parts.push(`${server}:${model}`);
	else if (server) parts.push(server);
	if (tokens > 0) parts.push(`${tokens} tokens`);
	if (cost) parts.push(cost);

	const changes: string[] = [];
	if (additions && additions > 0) changes.push(`+${additions}`);
	if (deletions && deletions > 0) changes.push(`-${deletions}`);

	return (
		<Box borderStyle="single" borderTop borderBottom={false} borderLeft={false} borderRight={false} paddingX={1}>
			<Box flexGrow={1} gap={1}>
				<Text dimColor>{parts.join(' · ')}</Text>
				{changes.length > 0 && (
					<Text>
						<Text color="green">{changes[0]}</Text>
						{changes[1] && <Text color="red"> {changes[1]}</Text>}
					</Text>
				)}
			</Box>
			<Box gap={1}>
				{planMode && <Badge label="PLAN" />}
				{verbose && <Badge label="VERBOSE" />}
				{permissionMode === 'dontAsk' && <Badge label="YOLO" />}
			</Box>
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/status-bar.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/layout/status-bar.tsx simse-code/tests/status-bar.test.tsx
git commit -m "feat(ink): add StatusBar component with server/model/tokens/badges"
```

---

### Task 13: Create PromptInput Component

**Files:**
- Create: `simse-code/components/input/prompt-input.tsx`
- Create: `simse-code/tests/prompt-input.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/prompt-input.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { PromptInput } from '../components/input/prompt-input.js';

describe('PromptInput', () => {
	test('renders prompt character', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} />,
		);
		expect(lastFrame()).toContain('>');
	});

	test('shows plan mode badge when active', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} planMode />,
		);
		expect(lastFrame()).toContain('PLAN');
	});

	test('disables input when disabled prop is set', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} disabled />,
		);
		// When disabled, the input area should still render
		expect(lastFrame()).toBeDefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/prompt-input.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/input/prompt-input.tsx
import TextInput from 'ink-text-input';
import { Box, Text } from 'ink';
import React, { useState } from 'react';
import { Badge } from '../shared/badge.js';

interface PromptInputProps {
	readonly onSubmit: (value: string) => void;
	readonly disabled?: boolean;
	readonly planMode?: boolean;
	readonly verbose?: boolean;
}

export function PromptInput({
	onSubmit,
	disabled = false,
	planMode,
	verbose,
}: PromptInputProps) {
	const [value, setValue] = useState('');

	const handleSubmit = (input: string) => {
		if (!input.trim()) return;
		onSubmit(input);
		setValue('');
	};

	return (
		<Box gap={1}>
			{planMode && <Badge label="PLAN" />}
			{verbose && <Badge label="VERBOSE" />}
			<Text bold color="cyan">
				{'>'}
			</Text>
			{!disabled && (
				<TextInput
					value={value}
					onChange={setValue}
					onSubmit={handleSubmit}
				/>
			)}
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/prompt-input.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/input/prompt-input.tsx simse-code/tests/prompt-input.test.tsx
git commit -m "feat(ink): add PromptInput component with TextInput and badges"
```

---

### Task 14: Create Banner Component

**Files:**
- Create: `simse-code/components/layout/banner.tsx`
- Create: `simse-code/tests/banner.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/banner.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Banner } from '../components/layout/banner.js';

describe('Banner', () => {
	test('renders app name', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('simse');
	});

	test('renders server name', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
			/>,
		);
		expect(lastFrame()).toContain('claude');
	});

	test('renders service counts', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
				noteCount={42}
				toolCount={7}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('42');
		expect(frame).toContain('7');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/banner.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/layout/banner.tsx
import { Box, Text } from 'ink';
import React from 'react';

interface BannerProps {
	readonly version: string;
	readonly workDir: string;
	readonly dataDir: string;
	readonly server?: string;
	readonly model?: string;
	readonly noteCount?: number;
	readonly toolCount?: number;
	readonly agentCount?: number;
}

export function Banner({
	version,
	workDir,
	dataDir,
	server,
	model,
	noteCount,
	toolCount,
	agentCount,
}: BannerProps) {
	return (
		<Box flexDirection="column" paddingX={1} marginBottom={1}>
			<Text bold color="cyan">
				simse <Text dimColor>v{version}</Text>
			</Text>

			<Box marginTop={1} flexDirection="column">
				<Text dimColor>  workDir  {workDir}</Text>
				<Text dimColor>  dataDir  {dataDir}</Text>
				{server && (
					<Text dimColor>
						  server   {server}
						{model ? ` (${model})` : ''}
					</Text>
				)}
			</Box>

			{(noteCount !== undefined || toolCount !== undefined) && (
				<Box marginTop={1} gap={2}>
					{noteCount !== undefined && (
						<Text dimColor>{noteCount} notes</Text>
					)}
					{toolCount !== undefined && (
						<Text dimColor>{toolCount} tools</Text>
					)}
					{agentCount !== undefined && (
						<Text dimColor>{agentCount} agents</Text>
					)}
				</Box>
			)}
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/banner.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/layout/banner.tsx simse-code/tests/banner.test.tsx
git commit -m "feat(ink): add Banner component with service status display"
```

---

### Task 15: Create Command Registry

**Files:**
- Create: `simse-code/command-registry.ts`
- Create: `simse-code/tests/command-registry.test.ts`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/command-registry.test.ts
import { describe, expect, test } from 'bun:test';
import { createCommandRegistry } from '../command-registry.js';
import type { CommandDefinition } from '../types.js';

describe('createCommandRegistry', () => {
	const testCommand: CommandDefinition = {
		name: 'test',
		aliases: ['t'],
		usage: '/test <arg>',
		description: 'A test command',
		category: 'meta',
		execute: (args) => ({ text: `executed: ${args}` }),
	};

	test('registers and looks up commands by name', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		expect(registry.get('test')).toBe(testCommand);
	});

	test('looks up commands by alias', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		expect(registry.get('t')).toBe(testCommand);
	});

	test('returns undefined for unknown commands', () => {
		const registry = createCommandRegistry();
		expect(registry.get('nonexistent')).toBeUndefined();
	});

	test('lists all commands', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		const all = registry.getAll();
		expect(all).toHaveLength(1);
		expect(all[0]!.name).toBe('test');
	});

	test('lists commands by category', () => {
		const registry = createCommandRegistry();
		registry.register(testCommand);
		registry.register({
			...testCommand,
			name: 'other',
			aliases: [],
			category: 'ai',
		});
		expect(registry.getByCategory('meta')).toHaveLength(1);
		expect(registry.getByCategory('ai')).toHaveLength(1);
	});

	test('registerAll registers multiple commands', () => {
		const registry = createCommandRegistry();
		registry.registerAll([testCommand, { ...testCommand, name: 'test2', aliases: [] }]);
		expect(registry.getAll()).toHaveLength(2);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/command-registry.test.ts`
Expected: FAIL — module not found

**Step 3: Write implementation**

```typescript
// simse-code/command-registry.ts
import type { CommandCategory, CommandDefinition } from './types.js';

export interface CommandRegistry {
	readonly register: (command: CommandDefinition) => void;
	readonly registerAll: (commands: readonly CommandDefinition[]) => void;
	readonly get: (nameOrAlias: string) => CommandDefinition | undefined;
	readonly getAll: () => readonly CommandDefinition[];
	readonly getByCategory: (category: CommandCategory) => readonly CommandDefinition[];
}

export function createCommandRegistry(): CommandRegistry {
	const commands = new Map<string, CommandDefinition>();
	const aliases = new Map<string, string>();

	function register(command: CommandDefinition): void {
		commands.set(command.name, command);
		if (command.aliases) {
			for (const alias of command.aliases) {
				aliases.set(alias, command.name);
			}
		}
	}

	function registerAll(cmds: readonly CommandDefinition[]): void {
		for (const cmd of cmds) register(cmd);
	}

	function get(nameOrAlias: string): CommandDefinition | undefined {
		return commands.get(nameOrAlias) ?? commands.get(aliases.get(nameOrAlias) ?? '');
	}

	function getAll(): readonly CommandDefinition[] {
		return [...commands.values()];
	}

	function getByCategory(category: CommandCategory): readonly CommandDefinition[] {
		return [...commands.values()].filter((c) => c.category === category);
	}

	return Object.freeze({ register, registerAll, get, getAll, getByCategory });
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/command-registry.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/command-registry.ts simse-code/tests/command-registry.test.ts
git commit -m "feat(ink): add CommandRegistry with name/alias lookup and category filtering"
```

---

### Task 16: Create Meta Feature Module (first feature module)

**Files:**
- Create: `simse-code/features/meta/commands.ts`
- Create: `simse-code/features/meta/components.tsx`
- Create: `simse-code/features/meta/index.ts`
- Create: `simse-code/tests/features-meta.test.tsx`

This task creates the `/help`, `/status`, `/context`, `/clear`, `/verbose`, `/plan` commands as the first feature module, establishing the pattern for all other modules.

**Step 1: Write the failing test**

```typescript
// simse-code/tests/features-meta.test.tsx
import { describe, expect, test } from 'bun:test';
import { metaCommands } from '../features/meta/index.js';

describe('meta feature module', () => {
	test('exports an array of command definitions', () => {
		expect(Array.isArray(metaCommands)).toBe(true);
		expect(metaCommands.length).toBeGreaterThan(0);
	});

	test('all commands have category "meta"', () => {
		for (const cmd of metaCommands) {
			expect(cmd.category).toBe('meta');
		}
	});

	test('includes help command', () => {
		const help = metaCommands.find((c) => c.name === 'help');
		expect(help).toBeDefined();
		expect(help!.aliases).toContain('?');
	});

	test('includes clear command', () => {
		const clear = metaCommands.find((c) => c.name === 'clear');
		expect(clear).toBeDefined();
	});

	test('includes verbose command', () => {
		const verbose = metaCommands.find((c) => c.name === 'verbose');
		expect(verbose).toBeDefined();
	});

	test('includes plan command', () => {
		const plan = metaCommands.find((c) => c.name === 'plan');
		expect(plan).toBeDefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/features-meta.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/features/meta/components.tsx
import { Box, Text } from 'ink';
import React from 'react';
import type { CommandDefinition } from '../../types.js';

interface HelpViewProps {
	readonly commands: readonly CommandDefinition[];
}

const CATEGORY_LABELS: Record<string, string> = {
	ai: 'AI & Chains',
	library: 'Library',
	tools: 'Tools & Agents',
	session: 'Session',
	files: 'Files & VFS',
	config: 'Configuration',
	meta: 'General',
};

export function HelpView({ commands }: HelpViewProps) {
	const categories = new Map<string, CommandDefinition[]>();
	for (const cmd of commands) {
		const list = categories.get(cmd.category) ?? [];
		list.push(cmd);
		categories.set(cmd.category, list);
	}

	return (
		<Box flexDirection="column" paddingX={1}>
			{[...categories.entries()].map(([category, cmds]) => (
				<Box key={category} flexDirection="column" marginBottom={1}>
					<Text bold color="cyan">
						{CATEGORY_LABELS[category] ?? category}
					</Text>
					{cmds.map((cmd) => (
						<Box key={cmd.name} gap={2} paddingLeft={2}>
							<Text>{cmd.usage.padEnd(30)}</Text>
							<Text dimColor>{cmd.description}</Text>
						</Box>
					))}
				</Box>
			))}
		</Box>
	);
}

interface ContextGridProps {
	readonly usedChars: number;
	readonly maxChars: number;
}

export function ContextGrid({ usedChars, maxChars }: ContextGridProps) {
	const ratio = Math.min(1, usedChars / maxChars);
	const pct = Math.round(ratio * 100);
	const width = 40;
	const filled = Math.round(ratio * width);

	const color = pct < 60 ? 'green' : pct < 85 ? 'yellow' : 'red';

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text>
				Context usage: <Text color={color} bold>{pct}%</Text>{' '}
				<Text dimColor>({usedChars.toLocaleString()} / {maxChars.toLocaleString()} chars)</Text>
			</Text>
			<Text color={color}>
				{'█'.repeat(filled)}{'░'.repeat(width - filled)}
			</Text>
		</Box>
	);
}
```

```typescript
// simse-code/features/meta/commands.ts
import React from 'react';
import type { CommandDefinition } from '../../types.js';
import { ContextGrid, HelpView } from './components.js';

/**
 * Create meta commands. These need access to the full command registry
 * to render help, so they accept a getter function.
 */
export function createMetaCommands(
	getCommands: () => readonly CommandDefinition[],
): readonly CommandDefinition[] {
	return [
		{
			name: 'help',
			aliases: ['?'],
			usage: '/help',
			description: 'Show available commands',
			category: 'meta',
			execute: () => ({
				element: React.createElement(HelpView, { commands: getCommands() }),
			}),
		},
		{
			name: 'clear',
			usage: '/clear',
			description: 'Clear conversation history',
			category: 'meta',
			execute: () => ({ text: 'Conversation cleared.' }),
		},
		{
			name: 'verbose',
			aliases: ['v'],
			usage: '/verbose [on|off]',
			description: 'Toggle verbose output',
			category: 'meta',
			execute: (args) => {
				// State mutation happens in session provider
				return { text: `Verbose mode: ${args || 'toggled'}` };
			},
		},
		{
			name: 'plan',
			usage: '/plan [on|off]',
			description: 'Toggle plan mode',
			category: 'meta',
			execute: (args) => {
				return { text: `Plan mode: ${args || 'toggled'}` };
			},
		},
		{
			name: 'context',
			usage: '/context',
			description: 'Show context window usage',
			category: 'meta',
			execute: () => ({
				element: React.createElement(ContextGrid, {
					usedChars: 0,
					maxChars: 200000,
				}),
			}),
		},
		{
			name: 'exit',
			aliases: ['quit', 'q'],
			usage: '/exit',
			description: 'Exit the application',
			category: 'meta',
			execute: () => undefined,
		},
	] as const;
}
```

```typescript
// simse-code/features/meta/index.ts
export { createMetaCommands } from './commands.js';
export { HelpView, ContextGrid } from './components.js';

// For tests: a standalone array of meta commands with a no-op getCommands
import { createMetaCommands } from './commands.js';
export const metaCommands = createMetaCommands(() => metaCommands);
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/features-meta.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/features/meta/ simse-code/tests/features-meta.test.tsx
git commit -m "feat(ink): add meta feature module (help, clear, verbose, plan, context, exit)"
```

---

### Task 17: Create Library Feature Module

**Files:**
- Create: `simse-code/features/library/commands.ts`
- Create: `simse-code/features/library/components.tsx`
- Create: `simse-code/features/library/index.ts`
- Create: `simse-code/tests/features-library.test.tsx`

This task migrates the `/add`, `/search`, `/notes`, `/topics`, `/get`, `/delete`, `/recommend` commands. The commands use `useServices()` inside their component renders to access the library.

**Step 1: Write the failing test**

```typescript
// simse-code/tests/features-library.test.tsx
import { describe, expect, test } from 'bun:test';
import { libraryCommands } from '../features/library/index.js';

describe('library feature module', () => {
	test('exports an array of command definitions', () => {
		expect(Array.isArray(libraryCommands)).toBe(true);
		expect(libraryCommands.length).toBeGreaterThan(0);
	});

	test('all commands have category "library"', () => {
		for (const cmd of libraryCommands) {
			expect(cmd.category).toBe('library');
		}
	});

	test('includes search command with alias', () => {
		const search = libraryCommands.find((c) => c.name === 'search');
		expect(search).toBeDefined();
		expect(search!.aliases).toContain('s');
	});

	test('includes add command', () => {
		const add = libraryCommands.find((c) => c.name === 'add');
		expect(add).toBeDefined();
	});

	test('includes notes command with alias', () => {
		const notes = libraryCommands.find((c) => c.name === 'notes');
		expect(notes).toBeDefined();
		expect(notes!.aliases).toContain('ls');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/features-library.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/features/library/components.tsx
import { Box, Text } from 'ink';
import React from 'react';

interface SearchResult {
	readonly id: string;
	readonly text: string;
	readonly topic: string;
	readonly score: number;
}

interface SearchResultsProps {
	readonly results: readonly SearchResult[];
	readonly query: string;
}

export function SearchResults({ results, query }: SearchResultsProps) {
	if (results.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>No results for "{query}"</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>{results.length} result{results.length === 1 ? '' : 's'} for "{query}"</Text>
			{results.map((r) => (
				<Box key={r.id} flexDirection="column" marginTop={1}>
					<Box gap={2}>
						<Text dimColor>[{r.id.slice(0, 8)}]</Text>
						<Text bold color="cyan">{r.topic}</Text>
						<Text dimColor>{r.score.toFixed(3)}</Text>
					</Box>
					<Text wrap="truncate-end">{r.text.slice(0, 200)}</Text>
				</Box>
			))}
		</Box>
	);
}

interface NoteListProps {
	readonly notes: readonly { id: string; text: string; topic: string; createdAt?: number }[];
	readonly topic?: string;
}

export function NoteList({ notes, topic }: NoteListProps) {
	if (notes.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>{topic ? `No notes in "${topic}"` : 'No notes'}</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>{notes.length} note{notes.length === 1 ? '' : 's'}{topic ? ` in "${topic}"` : ''}</Text>
			{notes.map((n) => (
				<Box key={n.id} gap={2}>
					<Text dimColor>[{n.id.slice(0, 8)}]</Text>
					<Text wrap="truncate-end">{n.text.slice(0, 100)}</Text>
				</Box>
			))}
		</Box>
	);
}

interface TopicListProps {
	readonly topics: readonly { name: string; count: number }[];
}

export function TopicList({ topics }: TopicListProps) {
	if (topics.length === 0) {
		return (
			<Box paddingLeft={2}>
				<Text dimColor>No topics</Text>
			</Box>
		);
	}

	return (
		<Box flexDirection="column" paddingLeft={2}>
			<Text bold>{topics.length} topic{topics.length === 1 ? '' : 's'}</Text>
			{topics.map((t) => (
				<Box key={t.name} gap={2}>
					<Text color="cyan">{t.name}</Text>
					<Text dimColor>({t.count})</Text>
				</Box>
			))}
		</Box>
	);
}
```

```typescript
// simse-code/features/library/commands.ts
import type { CommandDefinition } from '../../types.js';

export const libraryCommands: readonly CommandDefinition[] = [
	{
		name: 'add',
		usage: '/add <topic> <text>',
		description: 'Add a note to a topic',
		category: 'library',
		execute: (args) => {
			const spaceIdx = args.indexOf(' ');
			if (spaceIdx === -1) return { text: 'Usage: /add <topic> <text>' };
			return { text: `Adding to "${args.slice(0, spaceIdx)}"...` };
		},
	},
	{
		name: 'search',
		aliases: ['s'],
		usage: '/search <query>',
		description: 'Semantic search across library',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /search <query>' };
			return { text: `Searching for "${args}"...` };
		},
	},
	{
		name: 'recommend',
		aliases: ['rec'],
		usage: '/recommend <query>',
		description: 'Get recommendations weighted by recency/frequency',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /recommend <query>' };
			return { text: `Recommending for "${args}"...` };
		},
	},
	{
		name: 'topics',
		usage: '/topics',
		description: 'List all topics',
		category: 'library',
		execute: () => ({ text: 'Listing topics...' }),
	},
	{
		name: 'notes',
		aliases: ['ls'],
		usage: '/notes [topic]',
		description: 'List notes (optionally filtered by topic)',
		category: 'library',
		execute: (args) => ({ text: args ? `Notes in "${args}"...` : 'Listing all notes...' }),
	},
	{
		name: 'get',
		usage: '/get <id>',
		description: 'Get a note by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /get <id>' };
			return { text: `Getting note ${args}...` };
		},
	},
	{
		name: 'delete',
		aliases: ['rm'],
		usage: '/delete <id>',
		description: 'Delete a note by ID',
		category: 'library',
		execute: (args) => {
			if (!args.trim()) return { text: 'Usage: /delete <id>' };
			return { text: `Deleting note ${args}...` };
		},
	},
];
```

```typescript
// simse-code/features/library/index.ts
export { libraryCommands } from './commands.js';
export { SearchResults, NoteList, TopicList } from './components.js';
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/features-library.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/features/library/ simse-code/tests/features-library.test.tsx
git commit -m "feat(ink): add library feature module (add, search, notes, topics, get, delete, recommend)"
```

---

### Task 18: Create Remaining Feature Modules (tools, session, files, config, ai)

**Files:**
- Create: `simse-code/features/tools/commands.ts`
- Create: `simse-code/features/tools/index.ts`
- Create: `simse-code/features/session/commands.ts`
- Create: `simse-code/features/session/index.ts`
- Create: `simse-code/features/files/commands.ts`
- Create: `simse-code/features/files/index.ts`
- Create: `simse-code/features/config/commands.ts`
- Create: `simse-code/features/config/index.ts`
- Create: `simse-code/features/ai/commands.ts`
- Create: `simse-code/features/ai/index.ts`
- Create: `simse-code/tests/features-all.test.ts`

Follow the same pattern as Tasks 16-17 for each module. Each module exports its commands array and any components.

**Commands by module:**

- **tools/**: `/tools`, `/agents`, `/skills`
- **session/**: `/server`, `/agent`, `/model`, `/mcp`, `/acp`, `/library`, `/bypass-permissions`, `/embed`
- **files/**: `/files`, `/save`, `/validate`, `/discard`, `/diff`
- **config/**: `/config`, `/settings`, `/init`
- **ai/**: `/chain`, `/prompts` (bare text input handled separately in the App component)

**Step 1: Write the failing test**

```typescript
// simse-code/tests/features-all.test.ts
import { describe, expect, test } from 'bun:test';
import { toolsCommands } from '../features/tools/index.js';
import { sessionCommands } from '../features/session/index.js';
import { filesCommands } from '../features/files/index.js';
import { configCommands } from '../features/config/index.js';
import { aiCommands } from '../features/ai/index.js';

describe('all feature modules', () => {
	test('tools module exports commands with correct category', () => {
		expect(toolsCommands.length).toBeGreaterThan(0);
		for (const cmd of toolsCommands) expect(cmd.category).toBe('tools');
	});

	test('session module exports commands with correct category', () => {
		expect(sessionCommands.length).toBeGreaterThan(0);
		for (const cmd of sessionCommands) expect(cmd.category).toBe('session');
	});

	test('files module exports commands with correct category', () => {
		expect(filesCommands.length).toBeGreaterThan(0);
		for (const cmd of filesCommands) expect(cmd.category).toBe('files');
	});

	test('config module exports commands with correct category', () => {
		expect(configCommands.length).toBeGreaterThan(0);
		for (const cmd of configCommands) expect(cmd.category).toBe('config');
	});

	test('ai module exports commands with correct category', () => {
		expect(aiCommands.length).toBeGreaterThan(0);
		for (const cmd of aiCommands) expect(cmd.category).toBe('ai');
	});

	test('no duplicate command names across modules', () => {
		const allNames = [
			...toolsCommands, ...sessionCommands, ...filesCommands,
			...configCommands, ...aiCommands,
		].map((c) => c.name);
		const unique = new Set(allNames);
		expect(unique.size).toBe(allNames.length);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/features-all.test.ts`
Expected: FAIL — modules not found

**Step 3: Write all implementations**

Each module follows the same pattern as library/meta. Commands start as stubs that return text — the actual service integration (calling `app.library.search()`, `acpClient.listModels()`, etc.) happens in Task 20 when we wire up the App component with hooks.

Create each module with its commands and barrel export. Reference the existing `cli.ts` for exact command names, aliases, usage strings, and descriptions.

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/features-all.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/features/ simse-code/tests/features-all.test.ts
git commit -m "feat(ink): add tools, session, files, config, ai feature modules"
```

---

### Task 19: Create MainLayout Component

**Files:**
- Create: `simse-code/components/layout/main-layout.tsx`
- Create: `simse-code/tests/main-layout.test.tsx`

**Step 1: Write the failing test**

```typescript
// simse-code/tests/main-layout.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { MainLayout } from '../components/layout/main-layout.js';

describe('MainLayout', () => {
	test('renders children', () => {
		const { lastFrame } = render(
			<MainLayout>
				<></>
			</MainLayout>,
		);
		expect(lastFrame()).toBeDefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/main-layout.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```tsx
// simse-code/components/layout/main-layout.tsx
import { Box } from 'ink';
import React from 'react';
import type { ReactNode } from 'react';

interface MainLayoutProps {
	readonly children: ReactNode;
}

export function MainLayout({ children }: MainLayoutProps) {
	return (
		<Box flexDirection="column" flexGrow={1}>
			{children}
		</Box>
	);
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/main-layout.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/components/layout/main-layout.tsx simse-code/tests/main-layout.test.tsx
git commit -m "feat(ink): add MainLayout component"
```

---

### Task 20: Create App Component & CLI Entry Point

**Files:**
- Create: `simse-code/app-ink.tsx` (the root App component)
- Create: `simse-code/cli-ink.tsx` (the new entry point)
- Create: `simse-code/hooks/use-command-dispatch.ts`
- Create: `simse-code/tests/app-ink.test.tsx`

This is the integration task that wires everything together: providers, layout, command dispatch, and the REPL loop.

**Step 1: Write the failing test**

```typescript
// simse-code/tests/app-ink.test.tsx
import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { App } from '../app-ink.js';

describe('App', () => {
	test('renders without crashing', () => {
		const { lastFrame } = render(<App dataDir="/test" />);
		// Should render at minimum the prompt
		expect(lastFrame()).toBeDefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/app-ink.test.tsx`
Expected: FAIL — module not found

**Step 3: Write implementation**

```typescript
// simse-code/hooks/use-command-dispatch.ts
import { useCallback } from 'react';
import type { CommandDefinition, CommandResult } from '../types.js';
import type { CommandRegistry } from '../command-registry.js';

interface UseCommandDispatchResult {
	readonly dispatch: (input: string) => Promise<CommandResult | undefined>;
	readonly isCommand: (input: string) => boolean;
}

export function useCommandDispatch(
	registry: CommandRegistry,
): UseCommandDispatchResult {
	const isCommand = useCallback(
		(input: string) => input.startsWith('/'),
		[],
	);

	const dispatch = useCallback(
		async (input: string): Promise<CommandResult | undefined> => {
			const trimmed = input.trim();
			if (!trimmed.startsWith('/')) return undefined;

			const spaceIdx = trimmed.indexOf(' ');
			const name = spaceIdx === -1 ? trimmed.slice(1) : trimmed.slice(1, spaceIdx);
			const args = spaceIdx === -1 ? '' : trimmed.slice(spaceIdx + 1).trim();

			const command = registry.get(name);
			if (!command) {
				return { text: `Unknown command: /${name}. Type /help for available commands.` };
			}

			const result = command.execute(args);
			return result instanceof Promise ? await result : result ?? undefined;
		},
		[registry],
	);

	return { dispatch, isCommand };
}
```

```tsx
// simse-code/app-ink.tsx
import { Box, Text } from 'ink';
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
import type { OutputItem } from './types.js';

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
		return reg;
	}, []);

	const { dispatch, isCommand } = useCommandDispatch(registry);

	const handleSubmit = useCallback(
		async (input: string) => {
			setIsProcessing(true);

			// Add user message to output
			setItems((prev) => [...prev, { kind: 'message', role: 'user', text: input }]);

			if (isCommand(input)) {
				const result = await dispatch(input);
				if (result?.text) {
					setItems((prev) => [...prev, { kind: 'info', text: result.text! }]);
				} else if (result?.element) {
					setItems((prev) => [...prev, { kind: 'command-result', element: result.element }]);
				}
			} else {
				// Bare text — for now, echo back. Full agentic loop integration in a follow-up task.
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
```

```tsx
// simse-code/cli-ink.tsx
#!/usr/bin/env bun
import { render } from 'ink';
import React from 'react';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { App } from './app-ink.js';

function parseArgs(): { dataDir: string; serverName?: string } {
	const args = process.argv.slice(2);
	let dataDir = join(homedir(), '.simse');

	for (let i = 0; i < args.length; i++) {
		if (args[i] === '--data-dir' && args[i + 1]) {
			dataDir = args[i + 1]!;
			i++;
		}
	}

	return { dataDir };
}

const { dataDir, serverName } = parseArgs();
render(<App dataDir={dataDir} serverName={serverName} />);
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/app-ink.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/app-ink.tsx simse-code/cli-ink.tsx simse-code/hooks/use-command-dispatch.ts simse-code/tests/app-ink.test.tsx
git commit -m "feat(ink): add root App component, CLI entry point, and command dispatch hook"
```

---

### Task 21: Wire Up Agentic Loop Hook

**Files:**
- Create: `simse-code/hooks/use-agentic-loop.ts`
- Create: `simse-code/tests/use-agentic-loop.test.ts`

This hook wraps the existing `createAgenticLoop` from `loop.ts` and provides React state for streaming text, active tool calls, and completed items.

**Step 1: Write the failing test**

```typescript
// simse-code/tests/use-agentic-loop.test.ts
import { describe, expect, test } from 'bun:test';
// For now, test the hook's internal helpers (the hook itself needs Ink context)
import { deriveToolSummary } from '../hooks/use-agentic-loop.js';

describe('deriveToolSummary', () => {
	test('counts lines for multiline output', () => {
		const output = 'line1\nline2\nline3';
		expect(deriveToolSummary('vfs_read', output)).toBe('3 lines');
	});

	test('returns byte count for large output', () => {
		const output = 'x'.repeat(1000);
		expect(deriveToolSummary('vfs_read', output)).toContain('1000');
	});

	test('returns undefined for empty output', () => {
		expect(deriveToolSummary('vfs_read', '')).toBeUndefined();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/use-agentic-loop.test.ts`
Expected: FAIL — module not found

**Step 3: Write implementation**

```typescript
// simse-code/hooks/use-agentic-loop.ts
import { useCallback, useRef, useState } from 'react';
import type { OutputItem, ToolCallState } from '../types.js';

export function deriveToolSummary(
	name: string,
	output: string,
): string | undefined {
	if (!output) return undefined;
	const lines = output.split('\n');
	if (lines.length > 1) return `${lines.length} lines`;
	if (output.length > 100) return `${output.length} chars`;
	return undefined;
}

interface AgenticLoopState {
	readonly status: 'idle' | 'streaming' | 'tool-executing';
	readonly streamText: string;
	readonly activeToolCalls: readonly ToolCallState[];
	readonly completedItems: readonly OutputItem[];
}

interface UseAgenticLoopResult {
	readonly state: AgenticLoopState;
	readonly submit: (input: string) => Promise<void>;
	readonly abort: () => void;
}

/**
 * Hook for managing the agentic loop lifecycle.
 *
 * Full integration with createAgenticLoop will be wired in a follow-up
 * once providers are connected. For now, exports the state shape and helpers.
 */
export function useAgenticLoop(): UseAgenticLoopResult {
	const [state, setState] = useState<AgenticLoopState>({
		status: 'idle',
		streamText: '',
		activeToolCalls: [],
		completedItems: [],
	});

	const abortRef = useRef<AbortController | undefined>();

	const submit = useCallback(async (_input: string) => {
		const ctrl = new AbortController();
		abortRef.current = ctrl;

		setState((prev) => ({
			...prev,
			status: 'streaming',
			streamText: '',
			activeToolCalls: [],
		}));

		// TODO: Wire createAgenticLoop here with callbacks that
		// update streamText, activeToolCalls, and completedItems
		// via setState calls

		setState((prev) => ({
			...prev,
			status: 'idle',
		}));
	}, []);

	const abort = useCallback(() => {
		abortRef.current?.abort();
		setState((prev) => ({ ...prev, status: 'idle' }));
	}, []);

	return { state, submit, abort };
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/use-agentic-loop.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/hooks/use-agentic-loop.ts simse-code/tests/use-agentic-loop.test.ts
git commit -m "feat(ink): add useAgenticLoop hook with state management and tool summary"
```

---

### Task 22: Create Provider Barrel & Index Files

**Files:**
- Create: `simse-code/providers/index.ts`
- Create: `simse-code/components/shared/index.ts`
- Create: `simse-code/components/chat/index.ts`
- Create: `simse-code/components/input/index.ts`
- Create: `simse-code/components/layout/index.ts`
- Create: `simse-code/components/index.ts`
- Create: `simse-code/hooks/index.ts`
- Create: `simse-code/features/index.ts`

**Step 1: Write barrel exports**

Each barrel re-exports all public symbols from its directory. For example:

```typescript
// simse-code/providers/index.ts
export { ThemeProvider, useTheme } from './theme-provider.js';
export { ServicesProvider, useServices } from './services-provider.js';
export type { ServicesContextValue } from './services-provider.js';
export { SessionProvider, useSession } from './session-provider.js';
```

```typescript
// simse-code/components/index.ts
export * from './shared/index.js';
export * from './chat/index.js';
export * from './input/index.js';
export * from './layout/index.js';
```

```typescript
// simse-code/hooks/index.ts
export { useCommandDispatch } from './use-command-dispatch.js';
export { useAgenticLoop, deriveToolSummary } from './use-agentic-loop.js';
```

```typescript
// simse-code/features/index.ts
export { createMetaCommands, metaCommands } from './meta/index.js';
export { libraryCommands } from './library/index.js';
export { toolsCommands } from './tools/index.js';
export { sessionCommands } from './session/index.js';
export { filesCommands } from './files/index.js';
export { configCommands } from './config/index.js';
export { aiCommands } from './ai/index.js';
```

**Step 2: Verify everything compiles**

Run: `cd simse-code && bun x tsc --noEmit`
Expected: No errors

**Step 3: Run full test suite**

Run: `cd simse-code && bun test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add simse-code/providers/index.ts simse-code/components/ simse-code/hooks/index.ts simse-code/features/index.ts
git commit -m "feat(ink): add barrel exports for providers, components, hooks, features"
```

---

### Task 23: Update Package.json Entry Point

**Files:**
- Modify: `simse-code/package.json`

**Step 1: Update bin and scripts**

```json
{
  "name": "simse-code",
  "version": "1.0.0",
  "private": true,
  "type": "module",
  "bin": {
    "simse": "./cli-ink.tsx"
  },
  "scripts": {
    "start": "bun run cli-ink.tsx",
    "start:legacy": "bun run cli.ts",
    "bridge": "bun run acp-ollama-bridge.ts",
    "typecheck": "bun x tsc --noEmit",
    "test": "bun test"
  },
  "dependencies": {
    "simse": "file:..",
    "ink": "^5.2.0",
    "ink-text-input": "^6.0.0",
    "ink-spinner": "^5.0.0",
    "ink-select-input": "^6.0.0",
    "react": "^18.3.0"
  },
  "devDependencies": {
    "ink-testing-library": "^4.0.0",
    "@types/react": "^18.3.0"
  }
}
```

Note: Keep `start:legacy` pointing to old `cli.ts` for fallback during migration.

**Step 2: Verify it runs**

Run: `cd simse-code && bun run start -- --help`
Expected: Ink app renders (even if minimal)

**Step 3: Commit**

```bash
git add simse-code/package.json
git commit -m "chore: update package.json entry point to cli-ink.tsx"
```

---

### Task 24: Run Full Test Suite & Fix Issues

**Step 1: Run full test suite**

Run: `cd simse-code && bun test`
Expected: All tests pass

**Step 2: Run typecheck**

Run: `cd simse-code && bun x tsc --noEmit`
Expected: No errors

**Step 3: Run lint**

Run: `cd .. && bun run lint`
Expected: No errors (or fix any auto-fixable issues with `bun run lint:fix`)

**Step 4: Fix any issues found**

Address any compilation errors, test failures, or lint issues.

**Step 5: Commit fixes**

```bash
git add -A
git commit -m "fix: resolve test and typecheck issues in Ink CLI"
```

---

### Task 25: Final Integration Smoke Test

**Step 1: Start the Ink CLI**

Run: `cd simse-code && bun run start`

**Step 2: Verify basic interactions**

- Type `/help` — should render the HelpView component with all registered commands
- Type `/status` — should render status info
- Type some text — should show "(Agentic loop not yet wired)" placeholder
- Type `/exit` — should exit cleanly
- Press Ctrl+C — should exit cleanly

**Step 3: Verify legacy CLI still works**

Run: `cd simse-code && bun run start:legacy`

Expected: Original CLI works unchanged.

**Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "test: verify Ink CLI smoke test passes"
```

---

## Follow-Up Work (not in this plan)

These tasks are deferred for a future iteration:

1. **Full agentic loop integration** — wire `useAgenticLoop` to `createAgenticLoop` with real ACP streaming, tool execution callbacks, and state updates
2. **Full service integration** — connect library commands to real `app.library.*` calls via `useServices()` hook
3. **Permission flow** — wire ACP permission requests to `PermissionDialog` component
4. **Session persistence** — wire checkpoint/session store to SessionProvider
5. **Keyboard shortcuts** — implement Ctrl+C abort, Escape key, Ctrl+O verbose toggle via `useInput`
6. **Theme switching** — wire theme manager to ThemeProvider
7. **Skill invocation** — wire skill registry to command dispatch
8. **Remove legacy code** — once all features are migrated, remove old `cli.ts`, `ui.ts` render functions, `picker.ts`, `todo-ui.ts`, `status-line.ts`, `app-context.ts`
