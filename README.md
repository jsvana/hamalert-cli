# hamalert-cli

A command-line interface for managing HamAlert triggers. This tool allows ham radio operators to programmatically add and manage callsign alerts through the HamAlert API.

## Description

hamalert-cli provides a simple CLI interface for interacting with HamAlert.org, allowing you to create triggers that notify you when specific callsigns appear. The tool handles authentication and trigger management through a convenient command-line interface.

## Installation

### Prerequisites

- Rust and Cargo (install from [rustup.rs](https://rustup.rs))
- A HamAlert.org account

### Building from Source

```bash
git clone <repository-url>
cd hamalert-sub
cargo build --release
```

The compiled binary will be available at `target/release/hamalert-cli`.

Optionally, install it to your PATH:

```bash
cargo install --path .
```

### Agent Skill

This repository includes an agent skill for using the CLI safely:

```bash
npx skills add jsvana/hamalert-cli
```

## Configuration

Run the managed login flow:

```bash
hamalert-cli auth login
```

The command validates your HamAlert credentials, stores your password in the OS keyring when available, and writes your username to `~/.config/hamalert/config.toml`.

Preferred config:

```toml
username = "your_username"
```

On systems without a usable keyring backend, such as some headless Linux or SSH environments, hamalert-cli falls back to a legacy plaintext password in the config file:

```toml
username = "your_username"
password = "your_password"
```

### Custom Configuration Path

You can specify a different configuration file location using the `--config-file` flag:

```bash
hamalert-cli --config-file /path/to/config.toml <command>
```

## Commands

### auth

Manage stored HamAlert credentials.

#### auth login

Validate and store credentials:

```bash
hamalert-cli auth login
```

Interactive login prompts for a username and password. If the config file already has a username, it is used as the prompt default. After successful validation, the password is stored in the OS keyring when available. If keyring storage succeeds, any plaintext `password` is removed from the active config file.

Non-interactive login is also supported:

```bash
printf '%s\n' "$HAMALERT_PASSWORD" | hamalert-cli auth login --username N0CALL --password-stdin
hamalert-cli auth login --username N0CALL --password-env HAMALERT_PASSWORD
hamalert-cli auth login --username N0CALL --password "your_password"
```

Prefer `--password-stdin` or `--password-env` for scripts. `--password` is convenient but can expose the password through shell history or process inspection.

#### auth status

Show credential storage status and verify the resolved credentials against HamAlert:

```bash
hamalert-cli auth status
```

The status output reports the config path, username state, keyring availability, whether keyring/config passwords exist, which credential source would be used, and whether HamAlert login succeeds. It never prints password values.

#### auth logout

Remove locally stored credentials:

```bash
hamalert-cli auth logout
```

This deletes the keyring password when present and removes any plaintext `password` fallback from the active config file. The configured username is kept. There is no persistent HamAlert server session to revoke.

### add-trigger

Add callsign triggers with specified actions. Multiple callsigns create a single trigger with comma-separated callsigns:

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --callsign K3LR \
  --comment "Monitor activity" \
  --actions app
```

#### Available Actions

- `url` - HTTP/webhook notification
- `app` - Mobile app notification
- `threema` - Threema messenger notification
- `telnet` - Telnet cluster notification

#### Available Modes

Optionally filter by transmission mode:

- `cw` - CW (Morse code)
- `ft8` - FT8 digital mode
- `ssb` - SSB (Single Side Band)

### import-polo-notes

Import callsigns from a Ham2K PoLo notes file hosted at a URL:

```bash
hamalert-cli import-polo-notes \
  --url https://example.com/callsigns.txt \
  --comment "PoLo imports" \
  --actions app \
  --dry-run  # Preview without creating triggers
```

### import-file

Import callsigns from a local text file:

```bash
hamalert-cli import-file \
  --file callsigns.txt \
  --comment "Local imports" \
  --actions app \
  --dry-run  # Preview without creating triggers
```

#### File Format

One callsign per line. Additional text after the callsign is ignored (useful for notes):

```
W1AW
K3LR friend from contest
N0CALL met at hamfest
```

Empty lines, lines starting with `#`, and lines starting with `//` are skipped:

```
# Friends list
W1AW
K3LR

// DX stations
VP8LP
```

### backup

Export all triggers to a JSON file:

```bash
hamalert-cli backup
# Creates: hamalert-backup-YYYY-MM-DD.json

hamalert-cli backup --output my-triggers.json
```

### restore

Restore triggers from a backup file. Dry-run by default for safety:

```bash
# Preview what would happen
hamalert-cli restore --input hamalert-backup-2025-01-15.json

# Actually restore (creates auto-backup first, then replaces all triggers)
hamalert-cli restore --input hamalert-backup-2025-01-15.json --no-dry-run
```

### edit

Interactively edit an existing trigger using your `$EDITOR`:

```bash
hamalert-cli edit
# Shows numbered list of triggers
# Opens selected trigger in your editor
# Saves changes back to HamAlert
```

### set-callsigns

Replace the callsign list of an existing trigger with the contents of a file (one callsign per line, `#` comments allowed). Only `conditions.callsign` changes; every other condition, action, comment and option is kept, and the trigger is updated in place:

```bash
hamalert-cli set-callsigns --trigger-id <id> --file callsigns.txt               # Dry run: shows added/removed
hamalert-cli set-callsigns --trigger-id <id> --file callsigns.txt --no-dry-run  # Apply
hamalert-cli set-callsigns --comment "SOTA spots" --file callsigns.txt          # Select by exact comment
```

Trigger IDs (`_id`) are listed in `backup` output. As a safeguard, the update is refused if the file is empty or if more callsigns would be removed than `--max-removals` (default: 10% of the current list, minimum 5).

### bulk-delete

Interactively delete multiple triggers with a TUI multi-select interface:

```bash
hamalert-cli bulk-delete
# Navigate: j/k or arrows
# Toggle: Space (unchecked = will be deleted)
# Confirm: Enter
# Cancel: Esc

hamalert-cli bulk-delete --dry-run  # Preview without deleting
```

All triggers start checked (kept). Uncheck the ones you want to delete. An auto-backup is created before deletion.

### profile

Manage trigger profiles for different locations or activities.

#### profile list

Show all available profiles with match percentages:

```bash
hamalert-cli profile list
```

#### profile show

Display triggers in a specific profile:

```bash
hamalert-cli profile show home
```

#### profile status

Analyze current HamAlert triggers against saved profiles:

```bash
hamalert-cli profile status
```

#### profile save

Save current triggers (excluding permanent ones) as a profile:

```bash
hamalert-cli profile save home
hamalert-cli profile save portable --from-backup backup.json
```

#### profile switch

Switch to a different profile (dry-run by default):

```bash
hamalert-cli profile switch portable           # Preview
hamalert-cli profile switch portable --no-dry-run  # Execute
```

#### profile delete

Remove a saved profile:

```bash
hamalert-cli profile delete old-profile
```

#### profile set-permanent

Interactively select which triggers should be permanent (always active):

```bash
hamalert-cli profile set-permanent
hamalert-cli profile set-permanent --from-backup backup.json
```

#### profile show-permanent

Display current permanent triggers:

```bash
hamalert-cli profile show-permanent
```

## Examples

Monitor a specific callsign with app notifications:

```bash
hamalert-cli add-trigger --callsign K3LR --comment "K3LR spotted" --actions app
```

Add multiple callsigns (creates one trigger with comma-separated callsigns):

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --callsign K3LR \
  --callsign DX1DX \
  --comment "Multiple DX stations" \
  --actions app
```

Monitor a callsign only for FT8 activity:

```bash
hamalert-cli add-trigger \
  --callsign VP8LP \
  --comment "FT8 only" \
  --actions app \
  --mode ft8
```

Backup, clean up, and restore workflow:

```bash
# Backup current triggers
hamalert-cli backup

# Interactively delete unwanted triggers
hamalert-cli bulk-delete

# If something went wrong, restore from backup
hamalert-cli restore --input hamalert-backup-2025-01-15.json --no-dry-run
```

## License

MIT License - see [LICENSE](LICENSE) file for details.
