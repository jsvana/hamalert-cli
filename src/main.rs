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
}

#[derive(Clone, ValueEnum)]
enum Action {
    Url,
    App,
    Threema,
    Telnet,
}

#[derive(Clone, ValueEnum)]
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

            // Loop through each callsign and make a separate API call
            for cs in callsign {
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

    Ok(())
}
