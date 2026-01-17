pub mod schema;

use color_eyre::{eyre::eyre, Result};
use rusqlite::Connection;
use std::path::PathBuf;

/// Database connection wrapper for caching
pub struct Database {
  conn: Connection,
}

impl Database {
  /// Open or create the database at the default location
  pub fn open() -> Result<Self> {
    let path = Self::default_path()?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)
        .map_err(|e| eyre!("Failed to create database directory: {}", e))?;
    }

    let conn = Connection::open(&path)
      .map_err(|e| eyre!("Failed to open database at {}: {}", path.display(), e))?;

    let db = Self { conn };
    db.run_migrations()?;

    Ok(db)
  }

  /// Get the default database path
  fn default_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
      .or_else(|| dirs::home_dir().map(|p| p.join(".local/share")))
      .ok_or_else(|| eyre!("Could not determine data directory"))?;

    Ok(data_dir.join("j9s").join("cache.db"))
  }

  /// Run database migrations
  fn run_migrations(&self) -> Result<()> {
    self
      .conn
      .execute_batch(schema::SCHEMA)
      .map_err(|e| eyre!("Failed to run migrations: {}", e))?;
    Ok(())
  }

  /// Get a reference to the connection
  pub fn conn(&self) -> &Connection {
    &self.conn
  }
}
