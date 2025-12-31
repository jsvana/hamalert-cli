# Profiles Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable users to maintain different sets of triggers for different locations/activities, with permanent triggers that persist across all profiles.

**Architecture:** Add a `profile` subcommand group with nested subcommands. Profile data stored as JSON files in `~/.local/share/hamalert/profiles/`. Trigger matching uses `conditions` + `comment` for identity comparison. All destructive operations are dry-run by default with auto-backup.

**Tech Stack:** Rust, clap (nested subcommands), serde_json, inquire (TUI), existing patterns from backup/restore.

---

## Task 1: Add StoredTrigger Struct and Trigger Matching

The core data structure for profiles - a trigger without runtime-only fields.

**Files:**
- Modify: `src/main.rs:287-329` (near Trigger/EditableTrigger structs)

**Step 1: Write the failing test**

Add to the `tests` module at the bottom of main.rs:

```rust
#[test]
fn test_triggers_match_identical() {
    let t1 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Test trigger".to_string(),
        options: None,
    };
    let t2 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Test trigger".to_string(),
        options: None,
    };
    assert!(triggers_match(&t1, &t2));
}

#[test]
fn test_triggers_match_different_callsign() {
    let t1 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Test trigger".to_string(),
        options: None,
    };
    let t2 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "K2DEF"}),
        actions: vec!["app".to_string()],
        comment: "Test trigger".to_string(),
        options: None,
    };
    assert!(!triggers_match(&t1, &t2));
}

#[test]
fn test_triggers_match_different_comment() {
    let t1 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Comment A".to_string(),
        options: None,
    };
    let t2 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Comment B".to_string(),
        options: None,
    };
    assert!(!triggers_match(&t1, &t2));
}

#[test]
fn test_triggers_match_ignores_actions() {
    let t1 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["app".to_string()],
        comment: "Test".to_string(),
        options: None,
    };
    let t2 = StoredTrigger {
        conditions: serde_json::json!({"callsign": "W1ABC"}),
        actions: vec!["url".to_string(), "app".to_string()],
        comment: "Test".to_string(),
        options: None,
    };
    assert!(triggers_match(&t1, &t2));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_triggers_match -q`
Expected: FAIL with "cannot find struct `StoredTrigger`"

**Step 3: Write minimal implementation**

Add after the `EditableTrigger` struct (around line 329):

```rust
/// Trigger data for storage in profile files (without runtime fields like _id)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StoredTrigger {
    conditions: serde_json::Value,
    actions: Vec<String>,
    comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<serde_json::Value>,
}

impl StoredTrigger {
    fn from_trigger(trigger: &Trigger) -> Self {
        Self {
            conditions: trigger.conditions.clone(),
            actions: trigger.actions.clone(),
            comment: trigger.comment.clone(),
            options: trigger.options.clone(),
        }
    }
}

/// Check if two triggers match by conditions and comment (identity match)
fn triggers_match(a: &StoredTrigger, b: &StoredTrigger) -> bool {
    a.conditions == b.conditions && a.comment == b.comment
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_triggers_match -q`
Expected: PASS (4 tests)

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): add StoredTrigger struct and trigger matching"
```

---

## Task 2: Add Profile Directory Helper Functions

**Files:**
- Modify: `src/main.rs:186-193` (near backup_dir function)

**Step 1: Write the failing test**

```rust
#[test]
fn test_profiles_dir_is_under_data_dir() {
    let dir = profiles_dir().unwrap();
    assert!(dir.to_string_lossy().contains("hamalert"));
    assert!(dir.to_string_lossy().contains("profiles"));
}

#[test]
fn test_permanent_triggers_path_is_json() {
    let path = permanent_triggers_path().unwrap();
    assert!(path.to_string_lossy().ends_with("permanent.json"));
}

