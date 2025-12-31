# Profiles Feature Design

## Overview

Profiles allow users to maintain different sets of triggers for different locations or activities. Some triggers are "permanent" (always active regardless of profile), while others are profile-specific and get swapped when switching profiles.

## Use Cases

- Operating from home vs portable/POTA locations
- Different propagation patterns at different locations
- Different activities requiring different alert sets

## File Structure

```
~/.local/share/hamalert/
├── backups/                    # Existing backup location
│   └── hamalert-backup-*.json
├── permanent.json              # Triggers active in ALL profiles
├── current-profile             # Plain text file with current profile name
└── profiles/
    ├── home.json
    └── portable.json
```

### File Formats

`permanent.json` and profile files use the same format as backups - an array of trigger objects:

```json
[
  {
    "conditions": { "callsign": "KB8W" },
    "actions": ["app"],
    "comment": "Danny Miller is live"
  }
]
```

The `_id` and `user_id` fields are omitted since triggers get new IDs when created on HamAlert.

## Commands

```
hamalert-cli profile list
hamalert-cli profile show <name>
hamalert-cli profile status
hamalert-cli profile save <name> [--from-backup <file>]
hamalert-cli profile switch <name> [--no-dry-run]
hamalert-cli profile delete <name>
hamalert-cli profile set-permanent [--from-backup <file>]
hamalert-cli profile show-permanent
```

| Command | Description |
|---------|-------------|
| `list` | Show all profiles with match percentages, mark active with `*` |
| `show <name>` | Display triggers in a profile |
| `status` | Analyze current HamAlert triggers vs profiles, offer actions on mismatch |
| `save <name>` | Save current non-permanent triggers as profile |
| `save <name> --from-backup` | Create profile from existing backup file |
| `switch <name>` | Preview switch (dry-run default) |
| `switch <name> --no-dry-run` | Execute the switch |
| `delete <name>` | Remove a profile |
| `set-permanent` | Interactive TUI to select permanent triggers |
| `set-permanent --from-backup` | Set permanent triggers from a backup file |
| `show-permanent` | Display current permanent triggers |

## Workflows

### Profile Switch

`profile switch <name>` workflow:

1. **Load data**
   - Fetch current triggers from HamAlert
   - Load `permanent.json` (empty array if doesn't exist)
   - Load target profile file
   - Load `current-profile` to know previous profile

2. **Categorize current triggers** (match by `conditions` + `comment`):
   - Permanent: Match `permanent.json` - leave alone
   - Profile triggers: Match previous profile - will be replaced
   - Unexpected: Match neither - need user decision

3. **Dry-run output**
   ```
   Current profile: home
   Switching to: portable

   Permanent triggers (unchanged): 3
   Will DELETE 5 triggers from 'home'
   Will CREATE 4 triggers from 'portable'

   Found 2 unexpected triggers:
     → [D]elete / [S]ave to 'home' / [C]ancel?

   Run with --no-dry-run to execute.
   ```

4. **Execute** (with `--no-dry-run`):
   - Create timestamped backup
   - Delete non-permanent triggers
   - Create triggers from target profile
   - Update `current-profile` file

### Profile Save

**From current state:** `profile save <name>`

1. Fetch current triggers from HamAlert
2. Load `permanent.json`
3. Filter out permanent triggers
4. Save remaining to `profiles/<name>.json`
5. Update `current-profile` to `<name>`
6. If profile exists, prompt before overwriting

**From backup file:** `profile save <name> --from-backup <file>`

1. Load backup file
2. Filter out triggers matching `permanent.json`
3. Save remaining to `profiles/<name>.json`
4. Does NOT set as current profile or modify HamAlert

### Profile Status

Fetches current triggers and compares against all profiles:

```
$ hamalert-cli profile status

Current triggers on HamAlert: 10
Permanent triggers matched: 3

Profile match analysis:
  home:      7/7 triggers match (100%) <- best match
  portable:  2/4 triggers match (50%)

Recorded current profile: home
Status: In sync
```

When mismatch detected, offers actions:
```
Actions:
  [U]pdate record to 'portable' (no changes to HamAlert)
  [S]ave current triggers as new profile
  [I]gnore
```

### Integrated Status Checks

Profile match analysis runs automatically on:

- `profile list` - Shows match percentages, warns on mismatch
- `profile switch` - Warns if already on target profile
- `profile save` - Warns if overwriting with different content

### Permanent Triggers Management

`profile set-permanent` uses interactive TUI (same pattern as bulk-delete):

- Loads current triggers from HamAlert
- Pre-checks triggers already in `permanent.json`
- User toggles which triggers are permanent
- Saves updated `permanent.json`

## Trigger Matching

Triggers match by:
- `conditions` - deep equality
- `comment` - exact string match

Ignored for matching (but preserved in saved profiles):
- `_id`, `user_id`, `matchCount`
- `options`, `disabled`

## Edge Cases

**Empty states:**
- No `permanent.json` → treated as empty array
- No profiles exist → helpful message suggesting `profile save`
- No `current-profile` → "No current profile recorded"

**Errors:**
- Profile not found → list available profiles
- Corrupt JSON → report parse error
- HamAlert API failures → abort after backup taken
