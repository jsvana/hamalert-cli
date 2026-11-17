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
hamalert-cli add-trigger --callsign VP8LP --comment "FT8 spots" --actions app --mode ft8 --snr-from -24 --snr-to -5
hamalert-cli add-trigger --editor
hamalert-cli add-trigger --from-json trigger.json
hamalert-cli add-trigger --from-json - < trigger.json
```

`add-trigger` modifies live HamAlert state. Choose flags, `--editor`, or `--from-json` for trigger settings. `--editor` opens a JSON template with empty conditions, actions, and comment. `--from-json` accepts a file or `-` for standard input. See [Trigger settings JSON](data-formats.md#trigger-settings-json).

The `--actions` values are `url`, `app`, `threema`, and `telnet`. The `--mode` values are `cw`, `ft8`, and `ssb`.

Use `--snr-from` to set a minimum SNR in whole dB. Omit `--snr-to` for the default maximum.

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

`add-trigger` and both imports send multiple callsigns or modes as arrays in a single trigger. The legacy `--compact` and `--one-per-line` flags are accepted with a deprecation warning.

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

Dry-run is the default and previews replacing all existing triggers. `--no-dry-run` creates an automatic backup, deletes existing triggers, and recreates triggers from the backup file. Recreated triggers are enabled, even if they were paused in the backup.

## edit

```bash
hamalert-cli edit
hamalert-cli edit --trigger-id "$TRIGGER_ID"
hamalert-cli edit --trigger-id "$TRIGGER_ID" --snr-from -24 --snr-to -5
hamalert-cli edit --trigger-id "$TRIGGER_ID" --clear-snr
hamalert-cli edit --trigger-id "$TRIGGER_ID" --from-json trigger.json
hamalert-cli edit --trigger-id "$TRIGGER_ID" --from-json - < trigger.json
```

Without a settings flag or `--from-json`, opens the selected trigger's current settings in `$EDITOR` or `vi`. To skip the picker, pass the trigger's `_id` from a backup with `--trigger-id`. Standard input with `--from-json -` requires `--trigger-id`.

`--comment`, `--actions`, `--callsign`, `--mode`, and the SNR flags update the supplied fields. `--clear-snr` removes both SNR bounds. The editor and `--from-json` replace the full editable settings. Omit a condition or option key from the document to remove it. See [Trigger settings JSON](data-formats.md#trigger-settings-json).

## pause and resume

```bash
hamalert-cli pause [--trigger-id <TRIGGER_ID>]
hamalert-cli resume [--trigger-id <TRIGGER_ID>]
```

`pause` disables a live trigger and `resume` enables it. Both preserve the trigger's settings. Without `--trigger-id`, choose one enabled trigger to pause or one paused trigger to resume. Press Esc to cancel. With `--trigger-id`, use the exact `_id` from a backup or the picker. If the trigger is already in the requested state, the command reports it without sending an update.

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
