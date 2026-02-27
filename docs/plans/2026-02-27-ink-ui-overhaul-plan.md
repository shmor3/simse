# Ink UI Overhaul — Claude Code Visual Parity

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite all React Ink display components to pixel-match Claude Code's terminal UI while retaining simse branding.

**Architecture:** Replace bordered tool-call boxes with compact inline style. Add Ink-native markdown renderer for assistant messages. Build multi-line text input. Redesign banner, status bar, permission dialog, and thinking spinner to match CC conventions.

**Tech Stack:** React 19, Ink 6, chalk (already a dep), ink-spinner, ink-testing-library, bun:test

---

### Task 1: Markdown Component

**Files:**
- Create: `simse-code/components/chat/markdown.tsx`
- Test: `simse-code/tests/markdown.test.tsx`

**Step 1: Write the failing test**

Create `simse-code/tests/markdown.test.tsx` with test cases for:
- Plain text renders unchanged
- `**bold**` renders bold
- `` `inline code` `` renders in cyan
- Fenced code blocks with language label and `│` gutter
- `# Headers` render bold cyan (h1), bold (h2), underline (h3)
- `- list items` render with dash bullets
- `> blockquotes` render with dim `│` bar
- `---` renders as `─────` horizontal rule
- Empty text does not crash

**Step 2: Run test to verify it fails**

Run: `cd simse-code && bun test tests/markdown.test.tsx`
Expected: FAIL — module not found

**Step 3: Write the Markdown component**

Create `simse-code/components/chat/markdown.tsx`:

- `renderInline(line)` function — splits on `**bold**`, `*italic*`, `` `code` `` patterns using regex, returns `React.ReactNode[]` with appropriate `<Text>` wrappers
- `Markdown({ text })` component — splits text by newlines, iterates lines:
  - Toggle fenced code blocks (``` ``` ```) — render with `<Text dimColor>  │ </Text>` gutter
  - `---`/`***` → `<Text dimColor>{'─'.repeat(40)}</Text>`
  - `# Header` → `<Text bold color="cyan">`
  - `> quote` → `<Text dimColor>  │ </Text>` prefix
  - `- list` → indent + `- ` + renderInline
  - Default → renderInline
- Return `<Box flexDirection="column">{elements}</Box>`

**Step 4: Run test to verify it passes**

Run: `cd simse-code && bun test tests/markdown.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): add Markdown component for rich assistant message rendering`

---

### Task 2: Compact Tool Call Component

Replace the bordered `ToolCallBox` with Claude Code's inline style.

**Files:**
- Modify: `simse-code/components/chat/tool-call-box.tsx`
- Modify: `simse-code/tests/tool-call-box.test.tsx`

**Step 1: Update the tests**

Update `simse-code/tests/tool-call-box.test.tsx`:
- Active tool call: shows display name + primary arg, NO border chars (`╭`, `╰`)
- Completed: shows display name + arg + summary + formatted duration
- Failed: shows error with `⎿` tree connector
- Maps tool names to display names (e.g., `vfs_read` → `Read`, `file_edit` → `Update`, `bash` → `Bash`)
- Extracts primary arg from common keys (`path`, `command`, `query`, etc.)

**Step 2: Run tests to verify they fail**

Run: `cd simse-code && bun test tests/tool-call-box.test.tsx`
Expected: FAIL — borders still present, display names not mapped

**Step 3: Rewrite ToolCallBox as compact inline**

Rewrite `simse-code/components/chat/tool-call-box.tsx`:

- Add `TOOL_DISPLAY_NAMES` map (copy from `ui.ts`)
- Add `getDisplayName(name)` function
- Add `extractPrimaryArg(argsStr)` function — tries keys: path, file_path, filePath, filename, command, query, pattern, name, url
- Add `formatDuration(ms)` function
- `StatusIndicator` component: active → magenta `<InkSpinner type="dots" />`, completed → magenta `⏺`, failed → red `⏺`
- Main render: `<Box flexDirection="column" paddingLeft={2}>` (NO borderStyle)
  - Line 1: `<StatusIndicator /> <Text bold>{display}</Text> <Text dimColor>{suffix}</Text>`
  - Error: `    ⎿ <Text color="red">{error}</Text>`
  - Diff: lines with `⎿` prefix, colored `+green`/`-red`

**Step 4: Run tests to verify they pass**

Run: `cd simse-code && bun test tests/tool-call-box.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): replace bordered ToolCallBox with compact CC-style inline display`

---

### Task 3: Message List + Markdown Integration

Update `OutputItemView` to use `Markdown` for assistant messages and `❯` for user messages.

**Files:**
- Modify: `simse-code/components/chat/message-list.tsx`
- Modify: `simse-code/tests/message-list.test.tsx`

**Step 1: Update tests**

Update `simse-code/tests/message-list.test.tsx`:
- User messages render with `❯` prompt marker (not `>`)
- Assistant messages render through Markdown (bold text appears)
- Tool calls render inline (no borders)
- Error and info items still work

**Step 2: Run tests to verify failure**

Run: `cd simse-code && bun test tests/message-list.test.tsx`
Expected: FAIL — `❯` not present

**Step 3: Update MessageList component**

Modify `simse-code/components/chat/message-list.tsx`:
- Import `Markdown` from `./markdown.js`
- User message: `<Text color="cyan" bold>{'❯ '}</Text><Text bold>{item.text}</Text>`
- Assistant message: `<Box paddingLeft={2}><Markdown text={item.text} /></Box>`
- Rest stays the same (tool-call, command-result, error, info)

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/message-list.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): integrate Markdown rendering and ❯ prompt in MessageList`

---

### Task 4: Multi-Line Text Input

**Files:**
- Modify: `simse-code/components/input/text-input.tsx`
- Modify: `simse-code/tests/text-input.test.tsx`

**Step 1: Update tests**

Add to `simse-code/tests/text-input.test.tsx`:
- Multi-line value renders both lines
- Continuation lines show proper indentation

**Step 2: Run test to verify failure**

Run: `cd simse-code && bun test tests/text-input.test.tsx`
Expected: FAIL — newlines not rendered as multi-line

**Step 3: Update TextInput for multi-line support**

Modify `simse-code/components/input/text-input.tsx`:
- In `handleInput`: check if `key.shift && key.return` → insert `\n` at cursor position. Regular `key.return` (without shift) → submit as before.
- In render: split value by `\n`, render each line as separate `<Text>`. First line renders normally with cursor. Continuation lines indented 2 spaces. Cursor position tracked across multi-line (offset from start of full string, map to line:col).

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/text-input.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): add multi-line input support with Shift+Enter`

