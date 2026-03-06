# Visual Diffs Improvement Design

## Goal

Enhance the diff rendering in simse-code with word-level inline diffs, polished display, and wired-up integration so diffs appear automatically on writes and via the /diff command.

## Scope

1. **Word-level inline diffs** — character-level LCS on paired add/remove lines
2. **Polish diff rendering** — two-column gutter, improved truncation, compact mode
3. **Wire inline diffs** — auto-show on VFS writes, enhanced /diff command
4. **No syntax highlighting** — focus on diff markers only

## 1. Word-Level Inline Diffs

When a contiguous block of removes is followed by a contiguous block of adds within a hunk, pair them 1:1 in order. For each pair, compute character-level LCS to identify exactly which characters changed.

**Rendering:** Within a remove line's background, deleted characters get bold+bright styling. Within an add line's background, inserted characters get bold+bright styling. Common characters stay at normal diff background.

**Pairing heuristic:** Walk hunk lines, collect contiguous remove blocks and add blocks. Pair 1:1 in order. Unpaired lines render as full-line highlights (no word-level diff).

## 2. Polish — Gutter, Spacing, Truncation

**Gutter layout:**
```
   42 │ -const name = "hello";
   42 │ +const name = "world";
      │  // context
```

- Two-column line numbers (old for removes, new for adds, new for context)
- Fixed-width 4-char padded gutter with `│` separator
- Hunk headers: `@@ -10,5 +10,7 @@` with theme color
- Blank line between non-adjacent hunks

**Truncation:**
- Inline (tool result): maxLines 50, show `... N more changes (use /diff <file> to see all)`
- /diff command: maxLines 200
- Long lines (>120 chars) truncated with `...`

**Compact mode for inline tool results:**
- Tool header: `● Update(src/lib.ts)`
- Diff hunks with gutter
- Summary: `⎿ +3 -1`

## 3. Wiring

**Inline diffs on writes:**
- `onToolCallEnd` for vfs_write: check for previous version, compute diff via `vfs.diffVersions()`, render with `renderUnifiedDiff()` compact mode
- New files: show `⎿ Created new file (X lines)`

**/diff command:**
- No args: unified diffs for all changed files
- With path arg: unified diff for specific file
- Uses full maxLines (200)

**File tracker wiring:**
- Connect VFS `onFileWrite` to file tracker for change recording
- Store "before" content for diffing

## Files to Modify

- `simse-code/diff-display.ts` — word-level diff, gutter polish, compact mode
- `simse-code/cli.ts` — wire inline diffs on writes, enhance /diff command
- `tests/diff-display.test.ts` — tests for new rendering features
