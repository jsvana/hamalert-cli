# Data Formats and Locations

## Config

Default config path:

```text
~/.config/hamalert/config.toml
```

Preferred config stores only the username:

```toml
username = "N0CALL"
```

When the OS keyring is unavailable, the CLI may use a legacy plaintext fallback:

```toml
username = "N0CALL"
password = "secret"
```

Do not create or print plaintext password config unless the user explicitly asks for that fallback.

## Callsign Files

`import-file` reads a text file where the first word on each non-comment line is the callsign.

```text
W1AW
K3LR friend from contest
N0CALL met at hamfest
```

Skipped lines:

```text
# comment
// comment

```

Notes after the first word are ignored.

## Trigger settings JSON

The editor and `--from-json` use one JSON object containing `conditions`, `actions`, `comment`, and optional `options`. Use HamAlert condition keys such as `band`, `source`, `snrFrom`, and `snrTo`. When editing, include every condition and option you want to keep. Select the trigger separately by passing `--trigger-id` or using the picker.

The trigger settings schema provides field completion and validation in editors that support JSON Schema. [Trigger settings schema](https://github.com/jsvana/hamalert-cli/blob/main/schemas/trigger-settings.schema.json)

This example uses nested Club Log settings. It requires a configured Club Log account. Replace `N0CALL` with the callsign configured in Club Log. HamAlert populates `bandslot` when it syncs the trigger. [HamAlert web editor](https://github.com/hamalert/hamalert-web/blob/021bd79cc3c50f8dd9f8941873fb8f7619e2baee/js/trigger_editor.js#L534-L539) · [HamAlert Club Log setup](https://hamalert.org/help)

```json
{
  "conditions": {
    "band": ["20m", "40m"],
    "bandslot": []
  },
  "actions": ["app"],
  "comment": "Unworked Club Log band slots",
  "options": {
    "clublog": {
      "callsign": "N0CALL",
      "modes": "all",
      "status": ["confirmed", "worked", "verified"],
      "date": 0,
      "forceUpdate": true
    }
  }
}
```

## Backup Files

`backup` writes a JSON array of HamAlert trigger objects. A backup may include runtime fields such as `_id`, `user_id`, `matchCount`, `disabled`, `conditions`, `actions`, `comment`, and `options`.

Default backup directory:

```text
~/.local/share/hamalert/backups/
```

Use the actual path printed by `hamalert-cli backup`; platform data directories can vary.

## Profiles

Profile data lives under the platform data directory, normally:

```text
~/.local/share/hamalert/
  permanent.json
  current-profile
  profiles/
    home.json
    portable.json
```

Profile and permanent trigger files store trigger-like JSON without runtime-only IDs. Matching is based on `conditions` and `comment`; actions and options are preserved but are not part of identity matching.