---

### Task 5: Prompt Input Redesign

**Files:**
- Modify: `simse-code/components/input/prompt-input.tsx`
- Modify: `simse-code/tests/prompt-input.test.tsx`

**Step 1: Update tests**

Update `simse-code/tests/prompt-input.test.tsx`:
- Renders `❯` prompt marker (not `>`)
- Badges render before prompt
- Shows dimmed placeholder when empty and inactive

**Step 2: Run tests to verify failure**

Run: `cd simse-code && bun test tests/prompt-input.test.tsx`
Expected: FAIL — `❯` not present

**Step 3: Update PromptInput**

Change `>` to `❯` in `simse-code/components/input/prompt-input.tsx`.

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/prompt-input.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): use ❯ prompt marker in PromptInput`

---

### Task 6: Banner Redesign

**Files:**
- Modify: `simse-code/components/layout/banner.tsx`
- Modify: `simse-code/tests/banner.test.tsx`

**Step 1: Update tests**

Update `simse-code/tests/banner.test.tsx`:
- Renders version info
- Renders mascot lines (`╭──╮`, `╰─╮│`, `╰╯`)
- Renders server and model info
- Renders working directory
- Renders tips

**Step 2: Run tests to check baseline**

Run: `cd simse-code && bun test tests/banner.test.tsx`

**Step 3: Rewrite Banner**

Replace `simse-code/components/layout/banner.tsx` with:
- Single `<Box borderStyle="round" borderColor="gray">` (Ink's built-in borders)
- Top row: mascot (left, `marginRight={4}`) + tips column (right)
- Bottom: version, server:model, workDir (dimmed)
- Remove the manual two-column layout, `useMemo`, `useStdout`, `DIVIDER`, `RowData`
- Remove the hint lines below the box ("Try add..." and "? for shortcuts")

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/banner.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): redesign Banner as single bordered box with compact layout`

---

### Task 7: Status Bar Redesign

**Files:**
- Modify: `simse-code/components/layout/status-bar.tsx`
- Modify: `simse-code/tests/status-bar.test.tsx`

**Step 1: Update tests**

Update `simse-code/tests/status-bar.test.tsx`:
- Server:model renders with colon separator
- Token count formats with `k` suffix (12345 → `12.3k tokens`)
- Cost renders
- Mode badges on the right

**Step 2: Run tests to verify failure**

Run: `cd simse-code && bun test tests/status-bar.test.tsx`
Expected: FAIL — token formatting doesn't use `k` suffix

**Step 3: Update StatusBar**

