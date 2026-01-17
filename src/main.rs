mod app;
mod commands;
mod config;
mod db;
mod event;
mod jira;
mod ui;

use clap::Parser;
use color_eyre::Result;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize file logging. Returns guard that must be held for duration of program.
fn init_logging() -> Option<WorkerGuard> {
  // Use XDG state directory, falling back to data directory
  let log_dir = dirs::state_dir()
    .or_else(dirs::data_dir)
    .map(|d| d.join("j9s"))?;

  // Create directory if it doesn't exist
  std::fs::create_dir_all(&log_dir).ok()?;

  let file_appender = tracing_appender::rolling::daily(&log_dir, "j9s.log");
  let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

  let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

  tracing_subscriber::registry()
    .with(filter)
    .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
    .init();

  Some(guard)
}

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
  let _log_guard = init_logging();

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
