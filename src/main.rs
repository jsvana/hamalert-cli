use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
        } => {
            let action_strings: Vec<String> =
                actions.iter().map(|a| a.as_str().to_string()).collect();

            let mode_string = mode.as_ref().map(|m| m.as_str().to_string());

            if callsign.is_empty() {
                return Err("At least one --callsign must be provided".into());
            }
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
        Commands::ImportPoloNotes {
            url,
            comment,
            actions,
            mode,
            dry_run,
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

            if dry_run {
                println!("\nDry run - would add triggers for:");
                for cs in &callsigns {
                    println!(
                        "  {} (comment: {:?}, actions: {:?}, mode: {:?})",
                        cs, comment, action_strings, mode_string
                    );
                }
                println!("\nTotal: {} triggers", callsigns.len());
            } else {
                for cs in callsigns {
                    add_trigger(
                        &client,
                        &cs,
                        &comment,
                        action_strings.clone(),
                        mode_string.clone(),
                    )
                    .await?;
                }
            }
        }
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
        Commands::Restore {
            input: _,
            no_dry_run: _,
        } => {
            unimplemented!("Restore command not yet implemented")
        }
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
}
