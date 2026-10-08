use chrono::Local;
use clap::{Args, Parser, Subcommand, ValueEnum};
use inquire::{InquireError, MultiSelect, Password, Text};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "hamalert-cli";

#[derive(Deserialize, Serialize, Default)]
struct Config {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug)]
enum PasswordStoreError {
    Unavailable(String),
    Unexpected(String),
}

impl fmt::Display for PasswordStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordStoreError::Unavailable(message) => {
                write!(f, "Password store unavailable: {}", message)
            }
            PasswordStoreError::Unexpected(message) => {
                write!(f, "Password store error: {}", message)
            }
        }
    }
}

impl Error for PasswordStoreError {}

type PasswordStoreResult<T> = Result<T, PasswordStoreError>;

trait PasswordStore {
    fn get_password(&self, username: &str) -> PasswordStoreResult<Option<String>>;
    fn set_password(&self, username: &str, password: &str) -> PasswordStoreResult<()>;
    fn delete_password(&self, username: &str) -> PasswordStoreResult<()>;
    fn is_available(&self) -> bool;
}

struct KeyringPasswordStore;

impl KeyringPasswordStore {
    fn entry(username: &str) -> PasswordStoreResult<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, username).map_err(map_keyring_error)
    }
}

impl PasswordStore for KeyringPasswordStore {
    fn get_password(&self, username: &str) -> PasswordStoreResult<Option<String>> {
        match Self::entry(username)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn set_password(&self, username: &str, password: &str) -> PasswordStoreResult<()> {
        Self::entry(username)?
            .set_password(password)
            .map_err(map_keyring_error)
    }

    fn delete_password(&self, username: &str) -> PasswordStoreResult<()> {
        match Self::entry(username)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn is_available(&self) -> bool {
        Self::entry("__availability_check__").is_ok()
    }
}

fn map_keyring_error(error: keyring::Error) -> PasswordStoreError {
    match error {
        keyring::Error::NoDefaultStore
        | keyring::Error::NoStorageAccess(_)
        | keyring::Error::PlatformFailure(_) => PasswordStoreError::Unavailable(error.to_string()),
        error => PasswordStoreError::Unexpected(error.to_string()),
    }
}

#[derive(Debug, PartialEq)]
enum CredentialSource {
    Keyring,
    ConfigFallback,
}

struct ResolvedCredentials {
    username: String,
    password: String,
    source: CredentialSource,
}

#[derive(Debug, Parser)]
#[command(name = "hamalert-cli")]
#[command(about = "CLI for HamAlert API", long_about = None)]
struct Cli {
    #[arg(long)]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

/// Shared options for trigger creation
#[derive(Debug, Parser, Clone)]
struct TriggerOptions {
    #[arg(long)]
    comment: String,

    /// Notification actions (e.g., --actions app telnet)
    #[arg(long, value_enum, num_args = 1..)]
    actions: Vec<Action>,

    /// Filter by mode (e.g., --mode cw ft8)
    #[arg(long, value_enum, num_args = 1..)]
    mode: Vec<Mode>,

    /// Use compact format (comma-only, no spaces) for callsigns
    #[arg(long, conflicts_with = "one_per_line")]
    compact: bool,

    /// Send callsigns one per line instead of comma-separated
    #[arg(long, conflicts_with = "compact")]
    one_per_line: bool,
}

/// Shared options for import commands
#[derive(Debug, Parser, Clone)]
struct ImportOptions {
    #[command(flatten)]
    trigger: TriggerOptions,

    /// Show what would be added without actually adding triggers
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Configure stored HamAlert credentials
    #[command(subcommand)]
    Auth(AuthCommands),
    /// Add a trigger for one or more callsigns
    AddTrigger {
        #[arg(long)]
        callsign: Vec<String>,

        #[command(flatten)]
        options: TriggerOptions,
    },
    /// Add triggers for all callsigns in a Ham2K PoLo callsign notes file (fetched from URL)
    ImportPoloNotes {
        /// URL to the Ham2K PoLo callsign notes file
        #[arg(long)]
        url: String,

        #[command(flatten)]
        options: ImportOptions,
    },
    /// Import callsigns from a local file (one callsign per line)
    ImportFile {
        /// Path to the callsign file
        #[arg(long)]
        file: PathBuf,

        #[command(flatten)]
        options: ImportOptions,
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
    /// Replace the callsign list of an existing trigger from a file (one callsign per line)
    SetCallsigns {
        /// ID of the trigger to update (see `backup` output for IDs)
        #[arg(long, required_unless_present = "comment", conflicts_with = "comment")]
        trigger_id: Option<String>,

        /// Select the trigger by its exact comment instead of its ID
        #[arg(long)]
        comment: Option<String>,

        /// Path to the callsign file
        #[arg(long)]
        file: PathBuf,

        /// Actually update the trigger (default is dry-run)
        #[arg(long)]
        no_dry_run: bool,

        /// Refuse to apply if more than this many callsigns would be removed
        /// (default: 10% of the current list, minimum 5)
        #[arg(long)]
        max_removals: Option<usize>,
    },
    /// Manage trigger profiles for different locations/activities
    #[command(subcommand)]
    Profile(ProfileCommands),
}

#[derive(Debug, Subcommand)]
enum AuthCommands {
    /// Validate and store HamAlert credentials
    Login {
        #[arg(long)]
        username: Option<String>,

        #[command(flatten)]
        password: PasswordInput,
    },
    /// Show credential configuration status
    Status,
    /// Remove stored password while keeping configured username
    Logout,
}

#[derive(Args)]
#[group(multiple = false)]
struct PasswordInput {
    #[arg(long)]
    password_stdin: bool,

    #[arg(long)]
    password_env: Option<String>,

    #[arg(long)]
    password: Option<String>,
}

impl fmt::Debug for PasswordInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordInput")
            .field("password_stdin", &self.password_stdin)
            .field("password_env", &self.password_env)
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Clone, ValueEnum)]
enum Action {
    Url,
    App,
    Threema,
    Telnet,
}

#[derive(Debug, Clone, ValueEnum)]
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

fn profile_match_percentage(matched: usize, total: usize) -> usize {
    (matched * 100).checked_div(total).unwrap_or(100)
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

fn default_config_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("hamalert")
        .join("config.toml"))
}

fn config_path(config_file: Option<PathBuf>) -> Result<PathBuf, Box<dyn Error>> {
    Ok(match config_file {
        Some(path) => path,
        None => default_config_path()?,
    })
}

fn load_config(config_file: Option<PathBuf>) -> Result<Config, Box<dyn Error>> {
    let config_path = config_path(config_file)?;
    load_config_from_path(&config_path)
}

fn load_config_from_path(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    let config_content = fs::read_to_string(config_path).map_err(|e| {
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

fn load_config_or_default(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    match load_config_from_path(config_path) {
        Ok(config) => Ok(config),
        Err(error) => {
            if !config_path.exists() {
                Ok(Config::default())
            } else {
                Err(error)
            }
        }
    }
}

fn save_config(config_path: &Path, config: &Config) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_config_file(config_path, toml::to_string(config)?.as_bytes())?;
    Ok(())
}

#[cfg(unix)]
fn write_config_file(config_path: &Path, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(config_path)?;
    file.write_all(contents)?;
    fs::set_permissions(config_path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_config_file(config_path: &Path, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(config_path)?;
    file.write_all(contents)?;
    Ok(())
}

fn password_from_input(
    input: &PasswordInput,
    stdin_content: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    if input.password_stdin {
        Ok(Some(
            stdin_content.trim_end_matches(['\r', '\n']).to_string(),
        ))
    } else if let Some(var) = &input.password_env {
        Ok(Some(std::env::var(var).map_err(|_| {
            format!("Environment variable {} is not set", var)
        })?))
    } else {
        Ok(input.password.clone())
    }
}

fn non_interactive_login_credentials(
    config: &Config,
    username_arg: Option<String>,
    supplied_password: Option<String>,
) -> Option<(String, String)> {
    let username = username_arg.or_else(|| config.username.clone())?;
    let password = supplied_password?;
    Some((username, password))
}

fn remove_config_password_for_logout(config: &mut Config) -> bool {
    let had_password = config.password.take().is_some();
    had_password || config.username.is_some()
}

fn require_username(config: &Config) -> Result<String, Box<dyn Error>> {
    config.username.clone().ok_or_else(|| {
        "Config is missing username. Run `hamalert-cli auth login` to configure credentials."
            .to_string()
            .into()
    })
}

fn missing_password_error(username: &str) -> Box<dyn Error> {
    format!(
        "No password found for {}. Run `hamalert-cli auth login` to store credentials.",
        username
    )
    .into()
}

fn config_fallback_credentials(
    username: &str,
    password: &Option<String>,
) -> Option<ResolvedCredentials> {
    password.as_ref().map(|password| ResolvedCredentials {
        username: username.to_string(),
        password: password.clone(),
        source: CredentialSource::ConfigFallback,
    })
}

fn resolve_credentials(
    config: &Config,
    password_store: &dyn PasswordStore,
) -> Result<ResolvedCredentials, Box<dyn Error>> {
    let username = require_username(config)?;

    match password_store.get_password(&username) {
        Ok(Some(password)) => Ok(ResolvedCredentials {
            username,
            password,
            source: CredentialSource::Keyring,
        }),
        Ok(None) | Err(PasswordStoreError::Unavailable(_)) => {
            config_fallback_credentials(&username, &config.password)
                .ok_or_else(|| missing_password_error(&username))
        }
        Err(error @ PasswordStoreError::Unexpected(_)) => {
            if let Some(credentials) = config_fallback_credentials(&username, &config.password) {
                Ok(credentials)
            } else {
                Err(error.into())
            }
        }
    }
}

async fn login(client: &Client, username: &str, password: &str) -> Result<(), Box<dyn Error>> {
    let params = [("username", username), ("password", password)];

    let response = client
        .post("https://hamalert.org/login")
        .form(&params)
        .send()
        .await?;

    println!("Login status: {}", response.status());

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() || login_body_indicates_invalid_credentials(&body) {
        return Err("Login failed".into());
    }

    Ok(())
}

fn login_body_indicates_invalid_credentials(body: &str) -> bool {
    body.contains("Login failed; please check username and password")
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

/// How a trigger stores its callsign condition, so it can be written back the same way
#[derive(Debug, PartialEq)]
enum CallsignShape {
    /// A single string joined by this separator
    Text(&'static str),
    /// A JSON array of strings
    List,
}

/// Parse a callsign file (one per line, `#`/`//` comments allowed) into a sorted,
/// de-duplicated, uppercased list.
fn parse_callsign_file(content: &str) -> Vec<String> {
    let set: std::collections::BTreeSet<String> = parse_polo_notes_content(content)
        .into_iter()
        .map(|c| c.to_uppercase())
        .collect();
    set.into_iter().collect()
}

/// Read the callsigns out of a trigger's `conditions.callsign`, along with its shape.
fn parse_callsign_condition(
    conditions: &serde_json::Value,
) -> Result<(Vec<String>, CallsignShape), String> {
    match conditions.get("callsign") {
        Some(serde_json::Value::String(text)) => {
            let separator = if text.contains('\n') {
                "\n"
            } else if text.contains(", ") || !text.contains(',') {
                CallsignFormat::Default.separator()
            } else {
                CallsignFormat::Compact.separator()
            };
            let callsigns = text
                .split([',', '\n'])
                .map(|c| c.trim().to_uppercase())
                .filter(|c| !c.is_empty())
                .collect();
            Ok((callsigns, CallsignShape::Text(separator)))
        }
        Some(serde_json::Value::Array(items)) => {
            let callsigns = items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|c| c.trim().to_uppercase())
                        .ok_or_else(|| format!("Unexpected callsign entry: {}", v))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((callsigns, CallsignShape::List))
        }
        Some(other) => Err(format!("Unexpected callsign condition: {}", other)),
        None if conditions.get("fullCallsign").is_some() => Err(
            "Trigger uses a fullCallsign condition; only callsign conditions are supported"
                .to_string(),
        ),
        None => Err("Trigger has no callsign condition".to_string()),
    }
}

/// Write callsigns into `conditions.callsign` in the given shape, leaving other conditions alone.
fn apply_callsigns(
    conditions: &mut serde_json::Value,
    callsigns: &[String],
    shape: &CallsignShape,
) {
    let value = match shape {
        CallsignShape::Text(separator) => json!(callsigns.join(separator)),
        CallsignShape::List => json!(callsigns),
    };
    conditions["callsign"] = value;
}

/// Return (added, removed, unchanged count) going from `current` to `desired`.
fn diff_callsigns(current: &[String], desired: &[String]) -> (Vec<String>, Vec<String>, usize) {
    let current: std::collections::BTreeSet<&String> = current.iter().collect();
    let desired: std::collections::BTreeSet<&String> = desired.iter().collect();
    let added = desired
        .difference(&current)
        .map(|c| c.to_string())
        .collect();
    let removed = current
        .difference(&desired)
        .map(|c| c.to_string())
        .collect();
    let unchanged = current.intersection(&desired).count();
    (added, removed, unchanged)
}

/// Maximum removals allowed in one update: the explicit limit, or 10% of the list (minimum 5).
fn removal_limit(current_len: usize, max_removals: Option<usize>) -> usize {
    max_removals.unwrap_or_else(|| (current_len / 10).max(5))
}

/// Find exactly one trigger by ID or exact comment.
fn select_trigger<'a>(
    triggers: &'a [Trigger],
    trigger_id: Option<&str>,
    comment: Option<&str>,
) -> Result<&'a Trigger, String> {
    let matches: Vec<&Trigger> = triggers
        .iter()
        .filter(|t| match (trigger_id, comment) {
            (Some(id), _) => t.id == id,
            (None, Some(comment)) => t.comment == comment,
            (None, None) => false,
        })
        .collect();
    match matches.as_slice() {
        [trigger] => Ok(trigger),
        [] => Err("No matching trigger found".to_string()),
        _ => Err(format!(
            "{} triggers match; use --trigger-id instead",
            matches.len()
        )),
    }
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

/// Shared logic for importing callsigns from any source
async fn import_callsigns(
    client: &Client,
    callsigns: Vec<String>,
    options: &ImportOptions,
) -> Result<(), Box<dyn Error>> {
    let action_strings: Vec<String> = options
        .trigger
        .actions
        .iter()
        .map(|a| a.as_str().to_string())
        .collect();

    let mode_string = if options.trigger.mode.is_empty() {
        None
    } else {
        Some(
            options
                .trigger
                .mode
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
    };
    let format = CallsignFormat::from_flags(options.trigger.compact, options.trigger.one_per_line);

    if options.dry_run {
        println!("\nDry run - would add triggers for:");
        for cs in &callsigns {
            println!(
                "  {} (comment: {:?}, actions: {:?}, mode: {:?})",
                cs, options.trigger.comment, action_strings, mode_string
            );
        }
    } else {
        let combined_callsigns = callsigns.join(format.separator());
        add_trigger(
            client,
            &combined_callsigns,
            &options.trigger.comment,
            action_strings,
            mode_string,
        )
        .await?;
    }

    Ok(())
}

async fn handle_auth_command(
    command: AuthCommands,
    config_path: &Path,
    mut config: Config,
    password_store: &dyn PasswordStore,
    client: &Client,
) -> Result<(), Box<dyn Error>> {
    match command {
        AuthCommands::Login { username, password } => {
            auth_login(
                config_path,
                &mut config,
                username,
                &password,
                password_store,
                client,
            )
            .await
        }
        AuthCommands::Status => auth_status(config_path, &config, password_store, client).await,
        AuthCommands::Logout => auth_logout(config_path, &mut config, password_store),
    }
}

async fn auth_login(
    config_path: &Path,
    config: &mut Config,
    username_arg: Option<String>,
    password_input: &PasswordInput,
    password_store: &dyn PasswordStore,
    client: &Client,
) -> Result<(), Box<dyn Error>> {
    let mut stdin_content = String::new();
    if password_input.password_stdin {
        std::io::stdin().read_to_string(&mut stdin_content)?;
    }

    let supplied_password = password_from_input(password_input, &stdin_content)?;

    let (username, password) = if let Some((username, password)) =
        non_interactive_login_credentials(config, username_arg.clone(), supplied_password.clone())
    {
        login(client, &username, &password).await?;
        (username, password)
    } else {
        let mut last_error: Option<Box<dyn Error>> = None;
        let mut successful_credentials = None;

        for attempt in 1..=3 {
            let username = prompt_username(username_arg.as_deref().or(config.username.as_deref()))?;
            let password = match &supplied_password {
                Some(password) => password.clone(),
                None => Password::new("HamAlert password")
                    .without_confirmation()
                    .prompt()?,
            };

            match login(client, &username, &password).await {
                Ok(()) => {
                    successful_credentials = Some((username, password));
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < 3 {
                        println!("Login failed. Please try again.");
                    }
                }
            }
        }

        successful_credentials.ok_or_else(|| last_error.unwrap_or_else(|| "Login failed".into()))?
    };

    config.username = Some(username.clone());
    match password_store.set_password(&username, &password) {
        Ok(()) => {
            config.password = None;
            save_config(config_path, config)?;
            println!("Stored HamAlert credentials in the system keyring.");
        }
        Err(PasswordStoreError::Unavailable(message)) => {
            config.password = Some(password);
            save_config(config_path, config)?;
            println!(
                "Warning: keyring unavailable ({}). Stored password in config fallback.",
                message
            );
        }
        Err(error @ PasswordStoreError::Unexpected(_)) => return Err(error.into()),
    }

    Ok(())
}

fn prompt_username(default: Option<&str>) -> Result<String, Box<dyn Error>> {
    let mut prompt = Text::new("HamAlert username");
    if let Some(default) = default {
        prompt = prompt.with_initial_value(default);
    }
    Ok(prompt.prompt()?)
}

async fn auth_status(
    config_path: &Path,
    config: &Config,
    password_store: &dyn PasswordStore,
    client: &Client,
) -> Result<(), Box<dyn Error>> {
    println!("Config path: {}", config_path.display());
    println!(
        "Username: {}",
        config.username.as_deref().unwrap_or("(not configured)")
    );
    println!("Keyring available: {}", password_store.is_available());

    let mut keyring_presence = "not checked".to_string();
    if let Some(username) = &config.username {
        keyring_presence = match password_store.get_password(username) {
            Ok(Some(_)) => "present".to_string(),
            Ok(None) => "missing".to_string(),
            Err(PasswordStoreError::Unavailable(message)) => format!("unavailable ({})", message),
            Err(PasswordStoreError::Unexpected(message)) => format!("error ({})", message),
        };
    }
    println!("Keyring password: {}", keyring_presence);
    println!(
        "Config fallback password: {}",
        if config.password.is_some() {
            "present"
        } else {
            "missing"
        }
    );

    match resolve_credentials(config, password_store) {
        Ok(credentials) => {
            let source = match credentials.source {
                CredentialSource::Keyring => "keyring",
                CredentialSource::ConfigFallback => "config fallback",
            };
            println!("Credential source: {}", source);
            match login(client, &credentials.username, &credentials.password).await {
                Ok(()) => println!("HamAlert login: success"),
                Err(error) => println!("HamAlert login: failed ({})", error),
            }
        }
        Err(error) => {
            println!("Credential source: unavailable ({})", error);
            println!("HamAlert login: not attempted");
        }
    }

    Ok(())
}

fn auth_logout(
    config_path: &Path,
    config: &mut Config,
    password_store: &dyn PasswordStore,
) -> Result<(), Box<dyn Error>> {
    if let Some(username) = &config.username {
        match password_store.delete_password(username) {
            Ok(()) | Err(PasswordStoreError::Unavailable(_)) => {}
            Err(error @ PasswordStoreError::Unexpected(_)) => return Err(error.into()),
        }
    }

    if remove_config_password_for_logout(config) {
        save_config(config_path, config)?;
        println!("Removed stored HamAlert password. Username was kept.");
    } else {
        println!("No configured HamAlert credentials found.");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let config_path = config_path(cli.config_file.clone())?;
    let password_store = KeyringPasswordStore;

    if let Commands::Auth(command) = cli.command {
        let config = load_config_or_default(&config_path)?;
        let client = Client::builder().cookie_store(true).build()?;
        return handle_auth_command(command, &config_path, config, &password_store, &client).await;
    }

    // Load config from file
    let config = load_config(cli.config_file)?;

    // Create a client with cookie jar to maintain session
    let client = Client::builder().cookie_store(true).build()?;

    // Login first
    let credentials = resolve_credentials(&config, &password_store)?;
    login(&client, &credentials.username, &credentials.password).await?;

    // Execute the subcommand
    match cli.command {
        Commands::Auth(_) => unreachable!("auth commands are handled before login"),
        Commands::AddTrigger { callsign, options } => {
            let action_strings: Vec<String> = options
                .actions
                .iter()
                .map(|a| a.as_str().to_string())
                .collect();

            let mode_string = if options.mode.is_empty() {
                None
            } else {
                Some(
                    options
                        .mode
                        .iter()
                        .map(|m| m.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            };

            if callsign.is_empty() {
                return Err("At least one --callsign must be provided".into());
            }
            // Join callsigns with the specified format
            let format = CallsignFormat::from_flags(options.compact, options.one_per_line);
            let combined_callsigns = callsign.join(format.separator());
            add_trigger(
                &client,
                &combined_callsigns,
                &options.comment,
                action_strings,
                mode_string,
            )
            .await?;
        }
        Commands::ImportPoloNotes { url, options } => {
            let callsigns = fetch_polo_notes(&client, &url).await?;

            if callsigns.is_empty() {
                println!("No callsigns found at {}", url);
                return Ok(());
            }

            println!("Found {} callsigns at {}", callsigns.len(), url);

            import_callsigns(&client, callsigns, &options).await?;
        }
        Commands::ImportFile { file, options } => {
            let content = fs::read_to_string(&file)
                .map_err(|e| format!("Failed to read file {}: {}", file.display(), e))?;
            let callsigns = parse_polo_notes_content(&content);

            if callsigns.is_empty() {
                println!("No callsigns found in {}", file.display());
                return Ok(());
            }

            println!("Found {} callsigns in {}", callsigns.len(), file.display());

            import_callsigns(&client, callsigns, &options).await?;
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
        Commands::SetCallsigns {
            trigger_id,
            comment,
            file,
            no_dry_run,
            max_removals,
        } => {
            let content = fs::read_to_string(&file)
                .map_err(|e| format!("Failed to read file {}: {}", file.display(), e))?;
            let desired = parse_callsign_file(&content);

            if desired.is_empty() {
                return Err(format!(
                    "No callsigns found in {}; refusing to empty the trigger",
                    file.display()
                )
                .into());
            }

            let triggers = fetch_triggers(&client).await?;
            let mut trigger =
                select_trigger(&triggers, trigger_id.as_deref(), comment.as_deref())?.clone();
            let (current, shape) = parse_callsign_condition(&trigger.conditions)?;
            let (added, removed, unchanged) = diff_callsigns(&current, &desired);

            println!("Trigger: {}", format_trigger_for_display(&trigger));
            println!(
                "{} added, {} removed, {} unchanged",
                added.len(),
                removed.len(),
                unchanged
            );
            for callsign in &added {
                println!("  + {}", callsign);
            }
            for callsign in &removed {
                println!("  - {}", callsign);
            }

            if added.is_empty() && removed.is_empty() {
                println!("No changes.");
                return Ok(());
            }

            let limit = removal_limit(current.len(), max_removals);
            let refusal = (removed.len() > limit).then(|| {
                format!(
                    "Refusing to remove {} callsigns (limit {}). Review the removals above; \
                     to allow them, re-run with --max-removals {}",
                    removed.len(),
                    limit,
                    removed.len()
                )
            });

            if !no_dry_run {
                if let Some(refusal) = &refusal {
                    println!("\nWarning: applying would fail. {}", refusal);
                }
                println!("\nDry run: no changes made. Re-run with --no-dry-run to apply.");
                return Ok(());
            }

            if let Some(refusal) = refusal {
                return Err(refusal.into());
            }

            apply_callsigns(&mut trigger.conditions, &desired, &shape);
            update_trigger(&client, &trigger).await?;

            // HamAlert can return success without saving, so read the trigger back to confirm
            let triggers = fetch_triggers(&client).await?;
            let saved = select_trigger(&triggers, Some(&trigger.id), None)?;
            let (saved_callsigns, _) = parse_callsign_condition(&saved.conditions)?;
            let (missing, unexpected, _) = diff_callsigns(&saved_callsigns, &desired);
            if !missing.is_empty() || !unexpected.is_empty() {
                return Err(format!(
                    "Trigger '{}' did not save as expected: {} callsigns missing, {} unexpected",
                    trigger.comment,
                    missing.len(),
                    unexpected.len()
                )
                .into());
            }
            println!(
                "Updated trigger '{}' ({} callsigns, verified)",
                trigger.comment,
                saved_callsigns.len()
            );
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
                    let percentage = profile_match_percentage(matched, total);

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
                println!(
                    "Permanent triggers matched: {}/{}",
                    permanent_matched,
                    permanent.len()
                );

                let current_non_permanent = filter_out_permanent(&current_stored, &permanent);

                if profiles.is_empty() {
                    println!("\nNo profiles saved.");
                    return Ok(());
                }

                println!("\nProfile match analysis:");
                let mut best_match: Option<(String, usize, usize)> = None;

                for profile_name in &profiles {
                    let profile = load_profile(profile_name).unwrap_or_default();
                    let (matched, total) =
                        calculate_profile_match(&current_non_permanent, &profile);
                    let percentage = profile_match_percentage(matched, total);

                    let marker = if matched == total && total > 0 {
                        " <- best match"
                    } else {
                        ""
                    };
                    println!(
                        "  {:<15} {}/{} ({}% match){}",
                        profile_name, matched, total, percentage, marker
                    );

                    if best_match.is_none() || matched > best_match.as_ref().unwrap().1 {
                        best_match = Some((profile_name.clone(), matched, total));
                    }
                }

                // Current profile status
                println!(
                    "\nRecorded current profile: {}",
                    current_profile_name.as_deref().unwrap_or("(none)")
                );

                // Check for mismatch
                if let Some((best_name, best_matched, best_total)) = &best_match {
                    let is_in_sync = current_profile_name.as_ref() == Some(best_name)
                        && *best_matched == *best_total;

                    if is_in_sync {
                        println!("Status: ✓ In sync");
                    } else if current_profile_name.is_some()
                        && best_matched == best_total
                        && *best_total > 0
                    {
                        println!(
                            "Status: ⚠ Mismatch - HamAlert matches '{}' better",
                            best_name
                        );
                        println!("\nActions:");
                        println!(
                            "  [U]pdate record to '{}' (no changes to HamAlert)",
                            best_name
                        );
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
                                    let profile_triggers =
                                        filter_out_permanent(&current_stored, &permanent);
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
                        let mode = t
                            .conditions
                            .get("mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("any");
                        let callsign = t
                            .conditions
                            .get("callsign")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
                    }
                }
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
            ProfileCommands::Switch { name, no_dry_run } => {
                // Track whether we modified the profile during dry-run
                let mut profile_modified = false;

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
                println!(
                    "Current profile: {}",
                    current_profile_name.as_deref().unwrap_or("(none)")
                );
                println!("Switching to: {}\n", name);

                println!(
                    "Permanent triggers (unchanged): {}",
                    permanent_triggers.len()
                );
                if !permanent_triggers.is_empty() {
                    for t in &permanent_triggers {
                        let mode = t
                            .conditions
                            .get("mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("any");
                        let callsign = t
                            .conditions
                            .get("callsign")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
                    }
                }

                println!("\nWill DELETE {} triggers:", to_delete.len());
                for t in &to_delete {
                    println!("  - {}", format_trigger_for_display(t));
                }

                println!(
                    "\nWill CREATE {} triggers from '{}':",
                    target_profile.len(),
                    name
                );
                for t in &target_profile {
                    let mode = t
                        .conditions
                        .get("mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("any");
                    let callsign = t
                        .conditions
                        .get("callsign")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    println!("  - [{}] {} - \"{}\"", mode, callsign, t.comment);
                }

                // Handle unexpected triggers
                if !unexpected.is_empty() {
                    println!(
                        "\n⚠ Found {} unexpected triggers (not permanent, not in current profile):",
                        unexpected.len()
                    );
                    for t in &unexpected {
                        let mode = t
                            .conditions
                            .get("mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("any");
                        let callsign = t
                            .conditions
                            .get("callsign")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
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
                                    let mut updated_profile =
                                        current_profile_data.unwrap_or_default();
                                    for t in &unexpected {
                                        if !updated_profile.iter().any(|p| triggers_match(p, t)) {
                                            updated_profile.push(t.clone());
                                        }
                                    }
                                    save_profile(current_name, &updated_profile)?;
                                    println!(
                                        "Updated '{}' profile with {} additional triggers.",
                                        current_name,
                                        unexpected.len()
                                    );
                                    profile_modified = true;
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
                    if profile_modified {
                        println!("\nNote: Profile was updated with unexpected triggers.");
                    }
                    println!("DRY RUN - No trigger changes made on HamAlert.");
                    println!("Run with --no-dry-run to execute the switch.");
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
                println!(
                    "\nBacked up {} triggers to {}",
                    current_triggers.len(),
                    backup_path.display()
                );

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
    use clap::error::ErrorKind;

    struct FakePasswordStore {
        result: PasswordStoreResult<Option<String>>,
    }

    impl FakePasswordStore {
        fn with_password(password: &str) -> Self {
            Self {
                result: Ok(Some(password.to_string())),
            }
        }

        fn missing() -> Self {
            Self { result: Ok(None) }
        }

        fn unavailable(message: &str) -> Self {
            Self {
                result: Err(PasswordStoreError::Unavailable(message.to_string())),
            }
        }
    }

    impl PasswordStore for FakePasswordStore {
        fn get_password(&self, _username: &str) -> PasswordStoreResult<Option<String>> {
            match &self.result {
                Ok(password) => Ok(password.clone()),
                Err(PasswordStoreError::Unavailable(message)) => {
                    Err(PasswordStoreError::Unavailable(message.clone()))
                }
                Err(PasswordStoreError::Unexpected(message)) => {
                    Err(PasswordStoreError::Unexpected(message.clone()))
                }
            }
        }

        fn set_password(&self, _username: &str, _password: &str) -> PasswordStoreResult<()> {
            Ok(())
        }

        fn delete_password(&self, _username: &str) -> PasswordStoreResult<()> {
            Ok(())
        }

        fn is_available(&self) -> bool {
            !matches!(self.result, Err(PasswordStoreError::Unavailable(_)))
        }
    }

    #[test]
    fn test_resolve_credentials_prefers_keyring() {
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: Some("config-secret".to_string()),
        };
        let store = FakePasswordStore::with_password("keyring-secret");
        let password_store: &dyn PasswordStore = &store;

        assert!(password_store.is_available());
        password_store.set_password("N0CALL", "unused").unwrap();
        password_store.delete_password("N0CALL").unwrap();

        let credentials = resolve_credentials(&config, password_store).unwrap();

        assert_eq!(credentials.username, "N0CALL");
        assert_eq!(credentials.password, "keyring-secret");
        assert_eq!(credentials.source, CredentialSource::Keyring);
    }

    #[test]
    fn test_resolve_credentials_falls_back_when_keyring_missing() {
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: Some("config-secret".to_string()),
        };
        let store = FakePasswordStore::missing();

        let credentials = resolve_credentials(&config, &store).unwrap();

        assert_eq!(credentials.username, "N0CALL");
        assert_eq!(credentials.password, "config-secret");
        assert_eq!(credentials.source, CredentialSource::ConfigFallback);
    }

    #[test]
    fn test_resolve_credentials_falls_back_when_keyring_unavailable() {
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: Some("config-secret".to_string()),
        };
        let store = FakePasswordStore::unavailable("no store");

        let credentials = resolve_credentials(&config, &store).unwrap();

        assert_eq!(credentials.username, "N0CALL");
        assert_eq!(credentials.password, "config-secret");
        assert_eq!(credentials.source, CredentialSource::ConfigFallback);
    }

    #[test]
    fn test_resolve_credentials_errors_without_password() {
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: None,
        };
        let store = FakePasswordStore::missing();

        let error = match resolve_credentials(&config, &store) {
            Ok(_) => panic!("expected missing password error"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("No password found"));
        assert!(error.to_string().contains("hamalert-cli auth login"));
    }

    #[test]
    fn test_login_body_indicates_invalid_credentials() {
        let body = r#"
            <div class="alert alert-danger" role="alert">
                Login failed; please check username and password.
            </div>
        "#;

        assert!(login_body_indicates_invalid_credentials(body));
    }

    #[test]
    fn test_keyring_storage_access_error_is_unavailable() {
        let error = keyring::Error::NoStorageAccess(Box::new(std::io::Error::other("locked")));

        let mapped = map_keyring_error(error);

        assert!(matches!(mapped, PasswordStoreError::Unavailable(_)));
    }

    #[test]
    fn test_config_accepts_username_only() {
        let config: Config = toml::from_str("username = \"N0CALL\"\n").unwrap();
        assert_eq!(config.username.as_deref(), Some("N0CALL"));
        assert!(config.password.is_none());
    }

    #[test]
    fn test_config_accepts_legacy_password() {
        let config: Config =
            toml::from_str("username = \"N0CALL\"\npassword = \"secret\"\n").unwrap();
        assert_eq!(config.username.as_deref(), Some("N0CALL"));
        assert_eq!(config.password.as_deref(), Some("secret"));
    }

    #[test]
    fn test_auth_login_accepts_password_stdin() {
        let cli = Cli::try_parse_from([
            "hamalert-cli",
            "auth",
            "login",
            "--username",
            "N0CALL",
            "--password-stdin",
        ])
        .unwrap();

        match cli.command {
            Commands::Auth(AuthCommands::Login { username, password }) => {
                assert_eq!(username.as_deref(), Some("N0CALL"));
                assert!(password.password_stdin);
                assert!(password.password_env.is_none());
                assert!(password.password.is_none());
            }
            _ => panic!("expected auth login command"),
        }
    }

    #[test]
    fn test_auth_login_rejects_multiple_password_sources() {
        let error = Cli::try_parse_from([
            "hamalert-cli",
            "auth",
            "login",
            "--password-stdin",
            "--password",
            "secret",
        ])
        .unwrap_err();

        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_auth_logout_parses() {
        let cli = Cli::try_parse_from(["hamalert-cli", "auth", "logout"]).unwrap();

        match cli.command {
            Commands::Auth(AuthCommands::Logout) => {}
            _ => panic!("expected auth logout command"),
        }
    }

    #[test]
    fn test_password_input_from_env() {
        unsafe {
            std::env::set_var("HAMALERT_TEST_PASSWORD", "env-secret");
        }

        let input = PasswordInput {
            password_stdin: false,
            password_env: Some("HAMALERT_TEST_PASSWORD".to_string()),
            password: None,
        };

        let password = password_from_input(&input, "").unwrap();

        unsafe {
            std::env::remove_var("HAMALERT_TEST_PASSWORD");
        }

        assert_eq!(password.as_deref(), Some("env-secret"));
    }

    #[test]
    fn test_password_input_from_flag() {
        let input = PasswordInput {
            password_stdin: false,
            password_env: None,
            password: Some("flag-secret".to_string()),
        };

        let password = password_from_input(&input, "").unwrap();

        assert_eq!(password.as_deref(), Some("flag-secret"));
    }

    #[test]
    fn test_auth_login_uses_config_username_for_supplied_password() {
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: None,
        };

        let credentials =
            non_interactive_login_credentials(&config, None, Some("flag-secret".to_string()))
                .unwrap();

        assert_eq!(credentials.0, "N0CALL");
        assert_eq!(credentials.1, "flag-secret");
    }

    #[test]
    fn test_auth_logout_default_config_is_noop() {
        let mut config = Config::default();

        assert!(!remove_config_password_for_logout(&mut config));
        assert!(config.username.is_none());
        assert!(config.password.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_save_config_writes_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "hamalert-cli-test-config-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = Config {
            username: Some("N0CALL".to_string()),
            password: Some("secret".to_string()),
        };

        save_config(&path, &config).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        fs::remove_file(&path).unwrap();

        assert_eq!(mode, 0o600);
    }

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

    #[test]
    fn test_profile_match_percentage() {
        assert_eq!(profile_match_percentage(1, 2), 50);
        assert_eq!(profile_match_percentage(2, 2), 100);
    }

    #[test]
    fn test_profile_match_percentage_defaults_to_full_match_for_empty_profile() {
        assert_eq!(profile_match_percentage(0, 0), 100);
    }

    fn trigger(id: &str, comment: &str, conditions: serde_json::Value) -> Trigger {
        serde_json::from_value(json!({
            "_id": id,
            "conditions": conditions,
            "actions": ["url"],
            "comment": comment,
        }))
        .unwrap()
    }

    #[test]
    fn test_parse_callsign_file_dedupes_and_uppercases() {
        let content = "# members\nw1abc\nVE7XXX\n\nW1ABC\n// note\n";
        assert_eq!(parse_callsign_file(content), vec!["VE7XXX", "W1ABC"]);
    }

    #[test]
    fn test_parse_callsign_condition_string_shapes() {
        let (calls, shape) =
            parse_callsign_condition(&json!({"callsign": "W1ABC, ve7xxx"})).unwrap();
        assert_eq!(calls, vec!["W1ABC", "VE7XXX"]);
        assert_eq!(shape, CallsignShape::Text(", "));

        let (_, shape) = parse_callsign_condition(&json!({"callsign": "W1ABC,VE7XXX"})).unwrap();
        assert_eq!(shape, CallsignShape::Text(","));

        let (calls, shape) =
            parse_callsign_condition(&json!({"callsign": "W1ABC\nVE7XXX\n"})).unwrap();
        assert_eq!(calls, vec!["W1ABC", "VE7XXX"]);
        assert_eq!(shape, CallsignShape::Text("\n"));
    }

    #[test]
    fn test_parse_callsign_condition_array() {
        let (calls, shape) =
            parse_callsign_condition(&json!({"callsign": ["W1ABC", "ve7xxx"]})).unwrap();
        assert_eq!(calls, vec!["W1ABC", "VE7XXX"]);
        assert_eq!(shape, CallsignShape::List);
    }

    #[test]
    fn test_parse_callsign_condition_errors() {
        assert!(parse_callsign_condition(&json!({"fullCallsign": "W1ABC"})).is_err());
        assert!(parse_callsign_condition(&json!({"source": "sotawatch"})).is_err());
        assert!(parse_callsign_condition(&json!({"callsign": 5})).is_err());
    }

    #[test]
    fn test_apply_callsigns_preserves_other_conditions() {
        let mut conditions = json!({"callsign": "W1ABC", "source": ["sotawatch"], "band": ["20m"]});
        let desired = vec!["K7SAM".to_string(), "W1ABC".to_string()];
        apply_callsigns(&mut conditions, &desired, &CallsignShape::Text(", "));
        assert_eq!(
            conditions,
            json!({"callsign": "K7SAM, W1ABC", "source": ["sotawatch"], "band": ["20m"]})
        );

        apply_callsigns(&mut conditions, &desired, &CallsignShape::List);
        assert_eq!(conditions["callsign"], json!(["K7SAM", "W1ABC"]));
        assert_eq!(conditions["source"], json!(["sotawatch"]));
    }

    #[test]
    fn test_diff_callsigns() {
        let current = vec!["W1ABC".to_string(), "VE7XXX".to_string()];
        let desired = vec!["VE7XXX".to_string(), "K7SAM".to_string()];
        let (added, removed, unchanged) = diff_callsigns(&current, &desired);
        assert_eq!(added, vec!["K7SAM"]);
        assert_eq!(removed, vec!["W1ABC"]);
        assert_eq!(unchanged, 1);
    }

    #[test]
    fn test_removal_limit() {
        assert_eq!(removal_limit(10, None), 5);
        assert_eq!(removal_limit(200, None), 20);
        assert_eq!(removal_limit(200, Some(3)), 3);
    }

    #[test]
    fn test_select_trigger() {
        let triggers = vec![
            trigger("a1", "SOTA spots", json!({"callsign": "W1ABC"})),
            trigger("b2", "DX", json!({"callsign": "K7SAM"})),
            trigger("c3", "DX", json!({"callsign": "N6AAA"})),
        ];
        assert_eq!(
            select_trigger(&triggers, Some("b2"), None).unwrap().id,
            "b2"
        );
        assert_eq!(
            select_trigger(&triggers, None, Some("SOTA spots"))
                .unwrap()
                .id,
            "a1"
        );
        assert!(select_trigger(&triggers, Some("zz"), None).is_err());
        assert!(select_trigger(&triggers, None, Some("DX")).is_err());
    }
}
