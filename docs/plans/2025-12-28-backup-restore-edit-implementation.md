# Backup, Restore, and Edit Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add backup, restore, and edit commands to hamalert-cli, plus fix callsign handling to use comma-separated values.

**Architecture:** Extend the existing single-file CLI with three new subcommands. Reuse existing login/client infrastructure. Add helper functions for trigger fetching, deletion, and JSON file I/O.

**Tech Stack:** Rust, clap (CLI), reqwest (HTTP), serde_json (JSON), chrono (timestamps), std::process::Command (editor)

---

### Task 1: Fix Comma-Separated Callsigns

**Files:**
- Modify: `src/main.rs:266-276`

**Step 1: Update the add-trigger handler**

Change the loop to join callsigns:

```rust
Commands::AddTrigger {
    callsign,
    comment,
    actions,
    mode,
} => {
    let action_strings: Vec<String> =
        actions.iter().map(|a| a.as_str().to_string()).collect();

    let mode_string = mode.as_ref().map(|m| m.as_str().to_string());

    // Join all callsigns with commas for a single trigger
    let combined_callsigns = callsign.join(",");
    add_trigger(
        &client,
        &combined_callsigns,
        &comment,
        action_strings,
        mode_string,
    )
    .await?;
}
```

**Step 2: Run tests and checks**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "fix: combine multiple callsigns into single trigger

Instead of creating separate triggers for each callsign,
join them with commas for a single API call."
```

---

### Task 2: Add chrono Dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add chrono to dependencies**

Add after the existing dependencies:

```toml
chrono = "0.4"
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Successful build

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add chrono dependency for timestamp formatting"
```

---

### Task 3: Add Backup Subcommand Definition

**Files:**
- Modify: `src/main.rs` (Commands enum around line 27)

**Step 1: Add Backup variant to Commands enum**

Add after `ImportPoloNotes`:

```rust
/// Backup all triggers to a JSON file
Backup {
    /// Output file path (default: hamalert-backup-YYYY-MM-DD.json)
    #[arg(long)]
    output: Option<PathBuf>,
},
```

**Step 2: Add chrono import at top of file**

Add after other imports:

```rust
use chrono::Local;
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused variant (expected at this stage)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add backup subcommand definition"
```

---

### Task 4: Add fetch_triggers Helper Function

**Files:**
- Modify: `src/main.rs` (after `fetch_polo_notes` function, around line 204)

**Step 1: Add Trigger struct for deserialization**

Add before `fetch_triggers`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Trigger {
    #[serde(rename = "_id")]
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    conditions: serde_json::Value,
    actions: Vec<String>,
    comment: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "matchCount")]
    match_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<serde_json::Value>,
}
```

**Step 2: Add fetch_triggers function**

```rust
async fn fetch_triggers(client: &Client) -> Result<Vec<Trigger>, Box<dyn Error>> {
    let response = client
        .get("https://hamalert.org/ajax/triggers")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch triggers: {}", response.status()).into());
    }

    let triggers: Vec<Trigger> = response.json().await?;
    Ok(triggers)
}
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused function (expected)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add fetch_triggers helper function"
```

---

### Task 5: Implement Backup Command Handler

**Files:**
- Modify: `src/main.rs` (in main match block, after ImportPoloNotes handler)

**Step 1: Add backup handler**

```rust
Commands::Backup { output } => {
    let triggers = fetch_triggers(&client).await?;

    let output_path = output.unwrap_or_else(|| {
        let date = Local::now().format("%Y-%m-%d");
        PathBuf::from(format!("hamalert-backup-{}.json", date))
    });

    let json = serde_json::to_string_pretty(&triggers)?;
    fs::write(&output_path, json)?;

    println!(
        "Backed up {} triggers to {}",
        triggers.len(),
        output_path.display()
    );
}
```

**Step 2: Run tests and checks**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement backup command

