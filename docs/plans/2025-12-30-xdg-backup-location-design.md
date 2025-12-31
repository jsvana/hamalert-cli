# XDG-Compliant Backup Location

## Summary

Store hamalert backups in `~/.local/share/hamalert/backups/` instead of the current working directory, following XDG Base Directory specification.

## Design Decisions

- **Location**: `~/.local/share/hamalert/backups/` (XDG data dir)
- **Auto-create**: Directory created automatically when needed
- **`--output` flag**: Unchanged behavior - absolute/relative paths work as before, only the default changes

## Affected Commands

| Command | Current Behavior | New Behavior |
|---------|-----------------|--------------|
| `backup` | Writes to `./hamalert-backup-YYYY-MM-DD.json` | Writes to `~/.local/share/hamalert/backups/hamalert-backup-YYYY-MM-DD.json` |
| `restore` | Auto-backup to `./hamalert-backup-before-restore-TIMESTAMP.json` | Auto-backup to XDG backups dir |
| `bulk-delete` | Auto-backup to `./hamalert-backup-before-bulk-delete-TIMESTAMP.json` | Auto-backup to XDG backups dir |

## Implementation

Add helper function:

```rust
fn backup_dir() -> Result<PathBuf, Box<dyn Error>> {
    let data_dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("backups");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}
```

Update each command to use `backup_dir()?.join(filename)` for default paths.
