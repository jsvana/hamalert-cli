# Workflows

## Check Local Setup

```bash
skills/hamalert-cli/scripts/local-check.sh
hamalert-cli auth status
```

`auth status` verifies resolved credentials against HamAlert if credentials are available. It does not print password values.

## First-Time Login

Interactive:

```bash
hamalert-cli auth login
```

Non-interactive with stdin:

```bash
printf '%s\n' "$HAMALERT_PASSWORD" | hamalert-cli auth login --username N0CALL --password-stdin
```

Non-interactive with environment variable:

```bash
hamalert-cli auth login --username N0CALL --password-env HAMALERT_PASSWORD
```

Avoid `--password` unless the user accepts shell history and process-list exposure.

## Add a Callsign Alert

Confirm the requested callsign, comment, actions, and optional modes, then run:

```bash
hamalert-cli add-trigger --callsign W1AW --comment "W1AW spotted" --actions app
```

`add-trigger` changes live HamAlert state and has no dry-run mode.

## Add Multiple Callsigns as One Trigger

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --callsign K3LR \
  --comment "Monitor activity" \
  --actions app telnet
```

The CLI sends one trigger with the callsigns as a JSON array.

## Set a Minimum SNR

Create a trigger for FT8 spots at or above −15 dB.

```bash
hamalert-cli add-trigger --callsign W1AW --comment "W1AW FT8" \
  --actions app --mode ft8 --snr-from=-15
```

To change the minimum on an existing trigger, use its exact `_id` from a backup or choose it in the `edit` picker.

```bash
hamalert-cli edit --trigger-id "$TRIGGER_ID" --snr-from=-10
```

Both commands use the default maximum when `--snr-to` is omitted. For other conditions and options, use the [trigger settings JSON](data-formats.md#trigger-settings-json).

## Import a Local Callsign File

Preview:

```bash
hamalert-cli import-file --file callsigns.txt --comment "Local imports" --actions app --dry-run
```

Execute after approval:

```bash
hamalert-cli import-file --file callsigns.txt --comment "Local imports" --actions app
```

## Import Ham2K PoLo Notes

Preview:

```bash
hamalert-cli import-polo-notes --url https://example.com/callsigns.txt --comment "PoLo imports" --actions app --dry-run
```

Execute after approval:

```bash
hamalert-cli import-polo-notes --url https://example.com/callsigns.txt --comment "PoLo imports" --actions app
```

## Back Up Triggers

```bash
hamalert-cli backup
```

Tell the user the output path printed by the command. When no `--output` is given, backups are written under the platform data directory, normally `~/.local/share/hamalert/backups/`.

## Restore Triggers

Always preview first:

```bash
hamalert-cli restore --input <backup.json>
```

Execute only with explicit approval:

```bash
hamalert-cli restore --input <backup.json> --no-dry-run
```

Live restore deletes all existing triggers after creating an automatic backup.

## Clean Up Triggers

Preview the interactive selection:

```bash
hamalert-cli bulk-delete --dry-run
```

Execute only with explicit approval:

```bash
hamalert-cli bulk-delete
```

All entries begin checked, meaning kept. Unchecked entries are the deletion set.

## Use Profiles

Inspect state:

```bash
hamalert-cli profile list
hamalert-cli profile status
hamalert-cli profile show-permanent
```

Save current non-permanent triggers as a profile:

```bash
hamalert-cli profile save home
```

Preview a switch:

```bash
hamalert-cli profile switch portable
```

Execute a switch only with explicit approval:

```bash
hamalert-cli profile switch portable --no-dry-run
```

Select permanent triggers:

```bash
hamalert-cli profile set-permanent
```

Permanent triggers are excluded from profiles and remain active across profile switches.