#[test]
fn test_current_profile_path_exists() {
    let path = current_profile_path().unwrap();
    assert!(path.to_string_lossy().contains("current-profile"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_profiles_dir -q && cargo test test_permanent -q && cargo test test_current_profile_path -q`
Expected: FAIL with "cannot find function"

**Step 3: Write minimal implementation**

Add after `backup_dir()` function:

```rust
fn profiles_dir() -> Result<PathBuf, Box<dyn Error>> {
    let dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("profiles");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn permanent_triggers_path() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("permanent.json");
    Ok(path)
}

fn current_profile_path() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("current-profile");
    Ok(path)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_profiles_dir -q && cargo test test_permanent -q && cargo test test_current_profile_path -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): add directory helper functions"
```

---

## Task 3: Add Profile Load/Save Helper Functions

**Files:**
- Modify: `src/main.rs` (after the directory helpers)

**Step 1: Write the failing test**

```rust
#[test]
fn test_load_profile_not_found() {
    let result = load_profile("nonexistent_profile_xyz");
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_load_profile -q`
Expected: FAIL with "cannot find function"

**Step 3: Write minimal implementation**

```rust
fn load_profile(name: &str) -> Result<Vec<StoredTrigger>, Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Profile '{}' not found: {}", name, e))?;
    let triggers: Vec<StoredTrigger> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse profile '{}': {}", name, e))?;
    Ok(triggers)
}

fn save_profile(name: &str, triggers: &[StoredTrigger]) -> Result<PathBuf, Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    let json = serde_json::to_string_pretty(triggers)?;
    fs::write(&path, json)?;
    Ok(path)
}

fn load_permanent_triggers() -> Result<Vec<StoredTrigger>, Box<dyn Error>> {
    let path = permanent_triggers_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)?;
    let triggers: Vec<StoredTrigger> = serde_json::from_str(&content)?;
    Ok(triggers)
}

fn save_permanent_triggers(triggers: &[StoredTrigger]) -> Result<(), Box<dyn Error>> {
    let path = permanent_triggers_path()?;
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(triggers)?;
    fs::write(&path, json)?;
    Ok(())
}

fn load_current_profile_name() -> Result<Option<String>, Box<dyn Error>> {
    let path = current_profile_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let name = fs::read_to_string(&path)?.trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(name))
}

fn save_current_profile_name(name: &str) -> Result<(), Box<dyn Error>> {
    let path = current_profile_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, name)?;
    Ok(())
}

fn list_profiles() -> Result<Vec<String>, Box<dyn Error>> {
    let dir = profiles_dir()?;
    let mut profiles = vec![];
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    profiles.push(stem.to_string_lossy().to_string());
                }
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}

fn delete_profile(name: &str) -> Result<(), Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    if !path.exists() {
        return Err(format!("Profile '{}' not found", name).into());
    }
    fs::remove_file(&path)?;
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_load_profile -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): add profile load/save helper functions"
```

---

## Task 4: Add Profile Match Analysis Function

**Files:**
- Modify: `src/main.rs` (after load/save helpers)

**Step 1: Write the failing test**

```rust
#[test]
fn test_calculate_profile_match_full_match() {
    let current = vec![
        StoredTrigger {
            conditions: serde_json::json!({"callsign": "W1ABC"}),
            actions: vec!["app".to_string()],
            comment: "A".to_string(),
            options: None,
        },
        StoredTrigger {
            conditions: serde_json::json!({"callsign": "K2DEF"}),
            actions: vec!["app".to_string()],
            comment: "B".to_string(),
            options: None,
        },
    ];
    let profile = current.clone();
    let (matched, total) = calculate_profile_match(&current, &profile);
    assert_eq!(matched, 2);
    assert_eq!(total, 2);
}

