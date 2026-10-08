# Command Reference

Use `hamalert-cli <command> --help` for exact syntax. If no installed binary is available and the current directory is this repository, use `cargo run -- <command>`.

## Global

```bash
hamalert-cli [--config-file <CONFIG_FILE>] <COMMAND>
```

`--config-file` points at a TOML config file. Without it, the CLI uses the platform config directory, normally `~/.config/hamalert/config.toml`.

## auth

```bash
hamalert-cli auth login [--username <CALLSIGN>] [--password-stdin | --password-env <VAR> | --password <PASSWORD>]
hamalert-cli auth status
hamalert-cli auth logout
```

- `auth login` validates credentials and stores the password in the OS keyring when possible.
- `auth status` is read-only for local files and prints credential source/status without password values, but it does attempt a live HamAlert login when credentials resolve.
- `auth logout` removes the stored local password and keeps the username.

Prefer `--password-stdin` or `--password-env` for automated use.

## add-trigger

```bash
hamalert-cli add-trigger --callsign W1AW --comment "W1AW spotted" --actions app
hamalert-cli add-trigger --callsign W1AW --callsign K3LR --comment "Monitor activity" --actions app telnet
hamalert-cli add-trigger --callsign VP8LP --comment "FT8 only" --actions app --mode ft8
```

`add-trigger` modifies live HamAlert state. Multiple `--callsign` values are sent as one trigger by joining callsigns. The default separator is comma-space. Use `--compact` for comma-only or `--one-per-line` for newline-separated callsigns.

Actions: `url`, `app`, `threema`, `telnet`.
Modes: `cw`, `ft8`, `ssb`.

## import-polo-notes

```bash
hamalert-cli import-polo-notes --url <URL> --comment "PoLo imports" --actions app --dry-run
hamalert-cli import-polo-notes --url <URL> --comment "PoLo imports" --actions app
```

Imports callsigns from a Ham2K PoLo notes URL. Use `--dry-run` first. Dry-run still requires configured credentials because the CLI logs in before running non-auth commands.

## import-file

```bash
hamalert-cli import-file --file callsigns.txt --comment "Local imports" --actions app --dry-run
hamalert-cli import-file --file callsigns.txt --comment "Local imports" --actions app
```

Imports callsigns from a local text file. Use `--dry-run` first. See `data-formats.md` for file parsing rules.

## backup

```bash
hamalert-cli backup
hamalert-cli backup --output my-triggers.json
```

Fetches all live triggers and writes JSON. Without `--output`, the CLI writes to the HamAlert backup directory under the platform data directory, normally `~/.local/share/hamalert/backups/`.

## restore

```bash
hamalert-cli restore --input hamalert-backup-2026-07-08.json
hamalert-cli restore --input hamalert-backup-2026-07-08.json --no-dry-run
```

Dry-run is the default and previews replacing all existing triggers. `--no-dry-run` deletes existing triggers, creates an automatic backup first, and recreates triggers from the backup file.

## edit

```bash
hamalert-cli edit
```

Fetches triggers, asks the user to choose one, opens editable JSON in `$EDITOR` or `vi`, validates JSON, and updates the live trigger if changed.

## set-callsigns

```bash
hamalert-cli set-callsigns --trigger-id <id> --file callsigns.txt
hamalert-cli set-callsigns --comment "<exact comment>" --file callsigns.txt --no-dry-run
```

Replaces the callsign list of one existing trigger with the file's contents (one callsign per line, `#` comments allowed), updating it in place. Only `conditions.callsign` changes, written back in its original string or array form; other conditions, actions, comment, and options are kept. Dry-run by default, printing added and removed callsigns. Refuses an empty file, or more removals than `--max-removals` (default: 10% of the current list, minimum 5). After saving, it re-fetches the trigger and errors if the saved list does not match. Trigger IDs (`_id`) appear in `backup` output.

## bulk-delete

```bash
hamalert-cli bulk-delete --dry-run
hamalert-cli bulk-delete
```

Interactive TUI. All triggers start selected, meaning kept. Unchecked triggers are deleted. `--dry-run` still fetches live triggers and asks for a selection before previewing.

## profile

```bash
hamalert-cli profile list
hamalert-cli profile show <name>
hamalert-cli profile status
hamalert-cli profile save <name>
hamalert-cli profile save <name> --from-backup <file>
hamalert-cli profile switch <name>
hamalert-cli profile switch <name> --no-dry-run
hamalert-cli profile delete <name>
hamalert-cli profile set-permanent
hamalert-cli profile set-permanent --from-backup <file>
hamalert-cli profile show-permanent
```

Profiles manage saved trigger sets for different locations or activities. `profile switch` is dry-run by default; `--no-dry-run` replaces non-permanent live triggers with the target profile and creates an automatic backup first.
