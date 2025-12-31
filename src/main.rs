use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use inquire::{InquireError, MultiSelect};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Config {
    username: String,
    password: String,
}

#[derive(Parser)]
#[command(name = "hamalert-cli")]
#[command(about = "CLI for HamAlert API", long_about = None)]
struct Cli {
    #[arg(long)]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    AddTrigger {
        #[arg(long)]
        callsign: Vec<String>,

        #[arg(long)]
        comment: String,

        #[arg(long, value_enum)]
        actions: Vec<Action>,

        #[arg(long, value_enum)]
        mode: Option<Mode>,

        /// Use compact format (comma-only, no spaces) for callsigns
        #[arg(long, conflicts_with = "one_per_line")]
        compact: bool,

        /// Send callsigns one per line instead of comma-separated
        #[arg(long, conflicts_with = "compact")]
        one_per_line: bool,
    },
    /// Add triggers for all callsigns in a Ham2K PoLo callsign notes file
    ImportPoloNotes {
        /// URL to the Ham2K PoLo callsign notes file
        #[arg(long)]
        url: String,

        #[arg(long)]
        comment: String,

        #[arg(long, value_enum)]
        actions: Vec<Action>,

        #[arg(long, value_enum)]
        mode: Option<Mode>,

        /// Show what would be added without actually adding triggers
        #[arg(long)]
        dry_run: bool,

        /// Use compact format (comma-only, no spaces) for callsigns
        #[arg(long, conflicts_with = "one_per_line")]
        compact: bool,

        /// Send callsigns one per line instead of comma-separated
        #[arg(long, conflicts_with = "compact")]
        one_per_line: bool,
    },
    /// Backup all triggers to a JSON file
    Backup {
        /// Output file path (default: hamalert-backup-YYYY-MM-DD.json)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Restore triggers from a JSON backup file
    Restore {
        /// Input backup file path
        #[arg(long)]
        input: PathBuf,

        /// Actually perform the restore (default is dry-run)
        #[arg(long)]
        no_dry_run: bool,
    },
    /// Interactively edit an existing trigger
    Edit,
    /// Interactively delete multiple triggers with TUI selection
    BulkDelete {
        /// Show what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage trigger profiles for different locations/activities
    #[command(subcommand)]
    Profile(ProfileCommands),
}

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

#[derive(Clone, ValueEnum)]
enum Action {
    Url,
    App,
    Threema,
    Telnet,
}

#[derive(Clone, ValueEnum)]
#[allow(clippy::upper_case_acronyms)]
enum Mode {
    CW,
    FT8,
    SSB,
}

impl Action {
    fn as_str(&self) -> &str {
        match self {
            Action::Url => "url",
            Action::App => "app",
            Action::Threema => "threema",
            Action::Telnet => "telnet",
        }
    }
}

impl Mode {
    fn as_str(&self) -> &str {
        match self {
            Mode::CW => "cw",
            Mode::FT8 => "ft8",
            Mode::SSB => "ssb",
        }
    }
}

/// Determines how multiple callsigns are formatted when sent to HamAlert
#[derive(Clone, Copy, Default)]
enum CallsignFormat {
    /// Comma-space separated: "N0CALL, K0TEST, W0XYZ"
    #[default]
    Default,
    /// Comma-only (compact): "N0CALL,K0TEST,W0XYZ"
    Compact,
    /// One per line: "N0CALL\nK0TEST\nW0XYZ"
    OnePerLine,
}

impl CallsignFormat {
    fn separator(&self) -> &'static str {
        match self {
            CallsignFormat::Default => ", ",
            CallsignFormat::Compact => ",",
            CallsignFormat::OnePerLine => "\n",
        }
    }

    fn from_flags(compact: bool, one_per_line: bool) -> Self {
        match (compact, one_per_line) {
            (true, _) => CallsignFormat::Compact,
            (_, true) => CallsignFormat::OnePerLine,
            _ => CallsignFormat::Default,
        }
    }
}

#[derive(Serialize)]
struct TriggerData {
    conditions: Conditions,
    comment: String,
    actions: Vec<String>,
    options: serde_json::Value,
}

#[derive(Serialize)]
struct Conditions {
    callsign: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
}

