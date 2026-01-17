mod app;
mod config;
mod db;
mod event;
mod jira;
mod ui;

use clap::Parser;
use color_eyre::Result;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "j9s")]
#[command(about = "A terminal UI for Jira, inspired by k9s")]
#[command(version)]
struct Args {
  /// Path to config file (default: $XDG_CONFIG_HOME/j9s/config.yaml)
  #[arg(short, long)]
  config: Option<PathBuf>,

  /// Jira project key to use
  #[arg(short, long)]
  project: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
  color_eyre::install()?;

  let args = Args::parse();

  // Load configuration
  let config = config::Config::load(args.config.as_deref())?;

  // Override project if specified on command line
  let config = if let Some(project) = args.project {
    config::Config {
      default_project: Some(project),
      ..config
    }
  } else {
    config
  };

  // Initialize and run the app
  let mut app = app::App::new(config).await?;
  app.run().await?;

  Ok(())
}
