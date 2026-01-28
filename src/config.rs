use color_eyre::{eyre::eyre, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
  pub jira: JiraConfig,
  pub default_project: Option<String>,
  /// Custom title for header (defaults to Jira domain if not set)
  pub title: Option<String>,
  #[serde(default)]
  pub boards: BoardsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BoardsConfig {
  /// Swimlane names to hide in board views (case-insensitive)
  #[serde(default, deserialize_with = "deserialize_lowercase_set")]
  pub hide_swimlanes: BTreeSet<String>,
}

fn deserialize_lowercase_set<'de, D>(deserializer: D) -> Result<BTreeSet<String>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let v: Vec<String> = Vec::deserialize(deserializer)?;
  Ok(v.into_iter().map(|s| s.to_lowercase()).collect())
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
  /// Auto-detect based on URL: .atlassian.net = cloud, else on-premise
  #[default]
  Auto,
  /// Jira Cloud - uses Basic auth (email + API token as password)
  Cloud,
  /// Jira On-premise - uses Bearer auth (PAT) or Basic auth fallback
  Onpremise,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraConfig {
  pub url: String,
  pub email: String,
  /// Custom field name for epic link (e.g., "customfield_10014")
  pub epic_field: Option<String>,
  /// Authentication type: auto, cloud, or onpremise
  #[serde(default)]
  pub auth_type: AuthType,
}

impl Config {
  /// Load configuration from file.
  ///
  /// Search order:
  /// 1. Explicit path if provided
  /// 2. ./j9s.yaml (current directory)
  /// 3. $XDG_CONFIG_HOME/j9s/config.yaml
  /// 4. ~/.config/j9s/config.yaml
  pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
    let path = if let Some(p) = explicit_path {
      if p.exists() {
        Some(p.to_path_buf())
      } else {
        return Err(eyre!("Config file not found: {}", p.display()));
      }
    } else {
      Self::find_config_file()
    };

    match path {
      Some(p) => Self::load_from_path(&p),
      None => Err(eyre!(
        "No configuration file found. Create one at ~/.config/j9s/config.yaml\n\
                 See config.example.yaml for the format."
      )),
    }
  }

  fn find_config_file() -> Option<PathBuf> {
    // Check current directory
    let local = PathBuf::from("j9s.yaml");
    if local.exists() {
      return Some(local);
    }

    // Check XDG config directory
    if let Some(config_dir) = dirs::config_dir() {
      let xdg_path = config_dir.join("j9s").join("config.yaml");
      if xdg_path.exists() {
        return Some(xdg_path);
      }
    }

    None
  }

  fn load_from_path(path: &Path) -> Result<Self> {
    let contents = std::fs::read_to_string(path)
      .map_err(|e| eyre!("Failed to read config file {}: {}", path.display(), e))?;

    let config: Config = serde_yaml::from_str(&contents)
      .map_err(|e| eyre!("Failed to parse config file {}: {}", path.display(), e))?;

    Ok(config)
  }

  /// Get the Jira API token from environment variables.
  ///
  /// Checks J9S_JIRA_TOKEN first, then JIRA_API_TOKEN as fallback.
  pub fn get_api_token() -> Result<String> {
    std::env::var("J9S_JIRA_TOKEN")
      .or_else(|_| std::env::var("JIRA_API_TOKEN"))
      .map_err(|_| {
        eyre!(
          "Jira API token not found. Set J9S_JIRA_TOKEN or JIRA_API_TOKEN environment variable."
        )
      })
  }

  /// Get the Jira password from environment variables.
  ///
  /// Checks J9S_JIRA_PASSWORD.
  pub fn get_password() -> Result<String> {
    std::env::var("J9S_JIRA_PASSWORD")
      .map_err(|_| eyre!("Jira password not found. Set J9S_JIRA_PASSWORD environment variable."))
  }
}
