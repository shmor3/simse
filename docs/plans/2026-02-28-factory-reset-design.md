# Factory Reset Commands — Design

## Commands

| Command | Category | Deletes |
|---------|----------|---------|
| `/factory-reset` | `config` | Everything in `~/.simse/` (global configs, sessions, permissions) |
| `/factory-reset-project` | `config` | `.simse/` dir + `SIMSE.md` in current working directory |

## Confirmation UX

Both commands use a new `<ConfirmDialog>` component (Promise-based modal like `PermissionDialog`/`SetupSelector`):

1. Command fires, dialog appears with two options in an arrow-key list
2. **"No, cancel" (selected by default)** — pressing Enter cancels immediately
3. **"Yes, delete everything"** — user arrows down to select, must type `yes` into a text input, then press Enter to confirm
4. On confirm: recursively delete the target directory, show success message
5. On cancel: show "Cancelled" message

## New Files

- `simse-code/components/input/confirm-dialog.tsx` — reusable `<ConfirmDialog>` with arrow-key list, text input gate on "Yes" option
- `simse-code/features/config/reset.ts` — `createResetCommands(dataDir, workDir, onConfirm)` factory

## Modified Files

- `simse-code/app-ink.tsx` — add `pendingConfirm` state, `handleConfirm` callback, render `<ConfirmDialog>`, wire commands via `createResetCommands`

## Data Flow

```
/factory-reset → createResetCommands handler → onConfirm(message) → Promise<boolean>
  ↓                                                ↓
app-ink.tsx sets pendingConfirm state         <ConfirmDialog> renders
  ↓                                                ↓
user selects No (Enter) → resolve(false)     user selects Yes, types "yes", Enter → resolve(true)
  ↓                                                ↓
handler receives false → "Cancelled"         handler receives true → rm -rf dir → success msg
```