Modify `simse-code/components/layout/status-bar.tsx`:
- Add `formatTokens(tokens)` function: `tokens >= 1000 ? `${(tokens / 1000).toFixed(1)}k tokens` : `${tokens} tokens``
- Remove `additions`/`deletions` props (not in CC style)
- Clean up: left side dim info, right side colored badges

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/status-bar.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): compact CC-style StatusBar with k-formatted tokens`

---

### Task 8: Permission Dialog Redesign

**Files:**
- Modify: `simse-code/components/input/permission-dialog.tsx`
- Modify: `simse-code/tests/permission-dialog.test.tsx`

**Step 1: Update tests**

Update `simse-code/tests/permission-dialog.test.tsx`:
- Renders `⚠` warning icon
- Shows tool name
- NO bordered box (`╭` should not appear)
- Shows keyboard shortcuts `[y]`, `[n]`, `[a]`

**Step 2: Run tests to verify failure**

Run: `cd simse-code && bun test tests/permission-dialog.test.tsx`
Expected: FAIL — bordered box still present

**Step 3: Rewrite PermissionDialog**

Replace `simse-code/components/input/permission-dialog.tsx`:
- Remove `borderStyle="round"` and `borderColor="yellow"`
- Layout: `<Box flexDirection="column" paddingLeft={2} marginY={1}>`
  - Line 1: `⚠  simse wants to run <bold>{toolDisplay}</bold>`
  - Line 2: blank
  - Line 3: `Allow? [y]es / [n]o / [a]lways` (dimmed)
- Extract primary arg like ToolCallBox does

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/permission-dialog.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): CC-style PermissionDialog without bordered box`

---

### Task 9: Thinking Spinner Redesign

**Files:**
- Modify: `simse-code/components/shared/spinner.tsx`
- Modify: `simse-code/tests/shared-components.test.tsx`

**Step 1: Update tests**

Add to `simse-code/tests/shared-components.test.tsx`:
- ThinkingSpinner shows label and suffix with elapsed, tokens, server
- Uses magenta color (not cyan)

**Step 2: Run test to verify failure**

Run: `cd simse-code && bun test tests/shared-components.test.tsx`

**Step 3: Update ThinkingSpinner to CC style**

Modify `simse-code/components/shared/spinner.tsx`:
- Change spinner color from `cyan` to `magenta`
- Add `formatDuration(ms)` helper
- Format tokens with `k` suffix
- Suffix: `(elapsed · tokens · server)` separated by `·`
- Padding left 2, gap 1

**Step 4: Run tests**

Run: `cd simse-code && bun test tests/shared-components.test.tsx`
Expected: PASS

**Step 5: Commit**

Commit message: `feat(ink): CC-style ThinkingSpinner with magenta indicator and suffix`

---

### Task 10: App Wiring — Active Area Update

**Files:**
- Modify: `simse-code/app-ink.tsx`

**Step 1: Update active area rendering**

In `simse-code/app-ink.tsx`:
1. Import `Markdown` from `./components/chat/markdown.js`
2. Import `ThinkingSpinner` from `./components/shared/spinner.js`
3. Remove direct `InkSpinner` import
4. Replace active streaming text section:
   - Old: `<Text><Text color="magenta">{'● '}</Text>{loopState.streamText}</Text>`
   - New: `<Box paddingLeft={2}><Markdown text={loopState.streamText} /></Box>`
5. Replace thinking spinner:
   - Old: `<InkSpinner type="dots" />` with `<Text dimColor>Thinking...</Text>`
   - New: `<ThinkingSpinner />`

**Step 2: Run all tests**

Run: `cd simse-code && bun test`
Expected: ALL PASS

**Step 3: Commit**

Commit message: `feat(ink): wire new CC-style components into active area rendering`

---

### Task 11: Full Integration Verification

**Step 1: Run typecheck**

Run: `cd simse-code && bunx tsc --noEmit`
Expected: No errors

**Step 2: Run full test suite**

Run: `cd simse-code && bun test`
Expected: ALL PASS

**Step 3: Run lint**

Run: `cd simse-code && bun run lint`
Expected: No errors

**Step 4: Fix any issues found**

**Step 5: Commit fixes if any**

Commit message: `chore(ink): fix typecheck and lint issues from UI overhaul`

---

### Task 12: Clean Up Old Components

**Files:**
- Delete: `simse-code/components/chat/streaming-text.tsx` (replaced by Markdown)
- Delete: `simse-code/tests/streaming-text.test.tsx`
- Possibly delete: `simse-code/components/chat/inline-diff.tsx` if no longer imported
- Possibly delete: `simse-code/tests/inline-diff.test.tsx`

**Step 1: Search for imports of deleted files**

Check all files for imports of `streaming-text` and `inline-diff`.

**Step 2: Remove imports and delete files**

**Step 3: Run full test suite**

Run: `cd simse-code && bun test`
Expected: ALL PASS

**Step 4: Commit**

Commit message: `chore(ink): remove replaced streaming-text and inline-diff components`
