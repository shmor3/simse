# Librarian Explorer & "Notes" → "Volumes" Rename

## Summary

Two changes:
1. A new `/librarians` command with an interactive two-panel explorer for managing librarian definitions (create, edit, delete)
2. Rename all user-facing "notes" terminology to "volumes" to match the library analogy

## Command

- **Name:** `/librarians`
- **Aliases:** `/libs`
- **Category:** `library`
- **Behavior:** Opens a modal LibrarianExplorer component (same pattern as SettingsExplorer)

## UI: Two-Panel Navigation

### Panel 1 — Librarian List

Lists all registered librarians loaded from `{librariansDir}/*.json`. Last item is `+ New librarian...` for inline creation.

```
╭ Librarians ──────────────────────────────╮
│ ❯ default         General-purpose head…  │
│   code-reviewer   Code review speciali…  │
│   + New librarian...                     │
╰──────────────────────────────────────────╯
  ↑↓ navigate  ↵/→ select  esc dismiss
```

### Panel 2 — Librarian Detail

All `LibrarianDefinition` fields rendered as editable rows. Nested objects (permissions, thresholds, acp) shown as indented sub-fields.

```
╭ default ─────────────────────────────────╮
│ ❯ name           default                 │
│   description    General-purpose head…   │
│   purpose        General-purpose head…   │
│   topics         **                      │
│   permissions                            │
│     add          true                    │
│     delete       true                    │
│     reorganize   true                    │
│   thresholds                             │
│     topicComplexity  100                 │
│     escalateAt       500                 │
│   acp                                    │
│     command      (unset)                 │
│     args         (unset)                 │
│     agentId      (unset)                 │
│   ⚠ Delete librarian                    │
╰──────────────────────────────────────────╯
  ↑↓ navigate  ↵ edit  ←/esc back
```

## Field Editing

| Field | Edit Type | Dropdown Source |
|---|---|---|
| `name` | Text input | — (kebab-case validated) |
| `description` | Text input | — |
| `purpose` | Text input | — |
| `topics` | Text input (comma-separated globs) | — |
| `permissions.add` | Dropdown | `true`, `false` |
| `permissions.delete` | Dropdown | `true`, `false` |
| `permissions.reorganize` | Dropdown | `true`, `false` |
| `thresholds.topicComplexity` | Dropdown + custom | `25`, `50`, `100`, `200`, `Custom value...` |
| `thresholds.escalateAt` | Dropdown + custom | `100`, `250`, `500`, `1000`, `Custom value...` |
| `acp.command` | Dropdown + custom | Dynamic: on-disk ACP server commands, `(unset)`, `Custom value...` |
| `acp.args` | Text input (comma-separated) | — |
| `acp.agentId` | Dropdown + custom | Dynamic: ACP agents list, `(unset)`, `Custom value...` |

Persistence: each edit writes immediately to `{librariansDir}/{name}.json`.

## Create Flow

Selecting `+ New librarian...` opens detail panel with defaults:
- name: `""` (must fill), topics: `["**"]`, all permissions: `true`
- thresholds: `{ topicComplexity: 100, escalateAt: 500 }`
- First edit to `name` creates the file on disk

## Delete Flow

Selecting `Delete librarian` shows inline confirmation (`Yes` / `No`). Deletes the JSON file, returns to list. Default librarian cannot be deleted.

## Rename Flow

Editing `name` deletes old file, creates new file. Conflicts show error and revert.

## "Notes" → "Volumes" Rename

All user-facing occurrences:

| Location | Change |
|---|---|
| `/notes` command | → `/volumes` (no alias) |
| `NoteList` component | → `VolumeList` |
| `NoteView` type | → `VolumeView` |
| `SearchResultView.note` | → `SearchResultView.volume` |
| `getNote()` | → `getVolume()` |
| `getAllNotes()` | → `getAllVolumes()` |
| `getNotesByTopic()` | → `getVolumesByTopic()` |
| `noteCount` | → `volumeCount` |
| User-facing strings | "notes" → "volumes" everywhere |
| Prompt tips | Updated to use "volume" terminology |
| Config descriptions | "Max notes per topic…" → "Max volumes per topic…" |

## Files to Create

- `simse-code/components/input/librarian-explorer.tsx` — LibrarianExplorer component
- `simse-code/features/library/librarian-commands.ts` — `/librarians` command definition

## Files to Modify

- `simse-code/app-ink.tsx` — Wire up LibrarianExplorer modal + command registration
- `simse-code/features/library/commands.ts` — Rename `/notes` → `/volumes`, update descriptions
- `simse-code/features/library/components.tsx` — Rename NoteList → VolumeList
- `simse-code/app.ts` — Rename NoteView → VolumeView, getNote → getVolume, etc.
- `simse-code/components/input/prompt-input.tsx` — Update placeholder tips
- `simse-code/config.ts` — Update config description strings
- `simse-code/features/config/settings-schema.ts` — Update memory.json field descriptions
- Related test files
