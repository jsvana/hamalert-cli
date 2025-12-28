# Backup, Restore, and Edit Triggers Design

## Overview

Add three new commands to hamalert-cli and modify the existing callsign handling behavior.

## Changes

### 1. Comma-Separated Callsigns

**Current**: Multiple `--callsign` flags create separate triggers (one API call each).

**New**: Multiple `--callsign` flags create a single trigger with comma-separated callsigns (one API call total).

```bash
# Creates ONE trigger with callsign condition "W1ABC,K2DEF,N3GHI"
hamalert-cli add-trigger --callsign W1ABC --callsign K2DEF --callsign N3GHI --comment "Friends" --actions app
```

### 2. Backup Command

```bash
hamalert-cli backup [--output <file>]
```

- Fetches all triggers via `GET /ajax/triggers`
- Saves raw JSON to file
- Default filename: `hamalert-backup-YYYY-MM-DD.json`

### 3. Restore Command

```bash
hamalert-cli restore --input <file> [--no-dry-run]
```

**Dry-run (default)**:
- Shows what would happen
- Lists triggers that would be restored
- Prints: "This will DELETE all existing triggers and restore N triggers from backup"

**With `--no-dry-run`**:
1. Auto-backup current triggers to `hamalert-backup-before-restore-YYYY-MM-DD-HHMMSS.json`
2. Delete all existing triggers via `POST /ajax/trigger_delete`
3. Create each trigger from backup via `POST /ajax/trigger_update` (strips `_id`/`user_id`)

### 4. Edit Command

```bash
hamalert-cli edit
```

Interactive workflow:
1. Fetch and display numbered list of triggers
2. User selects trigger by number
3. Open trigger JSON in `$EDITOR` (falls back to `vi`)
4. On save: validate JSON, POST update if changed
5. On invalid JSON: show error, offer to re-edit

Display format:
```
1. [CW] AC3NJ, K2DEF - "CW Innovations classmate"
2. [any] KB8W - "Danny Miller is live"
Select trigger to edit (1-3), or q to quit:
```

Editable fields: `conditions`, `actions`, `comment`

## API Endpoints Used

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/ajax/triggers` | GET | Fetch all triggers |
| `/ajax/trigger_update` | POST | Create or update trigger |
| `/ajax/trigger_delete` | POST | Delete trigger by ID |

## Dependencies

No new dependencies required. Uses existing reqwest, serde_json, and std.
