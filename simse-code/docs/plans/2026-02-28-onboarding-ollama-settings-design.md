# Interactive Onboarding, Ollama Setup & Settings Explorer

## Problem

1. When simse runs without config files, the onboarding flow uses readline-based prompts that don't match the Ink React UI. The ollama preset writes defaults silently without letting the user configure URL, test the connection, or pick from available models.
2. There is no way to interactively browse and edit simse settings after initial setup.

## Features

### Feature 1: Ink-Based Onboarding Wizard

Replaces the readline-based `setup.ts` flow. Launches inside the Ink app when no `~/.simse/acp.json` exists.

**Steps:**

1. **ACP Provider Selection** — Arrow-key preset selector (reuses/extends existing `SetupSelector` pattern)
   - simse-engine, Ollama, Claude Code, GitHub Copilot, Custom
   - Selecting Ollama triggers the ollama sub-wizard (Feature 2)
   - Selecting simse-engine prompts for model/device
   - Selecting Custom opens text input

2. **Summarization Provider** — Three options
   - "Same as main provider" (Recommended, pre-selected)
   - "Different provider" (re-shows provider selector)
   - "Skip"

3. **Embedding Model** — Arrow-key selection
   - Small: Snowflake/snowflake-arctic-embed-xs (22M params)
   - Medium: nomic-ai/nomic-embed-text-v1.5 (137M params) (Recommended, pre-selected)
   - Large: Snowflake/snowflake-arctic-embed-l (335M params)
   - TEI server (prompts for URL)

4. **Library/Memory Settings** — Two paths
   - "Use recommended defaults" (Recommended) — skips to next step
   - "Customize" — field-by-field editing:
     - enabled: boolean toggle (default: true)
     - similarityThreshold: number (default: 0.7)
     - maxResults: number (default: 10)
     - autoSummarizeThreshold: number (default: 20)

5. **Log Level** — Arrow-key selection
   - debug, info, warn (Recommended), error, none

6. **Review & Confirm** — Summary of all settings
   - Enter to write all files atomically
   - Esc or Back to revisit steps
   - Shows list of files that will be created

**New files:**
- `components/input/onboarding-wizard.tsx` — Multi-step wizard component
- `features/config/onboarding.ts` — Onboarding logic, file writing, step definitions

**Modified files:**
- `app-ink.tsx` — Detect missing config, show onboarding wizard instead of banner
- `cli-ink.tsx` — Remove/bypass old readline-based `runSetup()` when Ink app handles onboarding

### Feature 2: Enhanced Ollama Sub-Wizard

A sub-flow triggered when Ollama is selected from either the onboarding wizard or `/setup ollama`.

**Steps:**

1. **URL Input** — TextInput with default `http://127.0.0.1:11434`

2. **Connection Test** — Automatic after URL entry
   - Spinner: "Testing connection to http://..."
   - HTTP GET to `<url>/api/tags`
   - **Success path**: Green checkmark, show ollama version if available, proceed to model selection
   - **Failure path**: Red error message, three options:
     - Retry — test again
     - Change URL — go back to step 1
     - Ignore — proceed with manual model name input

3. **Model Selection** (success path only)
   - Parse `/api/tags` response for model names + sizes
   - Arrow-key selectable list
   - If no models found, fall back to text input with default `llama3.2`

4. **Returns** `{ url, model }` to the parent wizard/setup command

**New files:**
- `components/input/ollama-wizard.tsx` — Ollama-specific sub-wizard component
- `features/config/ollama-test.ts` — Connection testing logic (`testOllamaConnection`, `listOllamaModels`)

**Modified files:**
- `features/config/setup.ts` — Ollama preset triggers sub-wizard instead of writing defaults
- `setup.ts` — Ollama readline preset gets connection testing added to `build()`

### Feature 3: Interactive Settings Explorer (`/settings`)

New `/settings` command renders an interactive Ink component for browsing and editing all config files.

**UI Layout:**
- File list (left/top): acp.json, config.json, embed.json, memory.json, mcp.json, summarize.json, .simse/settings.json
- Field list (right/below): shows fields of selected file with current values
- Navigation: arrow keys between files and fields, Enter to edit, Esc to cancel

**Field editing by type:**
- String → TextInput
- Number → TextInput with numeric validation
- Boolean → toggle on Enter
- Enum (e.g., log level) → cycle through options on Enter

**Behavior:**
- Changes write to disk immediately
- Shows confirmation message on save
- Read-only display for complex nested objects (e.g., ACP server args array)
- For arrays like `acp.servers`, show each server as a navigable sub-item

**New files:**
- `components/input/settings-explorer.tsx` — Main settings UI component
- `features/config/settings-editor.ts` — File I/O, schema definitions (which fields exist per file, types, defaults, descriptions)

**Modified files:**
- `features/config/commands.ts` — Replace stub `/settings` command with interactive explorer
- `app-ink.tsx` — Add pending state for settings explorer modal (same Promise-based pattern as setup selector)

## Component Architecture

```
OnboardingWizard
  ├── Step indicator (1/6, 2/6, etc.)
  ├── SetupSelector (step 1 — reused)
  │   └── OllamaWizard (sub-flow)
  │       ├── TextInput (URL)
  │       ├── Spinner (connection test)
  │       └── ModelSelector (arrow list)
  ├── SummarizeSelector (step 2)
  ├── EmbeddingSelector (step 3)
  ├── LibrarySettings (step 4)
  ├── LogLevelSelector (step 5)
  └── ReviewConfirm (step 6)

SettingsExplorer
  ├── FileList (left column)
  └── FieldEditor (right column)
      ├── TextInput (strings/numbers)
      ├── Toggle (booleans)
      └── Cycle (enums)
```

## Patterns

- All new components use `useInput` + `useState` (no classes)
- Promise-based modal pattern from `app-ink.tsx` for wizard/explorer overlays
- Factory functions for any non-component logic
- Wizard state is a simple `{ step, data }` object managed with `useState`
- Connection testing uses `fetch()` with a timeout (3s default)
- File writes use existing `writeFileSync` pattern with JSON.stringify + newline

## Testing

- Unit tests for `ollama-test.ts` (mock fetch, test success/failure/timeout)
- Unit tests for `settings-editor.ts` (schema validation, field type detection)
- Integration tests for onboarding file writing (temp directory, verify all files created)