#[test]
fn test_calculate_profile_match_partial() {
    let current = vec![
        StoredTrigger {
            conditions: serde_json::json!({"callsign": "W1ABC"}),
            actions: vec!["app".to_string()],
            comment: "A".to_string(),
            options: None,
        },
    ];
    let profile = vec![
        StoredTrigger {
            conditions: serde_json::json!({"callsign": "W1ABC"}),
            actions: vec!["app".to_string()],
            comment: "A".to_string(),
            options: None,
        },
        StoredTrigger {
            conditions: serde_json::json!({"callsign": "K2DEF"}),
            actions: vec!["app".to_string()],
            comment: "B".to_string(),
            options: None,
        },
    ];
    let (matched, total) = calculate_profile_match(&current, &profile);
    assert_eq!(matched, 1);
    assert_eq!(total, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_calculate_profile_match -q`
Expected: FAIL with "cannot find function"

**Step 3: Write minimal implementation**

```rust
/// Calculate how many triggers from a profile are present in current triggers
/// Returns (matched_count, profile_total)
fn calculate_profile_match(current: &[StoredTrigger], profile: &[StoredTrigger]) -> (usize, usize) {
    let matched = profile
        .iter()
        .filter(|p| current.iter().any(|c| triggers_match(c, p)))
        .count();
    (matched, profile.len())
}

/// Filter out permanent triggers from a list
fn filter_out_permanent(
    triggers: &[StoredTrigger],
    permanent: &[StoredTrigger],
) -> Vec<StoredTrigger> {
    triggers
        .iter()
        .filter(|t| !permanent.iter().any(|p| triggers_match(t, p)))
        .cloned()
        .collect()
}

/// Find triggers that don't match any profile or permanent triggers
fn find_unexpected_triggers(
    current: &[StoredTrigger],
    permanent: &[StoredTrigger],
    profile: Option<&[StoredTrigger]>,
) -> Vec<StoredTrigger> {
    current
        .iter()
        .filter(|t| {
            let is_permanent = permanent.iter().any(|p| triggers_match(t, p));
            let is_in_profile = profile
                .map(|p| p.iter().any(|pt| triggers_match(t, pt)))
                .unwrap_or(false);
            !is_permanent && !is_in_profile
        })
        .cloned()
        .collect()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_calculate_profile_match -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): add profile match analysis functions"
```

---

## Task 5: Add Profile Subcommand Structure

**Files:**
- Modify: `src/main.rs:28-102` (Commands enum)

**Step 1: Add ProfileCommands enum and Profile variant**

No test for this - it's pure CLI structure. Add to the `Commands` enum:

```rust
/// Manage trigger profiles for different locations/activities
#[command(subcommand)]
Profile(ProfileCommands),
```

And add the new enum after `Commands`:

```rust
#[derive(Subcommand)]
enum ProfileCommands {
    /// List all available profiles
    List,
    /// Show triggers in a profile
    Show {
        /// Profile name
        name: String,
    },
    /// Show current profile status and match analysis
    Status,
    /// Save current triggers as a profile
    Save {
        /// Profile name
        name: String,
        /// Create from backup file instead of current triggers
        #[arg(long)]
        from_backup: Option<PathBuf>,
    },
    /// Switch to a different profile
    Switch {
        /// Profile name to switch to
        name: String,
        /// Actually perform the switch (default is dry-run)
        #[arg(long)]
        no_dry_run: bool,
    },
    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
    /// Interactively select permanent triggers
    SetPermanent {
        /// Set from backup file instead of current triggers
        #[arg(long)]
        from_backup: Option<PathBuf>,
    },
    /// Show current permanent triggers
    ShowPermanent,
}
```

**Step 2: Verify compilation**

Run: `cargo build`
Expected: WARNING about unused ProfileCommands (not handled in match yet)

**Step 3: Add stub match arm in main()**

Add after the `BulkDelete` match arm:

```rust
Commands::Profile(profile_cmd) => {
    match profile_cmd {
        ProfileCommands::List => {
            println!("profile list - not yet implemented");
        }
        ProfileCommands::Show { name } => {
            println!("profile show {} - not yet implemented", name);
        }
        ProfileCommands::Status => {
            println!("profile status - not yet implemented");
        }
        ProfileCommands::Save { name, from_backup } => {
            println!("profile save {} - not yet implemented", name);
        }
        ProfileCommands::Switch { name, no_dry_run } => {
            println!("profile switch {} - not yet implemented", name);
        }
        ProfileCommands::Delete { name } => {
            println!("profile delete {} - not yet implemented", name);
        }
        ProfileCommands::SetPermanent { from_backup } => {
            println!("profile set-permanent - not yet implemented");
        }
        ProfileCommands::ShowPermanent => {
            println!("profile show-permanent - not yet implemented");
        }
    }
}
```

**Step 4: Verify it builds and runs**

Run: `cargo build && cargo run -- profile list`
Expected: "profile list - not yet implemented"

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): add profile subcommand structure"
```

---

## Task 6: Implement profile show-permanent

**Files:**
- Modify: `src/main.rs` (ProfileCommands::ShowPermanent match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::ShowPermanent => {
    let permanent = load_permanent_triggers()?;
    if permanent.is_empty() {
        println!("No permanent triggers set.");
        println!("\nUse 'hamalert-cli profile set-permanent' to select permanent triggers.");
    } else {
        println!("Permanent triggers ({}):", permanent.len());
        for trigger in &permanent {
            let mode = trigger
                .conditions
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("any");
            let callsign = trigger
                .conditions
                .get("callsign")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("  - [{}] {} - \"{}\"", mode, callsign, trigger.comment);
        }
    }
}
```

**Step 2: Verify it works**

Run: `cargo run -- profile show-permanent`
Expected: "No permanent triggers set." (since none exist yet)

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile show-permanent"
```

---

## Task 7: Implement profile set-permanent

**Files:**
- Modify: `src/main.rs` (ProfileCommands::SetPermanent match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::SetPermanent { from_backup } => {
    // Load triggers from backup file or fetch from HamAlert
    let triggers: Vec<Trigger> = match from_backup {
        Some(path) => {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read backup file: {}", e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse backup file: {}", e))?
        }
        None => fetch_triggers(&client).await?,
    };

    if triggers.is_empty() {
        println!("No triggers found.");
        return Ok(());
    }

    // Load existing permanent triggers
    let existing_permanent = load_permanent_triggers()?;

    // Convert to StoredTrigger for comparison
    let stored_triggers: Vec<StoredTrigger> =
        triggers.iter().map(StoredTrigger::from_trigger).collect();

    // Build display items
    let display_items: Vec<String> = triggers.iter().map(format_trigger_for_display).collect();

    // Pre-select triggers that are already permanent
    let default_selections: Vec<usize> = stored_triggers
        .iter()
        .enumerate()
        .filter(|(_, t)| existing_permanent.iter().any(|p| triggers_match(t, p)))
        .map(|(i, _)| i)
        .collect();

    println!("Select triggers to mark as PERMANENT (always active across all profiles):\n");

    let selected_result = MultiSelect::new(
        "Permanent triggers (checked = permanent):",
        display_items.clone(),
    )
    .with_default(&default_selections)
    .with_vim_mode(true)
    .with_page_size(15)
    .with_help_message("Space=toggle, j/k=navigate, Enter=confirm, Esc=cancel")
    .prompt();

    let selected_displays: Vec<String> = match selected_result {
        Ok(selected) => selected,
        Err(InquireError::OperationCanceled) | Err(InquireError::OperationInterrupted) => {
            println!("Operation cancelled.");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // Find which triggers were selected
    let selected_set: std::collections::HashSet<&str> =
        selected_displays.iter().map(|s| s.as_str()).collect();
    let new_permanent: Vec<StoredTrigger> = triggers
        .iter()
        .filter(|t| selected_set.contains(format_trigger_for_display(t).as_str()))
        .map(StoredTrigger::from_trigger)
        .collect();

    save_permanent_triggers(&new_permanent)?;
    println!("\nSaved {} permanent triggers.", new_permanent.len());
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile set-permanent"
```

---

## Task 8: Implement profile show

**Files:**
- Modify: `src/main.rs` (ProfileCommands::Show match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::Show { name } => {
    let profile = load_profile(&name)?;
    if profile.is_empty() {
        println!("Profile '{}' is empty.", name);
    } else {
        println!("Profile '{}' ({} triggers):", name, profile.len());
        for trigger in &profile {
            let mode = trigger
                .conditions
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("any");
            let callsign = trigger
                .conditions
                .get("callsign")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("  - [{}] {} - \"{}\"", mode, callsign, trigger.comment);
        }
    }
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile show"
```

---

## Task 9: Implement profile list

**Files:**
- Modify: `src/main.rs` (ProfileCommands::List match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::List => {
    let profiles = list_profiles()?;
    let current_profile = load_current_profile_name()?;
    let permanent = load_permanent_triggers()?;

    if profiles.is_empty() {
        println!("No profiles saved.");
        println!("\nUse 'hamalert-cli profile save <name>' to create one.");
        return Ok(());
    }

    // Fetch current triggers to calculate match percentages
    let current_triggers = fetch_triggers(&client).await?;
    let current_stored: Vec<StoredTrigger> = current_triggers
        .iter()
        .map(StoredTrigger::from_trigger)
        .collect();

    // Filter out permanent triggers for matching
    let current_non_permanent = filter_out_permanent(&current_stored, &permanent);

    println!("Profiles:");
    let mut best_match: Option<(&str, usize, usize)> = None;

    for profile_name in &profiles {
        let profile = load_profile(profile_name).unwrap_or_default();
        let (matched, total) = calculate_profile_match(&current_non_permanent, &profile);
        let percentage = if total > 0 {
            (matched * 100) / total
        } else {
            100
        };

        let is_current = current_profile.as_ref() == Some(profile_name);
        let marker = if is_current { "*" } else { " " };

        println!(
            "  {} {:<15} ({}/{}  {}% match){}",
            marker,
            profile_name,
            matched,
            total,
            percentage,
            if is_current { " <- current" } else { "" }
        );

        // Track best match
        if best_match.is_none() || matched > best_match.unwrap().1 {
            best_match = Some((profile_name, matched, total));
        }
    }

    // Warn if recorded profile doesn't match best
    if let Some(current) = &current_profile {
        if let Some((best_name, best_matched, best_total)) = best_match {
            if best_name != current && best_matched == best_total && best_total > 0 {
                let current_profile_data = load_profile(current).unwrap_or_default();
                let (current_matched, current_total) =
                    calculate_profile_match(&current_non_permanent, &current_profile_data);
                if current_matched < current_total {
                    println!(
                        "\n⚠ Current triggers match '{}' better than recorded '{}'",
                        best_name, current
                    );
                    println!("Run 'profile status' for details.");
                }
            }
        }
    }

    println!("\nPermanent triggers: {}", permanent.len());
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile list"
```

---

## Task 10: Implement profile save

**Files:**
- Modify: `src/main.rs` (ProfileCommands::Save match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::Save { name, from_backup } => {
    let permanent = load_permanent_triggers()?;

    let triggers: Vec<StoredTrigger> = match from_backup {
        Some(path) => {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read backup file: {}", e))?;
            let backup_triggers: Vec<Trigger> = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse backup file: {}", e))?;
            backup_triggers
                .iter()
                .map(StoredTrigger::from_trigger)
                .collect()
        }
        None => {
            let fetched = fetch_triggers(&client).await?;
            fetched.iter().map(StoredTrigger::from_trigger).collect()
        }
    };

    // Filter out permanent triggers
    let profile_triggers = filter_out_permanent(&triggers, &permanent);

    // Check if profile already exists
    let profile_path = profiles_dir()?.join(format!("{}.json", name));
    if profile_path.exists() {
        let existing = load_profile(&name)?;
        if existing != profile_triggers {
            println!("Profile '{}' already exists with different content.", name);
            println!("Existing: {} triggers, New: {} triggers", existing.len(), profile_triggers.len());
            print!("Overwrite? [y/N]: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut confirm = String::new();
            std::io::stdin().read_line(&mut confirm)?;
            if !confirm.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }
    }

    let path = save_profile(&name, &profile_triggers)?;
    println!(
        "Saved {} triggers to profile '{}' (excluded {} permanent)",
        profile_triggers.len(),
        name,
        triggers.len() - profile_triggers.len()
    );

    // Set as current profile if saving from live state
    if from_backup.is_none() {
        save_current_profile_name(&name)?;
        println!("Set '{}' as current profile.", name);
    }
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile save"
```

---

## Task 11: Implement profile delete

**Files:**
- Modify: `src/main.rs` (ProfileCommands::Delete match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::Delete { name } => {
    // Check if it's the current profile
    let current = load_current_profile_name()?;
    if current.as_ref() == Some(&name) {
        println!("Warning: '{}' is the current profile.", name);
        print!("Delete anyway? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut confirm = String::new();
        std::io::stdin().read_line(&mut confirm)?;
        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
        // Clear current profile
        let path = current_profile_path()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    delete_profile(&name)?;
    println!("Deleted profile '{}'.", name);
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile delete"
```

---

## Task 12: Implement profile status

**Files:**
- Modify: `src/main.rs` (ProfileCommands::Status match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::Status => {
    let current_triggers = fetch_triggers(&client).await?;
    let permanent = load_permanent_triggers()?;
    let current_profile_name = load_current_profile_name()?;
    let profiles = list_profiles()?;

    let current_stored: Vec<StoredTrigger> = current_triggers
        .iter()
        .map(StoredTrigger::from_trigger)
        .collect();

    // Count permanent matches
    let permanent_matched = current_stored
        .iter()
        .filter(|t| permanent.iter().any(|p| triggers_match(t, p)))
        .count();

    println!("Current triggers on HamAlert: {}", current_triggers.len());
    println!("Permanent triggers matched: {}/{}", permanent_matched, permanent.len());

    let current_non_permanent = filter_out_permanent(&current_stored, &permanent);

    if profiles.is_empty() {
        println!("\nNo profiles saved.");
        return Ok(());
    }

    println!("\nProfile match analysis:");
    let mut best_match: Option<(String, usize, usize)> = None;

    for profile_name in &profiles {
        let profile = load_profile(profile_name).unwrap_or_default();
        let (matched, total) = calculate_profile_match(&current_non_permanent, &profile);
        let percentage = if total > 0 { (matched * 100) / total } else { 100 };

        let marker = if matched == total && total > 0 {
            " <- best match"
        } else {
            ""
        };
        println!("  {:<15} {}/{} ({}% match){}", profile_name, matched, total, percentage, marker);

        if best_match.is_none() || matched > best_match.as_ref().unwrap().1 {
            best_match = Some((profile_name.clone(), matched, total));
        }
    }

    // Current profile status
    println!("\nRecorded current profile: {}", current_profile_name.as_deref().unwrap_or("(none)"));

    // Check for mismatch
    if let Some((best_name, best_matched, best_total)) = &best_match {
        let is_in_sync = current_profile_name.as_ref() == Some(best_name)
            && *best_matched == *best_total;

        if is_in_sync {
            println!("Status: ✓ In sync");
        } else if current_profile_name.is_some() && best_matched == best_total && *best_total > 0 {
            println!("Status: ⚠ Mismatch - HamAlert matches '{}' better", best_name);
            println!("\nActions:");
            println!("  [U]pdate record to '{}' (no changes to HamAlert)", best_name);
            println!("  [S]ave current triggers as new profile");
            println!("  [I]gnore");

            print!("\nChoice: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut choice = String::new();
            std::io::stdin().read_line(&mut choice)?;

            match choice.trim().to_lowercase().as_str() {
                "u" => {
                    save_current_profile_name(best_name)?;
                    println!("Updated current profile record to '{}'.", best_name);
                }
                "s" => {
                    print!("Enter profile name: ");
                    std::io::Write::flush(&mut std::io::stdout())?;
                    let mut new_name = String::new();
                    std::io::stdin().read_line(&mut new_name)?;
                    let new_name = new_name.trim();
                    if !new_name.is_empty() {
                        let profile_triggers = filter_out_permanent(&current_stored, &permanent);
                        save_profile(new_name, &profile_triggers)?;
                        save_current_profile_name(new_name)?;
                        println!("Saved and set '{}' as current profile.", new_name);
                    }
                }
                _ => {
                    println!("No changes made.");
                }
            }
        } else {
            println!("Status: No exact profile match");
        }
    }

    // Show unexpected triggers
    let current_profile_data = current_profile_name
        .as_ref()
        .and_then(|n| load_profile(n).ok());
    let unexpected = find_unexpected_triggers(
        &current_stored,
        &permanent,
        current_profile_data.as_deref(),
    );

    if !unexpected.is_empty() {
        println!("\nUnmatched triggers ({}):", unexpected.len());
        for t in &unexpected {
            let mode = t.conditions.get("mode").and_then(|v| v.as_str()).unwrap_or("any");
            let callsign = t.conditions.get("callsign").and_then(|v| v.as_str()).unwrap_or("?");
            println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
        }
    }
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile status"
```

---

## Task 13: Implement profile switch

This is the most complex command. Break into steps.

**Files:**
- Modify: `src/main.rs` (ProfileCommands::Switch match arm)

**Step 1: Replace stub implementation**

```rust
ProfileCommands::Switch { name, no_dry_run } => {
    // Load all data
    let target_profile = load_profile(&name)?;
    let permanent = load_permanent_triggers()?;
    let current_profile_name = load_current_profile_name()?;
    let current_triggers = fetch_triggers(&client).await?;

    let current_stored: Vec<StoredTrigger> = current_triggers
        .iter()
        .map(StoredTrigger::from_trigger)
        .collect();

    // Categorize current triggers
    let permanent_triggers: Vec<&StoredTrigger> = current_stored
        .iter()
        .filter(|t| permanent.iter().any(|p| triggers_match(t, p)))
        .collect();

    let current_profile_data = current_profile_name
        .as_ref()
        .and_then(|n| load_profile(n).ok());

    let unexpected = find_unexpected_triggers(
        &current_stored,
        &permanent,
        current_profile_data.as_deref(),
    );

    // Triggers to delete (non-permanent current triggers)
    let to_delete: Vec<&Trigger> = current_triggers
        .iter()
        .filter(|t| {
            let stored = StoredTrigger::from_trigger(t);
            !permanent.iter().any(|p| triggers_match(&stored, p))
        })
        .collect();

    // Display plan
    println!("Current profile: {}", current_profile_name.as_deref().unwrap_or("(none)"));
    println!("Switching to: {}\n", name);

    println!("Permanent triggers (unchanged): {}", permanent_triggers.len());
    if !permanent_triggers.is_empty() {
        for t in &permanent_triggers {
            let mode = t.conditions.get("mode").and_then(|v| v.as_str()).unwrap_or("any");
            let callsign = t.conditions.get("callsign").and_then(|v| v.as_str()).unwrap_or("?");
            println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
        }
    }

    println!("\nWill DELETE {} triggers:", to_delete.len());
    for t in &to_delete {
        println!("  - {}", format_trigger_for_display(t));
    }

    println!("\nWill CREATE {} triggers from '{}':", target_profile.len(), name);
    for t in &target_profile {
        let mode = t.conditions.get("mode").and_then(|v| v.as_str()).unwrap_or("any");
        let callsign = t.conditions.get("callsign").and_then(|v| v.as_str()).unwrap_or("?");
        println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
    }

    // Handle unexpected triggers
    if !unexpected.is_empty() {
        println!("\n⚠ Found {} unexpected triggers (not permanent, not in current profile):", unexpected.len());
        for t in &unexpected {
            let mode = t.conditions.get("mode").and_then(|v| v.as_str()).unwrap_or("any");
            let callsign = t.conditions.get("callsign").and_then(|v| v.as_str()).unwrap_or("?");
            println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
        }

        if !no_dry_run {
            println!("\n  [D]elete them");
            if let Some(ref current_name) = current_profile_name {
                println!("  [S]ave to '{}' profile first", current_name);
            }
            println!("  [C]ancel");

            print!("\nChoice: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut choice = String::new();
            std::io::stdin().read_line(&mut choice)?;

            match choice.trim().to_lowercase().as_str() {
                "d" => {
                    // Continue with deletion
                }
                "s" => {
                    if let Some(ref current_name) = current_profile_name {
                        // Update current profile to include unexpected triggers
                        let mut updated_profile = current_profile_data.unwrap_or_default();
                        for t in &unexpected {
                            if !updated_profile.iter().any(|p| triggers_match(p, t)) {
                                updated_profile.push(t.clone());
                            }
                        }
                        save_profile(current_name, &updated_profile)?;
                        println!("Updated '{}' profile with {} additional triggers.", current_name, unexpected.len());
                    }
                }
                _ => {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
        }
    }

    if !no_dry_run {
        println!("\nDRY RUN - No changes made.");
        println!("Run with --no-dry-run to execute.");
        return Ok(());
    }

    // Execute the switch
    // 1. Create backup
    let backup_path = backup_dir()?.join(format!(
        "hamalert-backup-before-switch-{}.json",
        Local::now().format("%Y-%m-%d-%H%M%S")
    ));
    let backup_json = serde_json::to_string_pretty(&current_triggers)?;
    fs::write(&backup_path, backup_json)?;
    println!("\nBacked up {} triggers to {}", current_triggers.len(), backup_path.display());

    // 2. Delete non-permanent triggers
    for trigger in &to_delete {
        delete_trigger(&client, &trigger.id).await?;
    }
    println!("Deleted {} triggers.", to_delete.len());

    // 3. Create triggers from target profile
    for stored in &target_profile {
        // Convert StoredTrigger to Trigger for API
        let trigger = Trigger {
            id: String::new(),
            user_id: None,
            conditions: stored.conditions.clone(),
            actions: stored.actions.clone(),
            comment: stored.comment.clone(),
            match_count: None,
            disabled: None,
            options: stored.options.clone(),
        };
        create_trigger_from_backup(&client, &trigger).await?;
    }
    println!("Created {} triggers from '{}'.", target_profile.len(), name);

    // 4. Update current profile
    save_current_profile_name(&name)?;
    println!("\nSwitched to profile '{}'.", name);
}
```

**Step 2: Verify it builds**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(profiles): implement profile switch"
```

---

## Task 14: Run Full Test Suite and Clippy

**Step 1: Run tests**

Run: `cargo test`
Expected: All tests pass (13 original + new profile tests)

**Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings

**Step 3: Run fmt**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues (or run `cargo fmt` to fix)

**Step 4: Final commit if any fixes needed**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting"
```

---

## Task 15: Update Documentation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

**Step 1: Add profile commands to README.md**

Add a new section after "bulk-delete":

```markdown
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
```

**Step 2: Update CLAUDE.md architecture section**

Add profile-related info to the Key components section.

**Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add profile commands documentation"
```

---

## Task 16: Final Integration Test

**Step 1: Manual testing workflow**

Run through this sequence to verify everything works:

```bash
# 1. Show initial state
cargo run -- profile list
cargo run -- profile show-permanent

# 2. Set some permanent triggers
cargo run -- profile set-permanent

# 3. Save current as "home"
cargo run -- profile save home

# 4. Check status
cargo run -- profile status

# 5. Try switching (dry-run)
cargo run -- profile switch home

# 6. List profiles
cargo run -- profile list
```

**Step 2: Verify all commands work without errors**

Each command should complete successfully with appropriate output.

---

## Summary

Total: 16 tasks covering:
- Core data structures (StoredTrigger, trigger matching)
- Helper functions (paths, load/save, analysis)
- All 8 profile subcommands
- Tests, linting, documentation
