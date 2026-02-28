# Smart Settings Dropdowns

## Problem

The settings explorer has 13 plain text input fields where users must type exact values (server names, agent IDs, model names, numeric thresholds). Most of these values are discoverable from on-disk config files or known presets, making text inputs unnecessary friction.

## Solution

Convert 10 text-input fields to populated dropdowns. Add dynamic resolvers that read local JSON files to populate options. Add inline sub-flows (SetupSelector/OllamaWizard) for "Add new server..." actions. Add preset dropdowns for number fields.

## Schema Changes

Extend `FieldSchema` with two new optional properties:

- **`presets?: readonly string[]`** — Static preset values shown as dropdown options for string/number fields. "Custom value..." appended automatically.
- **`resolve?: 'acp-servers' | 'agents' | 'embedding-models'`** — Dynamically populate dropdown at render time from on-disk data.

## Dynamic Resolvers

Three sync resolvers (no API calls):

1. **`acp-servers`** — Reads `acp.json`, extracts `servers[].name`. Appends "Add new server..." which triggers inline SetupSelector.
2. **`agents`** — Reads `acp.json` servers to derive agent IDs (`defaultAgent ?? name`), plus scans `.simse/agents/*.md` filenames.
3. **`embedding-models`** — Returns 3 known presets: `snowflake-arctic-embed-xs (22M)`, `nomic-embed-text-v1.5 (137M)`, `snowflake-arctic-embed-l (335M)`.

## Field Mapping

| Field | Config File | Edit Mode | Source |
|-------|-------------|-----------|--------|
| logLevel | config.json | enum dropdown | Static options (existing) |
| defaultAgent | config.json | dynamic dropdown | `resolve: 'agents'` |
| perplexityApiKey | config.json | text input | Secret |
| githubToken | config.json | text input | Secret |
| defaultServer | acp.json | dynamic dropdown | `resolve: 'acp-servers'` |
| embeddingModel | embed.json | dynamic dropdown | `resolve: 'embedding-models'` |
| dtype | embed.json | enum dropdown | Static options (existing) |
| teiUrl | embed.json | text input | URL |
| enabled | memory.json | boolean dropdown | Existing |
| similarityThreshold | memory.json | preset dropdown | `presets: ['0.5', '0.6', '0.7', '0.8', '0.9']` |
| maxResults | memory.json | preset dropdown | `presets: ['5', '10', '20', '50']` |
| autoSummarizeThreshold | memory.json | preset dropdown | `presets: ['0', '10', '20', '50']` |
| duplicateThreshold | memory.json | preset dropdown | `presets: ['0', '0.8', '0.85', '0.9', '0.95']` |
| duplicateBehavior | memory.json | enum dropdown | Static options (existing) |
| server | summarize.json | dynamic dropdown | `resolve: 'acp-servers'` |
| command | summarize.json | text input | Shell command |
| agent | summarize.json | dynamic dropdown | `resolve: 'agents'` |
| defaultAgent | settings.json | dynamic dropdown | `resolve: 'agents'` |
| logLevel | settings.json | enum dropdown | Static options (existing) |
| systemPrompt | settings.json | text input | Free-form |
| defaultServer | settings.json | dynamic dropdown | `resolve: 'acp-servers'` |
| conversationTopic | settings.json | text input | Free-form |
| chainTopic | settings.json | text input | Free-form |

**Result: 10 new dropdowns, only 7 text inputs remain** (5 free-form + 2 secrets).

## Inline Sub-Flow: "Add new server..."

When "Add new server..." is selected from any `acp-servers` dropdown:

1. Settings explorer enters `'setup-flow'` edit mode
2. Renders `<SetupSelector>` inline (replacing the field list)
3. If user selects Ollama, `<OllamaWizard>` runs inline
4. On completion: writes to `acp.json`, reloads file data, returns to field list with new server pre-selected
5. On dismiss (Esc): returns to field list, no changes

## Files Changed

- **`settings-schema.ts`** — Add `presets` and `resolve` to `FieldSchema`, add values to field definitions
- **`settings-explorer.tsx`** — Add resolver functions, modify `buildDropdownOptions` to check presets/resolve, add `'setup-flow'` edit mode for inline SetupSelector
