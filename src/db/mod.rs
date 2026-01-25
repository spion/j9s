use color_eyre::{eyre::eyre, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Open the application database, creating it if needed.
pub fn open_connection() -> Result<Arc<Mutex<Connection>>> {
  let path = default_path()?;

  // Ensure parent directory exists
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)
      .map_err(|e| eyre!("Failed to create database directory: {}", e))?;
  }

  let conn = Connection::open(&path)
    .map_err(|e| eyre!("Failed to open database at {}: {}", path.display(), e))?;

  Ok(Arc::new(Mutex::new(conn)))
}

/// Get the default database path.
fn default_path() -> Result<PathBuf> {
  let data_dir = dirs::data_dir()
    .or_else(|| dirs::home_dir().map(|p| p.join(".local/share")))
    .ok_or_else(|| eyre!("Could not determine data directory"))?;

  Ok(data_dir.join("j9s").join("cache.db"))
}