fn backup_dir() -> Result<PathBuf, Box<dyn Error>> {
    let data_dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("backups");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

#[allow(dead_code)]
fn profiles_dir() -> Result<PathBuf, Box<dyn Error>> {
    let dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("profiles");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[allow(dead_code)]
fn permanent_triggers_path() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("permanent.json");
    Ok(path)
}

#[allow(dead_code)]
fn current_profile_path() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("hamalert")
        .join("current-profile");
    Ok(path)
}

#[allow(dead_code)]
fn load_profile(name: &str) -> Result<Vec<StoredTrigger>, Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Profile '{}' not found: {}", name, e))?;
    let triggers: Vec<StoredTrigger> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse profile '{}': {}", name, e))?;
    Ok(triggers)
}

#[allow(dead_code)]
fn save_profile(name: &str, triggers: &[StoredTrigger]) -> Result<PathBuf, Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    let json = serde_json::to_string_pretty(triggers)?;
    fs::write(&path, json)?;
    Ok(path)
}

#[allow(dead_code)]
fn load_permanent_triggers() -> Result<Vec<StoredTrigger>, Box<dyn Error>> {
    let path = permanent_triggers_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)?;
    let triggers: Vec<StoredTrigger> = serde_json::from_str(&content)?;
    Ok(triggers)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn save_current_profile_name(name: &str) -> Result<(), Box<dyn Error>> {
    let path = current_profile_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, name)?;
    Ok(())
}

#[allow(dead_code)]
fn list_profiles() -> Result<Vec<String>, Box<dyn Error>> {
    let dir = profiles_dir()?;
    let mut profiles = vec![];
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false)
                && let Some(stem) = path.file_stem()
            {
                profiles.push(stem.to_string_lossy().to_string());
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}

#[allow(dead_code)]
fn delete_profile(name: &str) -> Result<(), Box<dyn Error>> {
    let path = profiles_dir()?.join(format!("{}.json", name));
    if !path.exists() {
        return Err(format!("Profile '{}' not found", name).into());
    }
    fs::remove_file(&path)?;
    Ok(())
}

/// Calculate how many triggers from a profile are present in current triggers
/// Returns (matched_count, profile_total)
#[allow(dead_code)]
fn calculate_profile_match(current: &[StoredTrigger], profile: &[StoredTrigger]) -> (usize, usize) {
    let matched = profile
        .iter()
        .filter(|p| current.iter().any(|c| triggers_match(c, p)))
        .count();
    (matched, profile.len())
}

/// Filter out permanent triggers from a list
#[allow(dead_code)]
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
#[allow(dead_code)]
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

fn load_config(config_file: Option<PathBuf>) -> Result<Config, Box<dyn Error>> {
    let config_path = if let Some(path) = config_file {
        path
    } else {
        // Use XDG_CONFIG_HOME or default to ~/.config
        let config_dir = dirs::config_dir()
            .ok_or("Could not determine config directory")?
            .join("hamalert");
        config_dir.join("config.toml")
    };

    let config_content = fs::read_to_string(&config_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "Config file not found at: {}\n\n\
                Please create a config file with the following format:\n\n\
                username = \"your_username\"\n\
                password = \"your_password\"\n",
                config_path.display()
            )
        } else {
            format!(
                "Failed to read config file at {}: {}",
                config_path.display(),
                e
            )
        }
    })?;

    let config: Config = toml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    Ok(config)
}

async fn login(client: &Client, username: &str, password: &str) -> Result<(), Box<dyn Error>> {
    let params = [("username", username), ("password", password)];

    let response = client
        .post("https://hamalert.org/login")
        .form(&params)
        .send()
        .await?;

    println!("Login status: {}", response.status());

    if !response.status().is_success() {
        return Err("Login failed".into());
    }

    Ok(())
}

/// Parse Ham2K PoLo callsign notes content and extract callsigns.
/// Each line's first word is treated as a callsign.
/// Empty lines and comment lines (starting with # or //) are skipped.
fn parse_polo_notes_content(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Skip empty lines
            if trimmed.is_empty() {
                return None;
            }
            // Skip comment lines
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                return None;
            }
            // Extract the first word (callsign)
            trimmed.split_whitespace().next().map(|s| s.to_string())
        })
        .collect()
}

/// Fetch and parse Ham2K PoLo callsign notes from a URL.
async fn fetch_polo_notes(client: &Client, url: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch PoLo notes from {}: {}",
            url,
            response.status()
        )
        .into());
    }

    let content = response.text().await?;
    Ok(parse_polo_notes_content(&content))
}

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