Fetches all triggers from HamAlert and saves them as JSON.
Default filename includes current date."
```

---

### Task 6: Add Restore Subcommand Definition

**Files:**
- Modify: `src/main.rs` (Commands enum)

**Step 1: Add Restore variant**

Add after `Backup`:

```rust
/// Restore triggers from a JSON backup file
Restore {
    /// Input backup file path
    #[arg(long)]
    input: PathBuf,

    /// Actually perform the restore (default is dry-run)
    #[arg(long)]
    no_dry_run: bool,
},
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused variant

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add restore subcommand definition"
```

---

### Task 7: Add delete_trigger Helper Function

**Files:**
- Modify: `src/main.rs` (after `add_trigger` function)

**Step 1: Add delete_trigger function**

```rust
async fn delete_trigger(client: &Client, id: &str) -> Result<(), Box<dyn Error>> {
    let response = client
        .post("https://hamalert.org/ajax/trigger_delete")
        .form(&[("id", id)])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to delete trigger {}: {}", id, response.status()).into());
    }

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused function

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add delete_trigger helper function"
```

---

### Task 8: Add create_trigger_from_backup Helper Function

**Files:**
- Modify: `src/main.rs` (after `delete_trigger`)

**Step 1: Add function to create trigger without ID**

```rust
async fn create_trigger_from_backup(
    client: &Client,
    trigger: &Trigger,
) -> Result<(), Box<dyn Error>> {
    // Build trigger data without _id so a new one is created
    let trigger_data = serde_json::json!({
        "conditions": trigger.conditions,
        "actions": trigger.actions,
        "comment": trigger.comment,
        "options": trigger.options.clone().unwrap_or(serde_json::json!({})),
    });

    let response = client
        .post("https://hamalert.org/ajax/trigger_update")
        .json(&trigger_data)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to create trigger '{}': {}",
            trigger.comment,
            response.status()
        )
        .into());
    }

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused function

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add create_trigger_from_backup helper"
```

---

### Task 9: Implement Restore Command Handler

**Files:**
- Modify: `src/main.rs` (in main match block)

**Step 1: Add restore handler**

```rust
Commands::Restore { input, no_dry_run } => {
    // Read and parse backup file
    let backup_content = fs::read_to_string(&input).map_err(|e| {
        format!("Failed to read backup file {}: {}", input.display(), e)
    })?;
    let backup_triggers: Vec<Trigger> = serde_json::from_str(&backup_content)
        .map_err(|e| format!("Failed to parse backup file: {}", e))?;

    // Fetch current triggers
    let current_triggers = fetch_triggers(&client).await?;

    if !no_dry_run {
        println!("DRY RUN - No changes will be made\n");
        println!(
            "This will DELETE {} existing triggers and restore {} triggers from backup.\n",
            current_triggers.len(),
            backup_triggers.len()
        );
        println!("Triggers to be restored:");
        for trigger in &backup_triggers {
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
            println!("  [{}] {} - \"{}\"", mode, callsign, trigger.comment);
        }
        println!("\nRun with --no-dry-run to execute.");
        return Ok(());
    }

    // Create auto-backup before destructive operation
    let backup_path = PathBuf::from(format!(
        "hamalert-backup-before-restore-{}.json",
        Local::now().format("%Y-%m-%d-%H%M%S")
    ));
    let backup_json = serde_json::to_string_pretty(&current_triggers)?;
    fs::write(&backup_path, backup_json)?;
    println!(
        "Backed up {} existing triggers to {}",
        current_triggers.len(),
        backup_path.display()
    );

    // Delete all existing triggers
    for trigger in &current_triggers {
        delete_trigger(&client, &trigger.id).await?;
    }
    println!("Deleted {} existing triggers", current_triggers.len());

    // Restore from backup
    for trigger in &backup_triggers {
        create_trigger_from_backup(&client, trigger).await?;
        println!("Restored trigger: {}", trigger.comment);
    }
    println!(
        "\nRestored {} triggers from {}",
        backup_triggers.len(),
        input.display()
    );
}
```

**Step 2: Run tests and checks**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement restore command

- Dry-run by default showing what would happen
- Auto-backup before destructive restore
- Deletes all existing triggers then restores from file"
```

---

### Task 10: Add Edit Subcommand Definition

**Files:**
- Modify: `src/main.rs` (Commands enum)

**Step 1: Add Edit variant**

```rust
/// Interactively edit an existing trigger
Edit,
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused variant

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add edit subcommand definition"
```

---

### Task 11: Add format_trigger_for_display Helper

**Files:**
- Modify: `src/main.rs` (after Trigger struct)

**Step 1: Add display formatting function**

```rust
fn format_trigger_for_display(trigger: &Trigger) -> String {
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
    format!("[{}] {} - \"{}\"", mode, callsign, trigger.comment)
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused function

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add format_trigger_for_display helper"
```

---

### Task 12: Add update_trigger Helper Function

**Files:**
- Modify: `src/main.rs` (after `create_trigger_from_backup`)

**Step 1: Add update function that preserves ID**

```rust
async fn update_trigger(client: &Client, trigger: &Trigger) -> Result<(), Box<dyn Error>> {
    let trigger_data = serde_json::json!({
        "_id": trigger.id,
        "conditions": trigger.conditions,
        "actions": trigger.actions,
        "comment": trigger.comment,
        "options": trigger.options.clone().unwrap_or(serde_json::json!({})),
    });

    let response = client
        .post("https://hamalert.org/ajax/trigger_update")
        .json(&trigger_data)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to update trigger '{}': {}",
            trigger.comment,
            response.status()
        )
        .into());
    }

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warning about unused function

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add update_trigger helper function"
```

---

### Task 13: Add EditableTrigger Struct

**Files:**
- Modify: `src/main.rs` (after Trigger struct)

