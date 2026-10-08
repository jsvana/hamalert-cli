# Safety Rules

## Credential Safety

- Never print passwords or ask the user to paste a password into chat.
- Prefer `auth login --password-stdin` or `auth login --password-env`.
- Avoid `auth login --password` unless the user explicitly accepts shell history and process-list exposure.
- `auth status` does not print passwords, but it may attempt a live login with resolved credentials.

## Read-Only or Mostly Read-Only Commands

These do not intentionally modify live HamAlert triggers:

```bash
hamalert-cli auth status
hamalert-cli backup
hamalert-cli restore --input <file>
hamalert-cli import-file --file <file> ... --dry-run
hamalert-cli import-polo-notes --url <url> ... --dry-run
hamalert-cli bulk-delete --dry-run
hamalert-cli set-callsigns --trigger-id <id> --file <file>
hamalert-cli profile list
hamalert-cli profile show <name>
hamalert-cli profile status
hamalert-cli profile show-permanent
hamalert-cli profile switch <name>
```

Important caveats:

- Non-auth commands still log in before they run.
- Dry-runs often fetch live triggers.
- `profile status` may offer to update local profile bookkeeping.
- `profile switch <name>` dry-run may offer to save unexpected triggers into a local profile before exiting.

## Commands That Modify Live HamAlert

Ask for explicit approval before running these unless approval is already clear:

```bash
hamalert-cli add-trigger ...
hamalert-cli import-file ...            # without --dry-run
hamalert-cli import-polo-notes ...      # without --dry-run
hamalert-cli restore --input <file> --no-dry-run
hamalert-cli edit
hamalert-cli bulk-delete                # without --dry-run
hamalert-cli set-callsigns ... --no-dry-run
hamalert-cli profile switch <name> --no-dry-run
```

## Commands That Modify Local State

These change local config/profile files:

```bash
hamalert-cli auth login
hamalert-cli auth logout
hamalert-cli profile save <name>
hamalert-cli profile delete <name>
hamalert-cli profile set-permanent
hamalert-cli profile switch <name> --no-dry-run
```

## Backup Expectations

- Run `hamalert-cli backup` before live destructive changes when practical.
- `restore --no-dry-run`, `bulk-delete`, and `profile switch --no-dry-run` create automatic backups before destructive live changes.
- Report the printed backup path to the user.

## Interaction Safety

- For `bulk-delete`, all triggers start checked, meaning kept. Unchecked triggers are deleted.
- For `profile set-permanent`, checked triggers become permanent.
- If the user is not present for an interactive TUI, stop and ask for input rather than guessing.
