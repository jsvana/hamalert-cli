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

## Configuration

Create a configuration file at `~/.config/hamalert/config.toml` with your HamAlert credentials:

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

Actions need to be added individually:

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --comment "Monitor activity" \
  --actions app \
  --actions telnet
```

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