**Step 1: Add struct for editor JSON (excludes internal fields)**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EditableTrigger {
    conditions: serde_json::Value,
    actions: Vec<String>,
    comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<serde_json::Value>,
}

impl EditableTrigger {
    fn from_trigger(trigger: &Trigger) -> Self {
        Self {
            conditions: trigger.conditions.clone(),
            actions: trigger.actions.clone(),
            comment: trigger.comment.clone(),
            options: trigger.options.clone(),
        }
    }

    fn apply_to_trigger(self, trigger: &mut Trigger) {
        trigger.conditions = self.conditions;
        trigger.actions = self.actions;
        trigger.comment = self.comment;
        trigger.options = self.options;
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Warnings about unused items

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add EditableTrigger struct for editor workflow"
```

---

### Task 14: Implement Edit Command Handler

**Files:**
- Modify: `src/main.rs` (in main match block)

**Step 1: Add edit handler**

```rust
Commands::Edit => {
    let triggers = fetch_triggers(&client).await?;

    if triggers.is_empty() {
        println!("No triggers found.");
        return Ok(());
    }

    // Display numbered list
    println!("Select a trigger to edit:\n");
    for (i, trigger) in triggers.iter().enumerate() {
        println!("  {}. {}", i + 1, format_trigger_for_display(trigger));
    }
    println!("\nEnter number (1-{}), or q to quit: ", triggers.len());

    // Read user selection
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.eq_ignore_ascii_case("q") {
        println!("Cancelled.");
        return Ok(());
    }

    let selection: usize = input
        .parse()
        .map_err(|_| "Invalid selection")?;

    if selection < 1 || selection > triggers.len() {
        return Err(format!("Selection must be between 1 and {}", triggers.len()).into());
    }

    let mut trigger = triggers[selection - 1].clone();
    let original_editable = EditableTrigger::from_trigger(&trigger);

    // Create temp file with editable JSON
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("hamalert-edit-{}.json", trigger.id));
    let json = serde_json::to_string_pretty(&original_editable)?;
    fs::write(&temp_path, &json)?;

    // Open in editor
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    loop {
        let status = std::process::Command::new(&editor)
            .arg(&temp_path)
            .status()
            .map_err(|e| format!("Failed to open editor '{}': {}", editor, e))?;

        if !status.success() {
            fs::remove_file(&temp_path).ok();
            return Err("Editor exited with error".into());
        }

        // Read and parse edited content
        let edited_content = fs::read_to_string(&temp_path)?;

        match serde_json::from_str::<EditableTrigger>(&edited_content) {
            Ok(edited) => {
                // Check if anything changed
                let edited_json = serde_json::to_string(&edited)?;
                let original_json = serde_json::to_string(&original_editable)?;

                if edited_json == original_json {
                    println!("No changes made.");
                } else {
                    edited.apply_to_trigger(&mut trigger);
                    update_trigger(&client, &trigger).await?;
                    println!("Updated trigger: {}", trigger.comment);
                }

                fs::remove_file(&temp_path).ok();
                break;
            }
            Err(e) => {
                println!("Invalid JSON: {}", e);
                println!("Press Enter to re-edit, or 'q' to quit without saving: ");

                let mut retry_input = String::new();
                std::io::stdin().read_line(&mut retry_input)?;

                if retry_input.trim().eq_ignore_ascii_case("q") {
                    fs::remove_file(&temp_path).ok();
                    println!("Cancelled without saving.");
                    break;
                }
            }
        }
    }
}
```

**Step 2: Run tests and checks**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement edit command

Interactive trigger editing:
- Shows numbered list of triggers
- Opens selected trigger in \$EDITOR
- Validates JSON and offers re-edit on errors
- Updates trigger via API if changes detected"
```

---

### Task 15: Update Restore to Use format_trigger_for_display

**Files:**
- Modify: `src/main.rs` (restore handler)

**Step 1: Refactor restore dry-run output**

Replace the manual formatting in the restore handler's dry-run loop with:

```rust
println!("Triggers to be restored:");
for trigger in &backup_triggers {
    println!("  {}", format_trigger_for_display(trigger));
}
```

**Step 2: Run tests and checks**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "refactor: use format_trigger_for_display in restore"
```

---

### Task 16: Final Integration Test

**Step 1: Build release and verify CLI help**

Run: `cargo build --release && ./target/release/hamalert-cli --help`

Expected output should show all subcommands:
- add-trigger
- import-polo-notes
- backup
- restore
- edit

**Step 2: Run full test suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

**Step 3: Final commit if any formatting changes**

```bash
git add -A
git commit -m "chore: final formatting cleanup" --allow-empty
```

---

## Summary

16 tasks implementing:
1. Comma-separated callsigns fix (Task 1)
2. Backup command (Tasks 2-5)
3. Restore command (Tasks 6-9)
4. Edit command (Tasks 10-14)
5. Cleanup (Tasks 15-16)

Each task is atomic and commits independently for easy review/rollback.