/// Trigger data for storage in profile files (without runtime fields like _id)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StoredTrigger {
    conditions: serde_json::Value,
    actions: Vec<String>,
    comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<serde_json::Value>,
}

impl StoredTrigger {
    #[allow(dead_code)]
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
#[allow(dead_code)]
fn triggers_match(a: &StoredTrigger, b: &StoredTrigger) -> bool {
    a.conditions == b.conditions && a.comment == b.comment
}

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

async fn add_trigger(
    client: &Client,
    callsign: &str,
    comment: &str,
    actions: Vec<String>,
    mode: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let trigger_data = TriggerData {
        conditions: Conditions {
            callsign: callsign.to_string(),
            mode,
        },
        comment: comment.to_string(),
        actions,
        options: json!({}),
    };

    let response = client
        .post("https://hamalert.org/ajax/trigger_update")
        .json(&trigger_data)
        .send()
        .await?;

    println!("Add trigger status for {}: {}", callsign, response.status());

    // Optionally print the response body
    let body = response.text().await?;
    if !body.is_empty() {
        println!("Response: {}", body);
    }

    Ok(())
}

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Load config from file
    let config = load_config(cli.config_file)?;

    // Create a client with cookie jar to maintain session
    let client = Client::builder().cookie_store(true).build()?;

    // Login first
    login(&client, &config.username, &config.password).await?;

    // Execute the subcommand
    match cli.command {
        Commands::AddTrigger {
            callsign,
            comment,
            actions,
            mode,
            compact,
            one_per_line,
        } => {
            let action_strings: Vec<String> =
                actions.iter().map(|a| a.as_str().to_string()).collect();

            let mode_string = mode.as_ref().map(|m| m.as_str().to_string());

            if callsign.is_empty() {
                return Err("At least one --callsign must be provided".into());
            }
            // Join callsigns with the specified format
            let format = CallsignFormat::from_flags(compact, one_per_line);
            let combined_callsigns = callsign.join(format.separator());
            add_trigger(
                &client,
                &combined_callsigns,
                &comment,
                action_strings,
                mode_string,
            )
            .await?;
        }
        Commands::ImportPoloNotes {
            url,
            comment,
            actions,
            mode,
            dry_run,
            compact,
            one_per_line,
        } => {
            let callsigns = fetch_polo_notes(&client, &url).await?;

            if callsigns.is_empty() {
                println!("No callsigns found at {}", url);
                return Ok(());
            }

            println!("Found {} callsigns at {}", callsigns.len(), url);

            let action_strings: Vec<String> =
                actions.iter().map(|a| a.as_str().to_string()).collect();

            let mode_string = mode.as_ref().map(|m| m.as_str().to_string());
            let format = CallsignFormat::from_flags(compact, one_per_line);

            if dry_run {
                println!("\nDry run - would add triggers for:");
                for cs in &callsigns {
                    println!(
                        "  {} (comment: {:?}, actions: {:?}, mode: {:?})",
                        cs, comment, action_strings, mode_string
                    );
                }
            } else {
                let combined_callsigns = callsigns.join(format.separator());
                add_trigger(
                    &client,
                    &combined_callsigns,
                    &comment,
                    action_strings.clone(),
                    mode_string.clone(),
                )
                .await?;
            }
        }
        Commands::Backup { output } => {
            let triggers = fetch_triggers(&client).await?;

            let output_path = match output {
                Some(path) => path,
                None => {
                    let date = Local::now().format("%Y-%m-%d");
                    backup_dir()?.join(format!("hamalert-backup-{}.json", date))
                }
            };

            let json = serde_json::to_string_pretty(&triggers)?;
            fs::write(&output_path, json)?;

            println!(
                "Backed up {} triggers to {}",
                triggers.len(),
                output_path.display()
            );
        }
        Commands::Restore { input, no_dry_run } => {
            // Read and parse backup file
            let backup_content = fs::read_to_string(&input)
                .map_err(|e| format!("Failed to read backup file {}: {}", input.display(), e))?;
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
                    println!("  {}", format_trigger_for_display(trigger));
                }
                println!("\nRun with --no-dry-run to execute.");
                return Ok(());
            }

            // Create auto-backup before destructive operation
            let backup_path = backup_dir()?.join(format!(
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

            let selection: usize = input.parse().map_err(|_| "Invalid selection")?;

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
        Commands::BulkDelete { dry_run } => {
            let triggers = fetch_triggers(&client).await?;

            if triggers.is_empty() {
                println!("No triggers found.");
                return Ok(());
            }

            println!("Fetched {} triggers.\n", triggers.len());
            println!("Instructions:");
            println!("  j/k or arrows: Navigate up/down");
            println!("  Space: Toggle selection (unchecked = will be DELETED)");
            println!("  Enter: Confirm");
            println!("  Esc: Cancel\n");

            // Build display items
            let display_items: Vec<String> =
                triggers.iter().map(format_trigger_for_display).collect();

            // All items start selected (checked = keep)
            let default_selections: Vec<usize> = (0..triggers.len()).collect();

            // Run the interactive multi-select
            let kept_result = MultiSelect::new(
                "Select triggers to KEEP (unchecked will be deleted):",
                display_items.clone(),
            )
            .with_default(&default_selections)
            .with_vim_mode(true)
            .with_page_size(15)
            .with_help_message("Space=toggle, j/k=navigate, Enter=confirm, Esc=cancel")
            .prompt();

            let kept_displays: Vec<String> = match kept_result {
                Ok(selected) => selected,
                Err(InquireError::OperationCanceled) | Err(InquireError::OperationInterrupted) => {
                    println!("Operation cancelled.");
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            };

            // Find triggers to delete (those NOT in kept list)
            let kept_set: std::collections::HashSet<&str> =
                kept_displays.iter().map(|s| s.as_str()).collect();
            let to_delete: Vec<&Trigger> = triggers
                .iter()
                .filter(|t| !kept_set.contains(format_trigger_for_display(t).as_str()))
                .collect();

            if to_delete.is_empty() {
                println!("No triggers selected for deletion.");
                return Ok(());
            }

            // Show summary
            println!("\nTriggers to DELETE ({}):", to_delete.len());
            for trigger in &to_delete {
                println!("  - {}", format_trigger_for_display(trigger));
            }

            // Dry run mode
            if dry_run {
                println!("\n[DRY RUN] No triggers were deleted.");
                return Ok(());
            }

            // Confirmation prompt
            println!();
            print!("Proceed with deletion? [y/N]: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut confirm_input = String::new();
            std::io::stdin().read_line(&mut confirm_input)?;
            if !confirm_input.trim().eq_ignore_ascii_case("y") {
                println!("Deletion cancelled.");
                return Ok(());
            }

            // Auto-backup before deletion
            let backup_path = backup_dir()?.join(format!(
                "hamalert-backup-before-bulk-delete-{}.json",
                Local::now().format("%Y-%m-%d-%H%M%S")
            ));
            let backup_json = serde_json::to_string_pretty(&triggers)?;
            fs::write(&backup_path, backup_json)?;
            println!(
                "Backed up {} triggers to {}",
                triggers.len(),
                backup_path.display()
            );

            // Delete the selected triggers
            for trigger in &to_delete {
                delete_trigger(&client, &trigger.id).await?;
                println!("Deleted: {}", format_trigger_for_display(trigger));
            }

            println!(
                "\nDeleted {} trigger(s). Kept {} trigger(s).",
                to_delete.len(),
                triggers.len() - to_delete.len()
            );
        }
        Commands::Profile(profile_cmd) => match profile_cmd {
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
                    let (matched, total) =
                        calculate_profile_match(&current_non_permanent, &profile);
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
                if let Some(current) = &current_profile
                    && let Some((best_name, best_matched, best_total)) = best_match
                    && best_name != current
                    && best_matched == best_total
                    && best_total > 0
                {
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

                println!("\nPermanent triggers: {}", permanent.len());
            }
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
            ProfileCommands::Status => {
                println!("profile status - not yet implemented");
            }
            ProfileCommands::Save { name, from_backup } => {
                let permanent = load_permanent_triggers()?;

                let triggers: Vec<StoredTrigger> = match &from_backup {
                    Some(path) => {
                        let content = fs::read_to_string(path)
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
                    if existing == profile_triggers {
                        // Identical content - no need to re-save
                        println!(
                            "Profile '{}' already has identical content. No changes needed.",
                            name
                        );
                        // Still set as current profile if saving from live state
                        if from_backup.is_none() {
                            save_current_profile_name(&name)?;
                            println!("Set '{}' as current profile.", name);
                        }
                        return Ok(());
                    }
                    // Different content - prompt for confirmation
                    println!("Profile '{}' already exists with different content.", name);
                    println!(
                        "Existing: {} triggers, New: {} triggers",
                        existing.len(),
                        profile_triggers.len()
                    );
                    print!("Overwrite? [y/N]: ");
                    std::io::Write::flush(&mut std::io::stdout())?;
                    let mut confirm = String::new();
                    std::io::stdin().read_line(&mut confirm)?;
                    if !confirm.trim().eq_ignore_ascii_case("y") {
                        println!("Cancelled.");
                        return Ok(());
                    }
                }

                let _path = save_profile(&name, &profile_triggers)?;
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
            ProfileCommands::Switch {
                name,
                no_dry_run: _,
            } => {
                println!("profile switch {} - not yet implemented", name);
            }
            ProfileCommands::Delete { name } => {
                println!("profile delete {} - not yet implemented", name);
            }
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
                let display_items: Vec<String> =
                    triggers.iter().map(format_trigger_for_display).collect();

                // Pre-select triggers that are already permanent
                let default_selections: Vec<usize> = stored_triggers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| existing_permanent.iter().any(|p| triggers_match(t, p)))
                    .map(|(i, _)| i)
                    .collect();

                println!(
                    "Select triggers to mark as PERMANENT (always active across all profiles):\n"
                );

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
                    Err(InquireError::OperationCanceled)
                    | Err(InquireError::OperationInterrupted) => {
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
            ProfileCommands::ShowPermanent => {
                let permanent = load_permanent_triggers()?;
                if permanent.is_empty() {
                    println!("No permanent triggers set.");
                    println!(
                        "\nUse 'hamalert-cli profile set-permanent' to select permanent triggers."
                    );
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
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_polo_notes_simple_callsigns() {
        let content = "W1ABC\nK2DEF\nN3GHI";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF", "N3GHI"]);
    }

    #[test]
    fn test_parse_polo_notes_callsigns_with_notes() {
        let content = "W1ABC friend from club\nK2DEF met at field day\nN3GHI";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF", "N3GHI"]);
    }

    #[test]
    fn test_parse_polo_notes_empty_content() {
        let content = "";
        let result = parse_polo_notes_content(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_polo_notes_only_empty_lines() {
        let content = "\n\n\n";
        let result = parse_polo_notes_content(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_polo_notes_hash_comments() {
        let content = "# This is a comment\nW1ABC\n# Another comment\nK2DEF";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF"]);
    }

    #[test]
    fn test_parse_polo_notes_slash_comments() {
        let content = "// This is a comment\nW1ABC\n// Another comment\nK2DEF";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF"]);
    }

    #[test]
    fn test_parse_polo_notes_mixed_comments() {
        let content = "# Hash comment\n// Slash comment\nW1ABC";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC"]);
    }

    #[test]
    fn test_parse_polo_notes_whitespace_handling() {
        let content = "  W1ABC  \n\tK2DEF\t\n   N3GHI   notes here";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF", "N3GHI"]);
    }

    #[test]
    fn test_parse_polo_notes_mixed_content() {
        let content = "# Header comment\n\nW1ABC friend\n\n// Another comment\nK2DEF\n\n";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC", "K2DEF"]);
    }

    #[test]
    fn test_parse_polo_notes_only_comments() {
        let content = "# Comment 1\n// Comment 2\n# Comment 3";
        let result = parse_polo_notes_content(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_polo_notes_indented_comments() {
        let content = "  # Indented hash comment\n  // Indented slash comment\nW1ABC";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC"]);
    }

    #[test]
    fn test_parse_polo_notes_single_callsign() {
        let content = "W1ABC";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC"]);
    }

    #[test]
    fn test_parse_polo_notes_callsign_with_hash_in_note() {
        // A hash in the middle of a note (not at start) should not be treated as comment
        let content = "W1ABC note with #hashtag";
        let result = parse_polo_notes_content(content);
        assert_eq!(result, vec!["W1ABC"]);
    }

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

    #[test]
    fn test_load_profile_not_found() {
        let result = load_profile("nonexistent_profile_xyz");
        assert!(result.is_err());
    }

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
        let current = vec![StoredTrigger {
            conditions: serde_json::json!({"callsign": "W1ABC"}),
            actions: vec!["app".to_string()],
            comment: "A".to_string(),
            options: None,
        }];
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
}
