//! Cache storage trait and SQLite implementation.

use chrono::{DateTime, Utc};
use color_eyre::{eyre::eyre, Result};
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::traits::Cacheable;

/// Result of a cached query lookup.
#[derive(Debug, Clone)]
pub struct CachedQueryResult<T> {
  /// The cached entities in order
  pub entities: Vec<T>,
  /// When the query result was cached
  pub cached_at: DateTime<Utc>,
  /// Maximum updated_at value for incremental fetching
  pub max_updated: Option<String>,
}

/// A single cached entity.
#[derive(Debug, Clone)]
pub struct CachedEntity<T> {
  /// The cached entity
  pub entity: T,
  /// When the entity was cached
  pub cached_at: DateTime<Utc>,
}

/// Trait for cache storage backends.
pub trait CacheStorage: Send + Sync {
  /// Store entities from a query result.
  fn store_query_result<T: Cacheable>(&self, key: &str, entities: &[T]) -> Result<()>;

  /// Get cached entities for a query.
  fn get_query_result<T: Cacheable>(&self, key: &str) -> Result<Option<CachedQueryResult<T>>>;

  /// Get a single entity by key.
  fn get_entity<T: Cacheable>(&self, entity_key: &str) -> Result<Option<CachedEntity<T>>>;

  /// Store a single entity.
  fn store_entity<T: Cacheable>(&self, entity: &T) -> Result<()>;

  /// Get the max updated_at for incremental fetching.
  fn get_max_updated(&self, key: &str) -> Result<Option<String>>;

  /// Merge new entities into an existing query result (upsert by key).
  fn merge_query_result<T: Cacheable>(&self, key: &str, new_entities: &[T]) -> Result<()>;
}

/// Storage implementation that doesn't cache anything.
/// Used when caching is disabled - all operations are no-ops.
pub struct NoopStorage;

impl CacheStorage for NoopStorage {
  fn store_query_result<T: Cacheable>(&self, _key: &str, _entities: &[T]) -> Result<()> {
    Ok(()) // Discard
  }

  fn get_query_result<T: Cacheable>(&self, _key: &str) -> Result<Option<CachedQueryResult<T>>> {
    Ok(None) // Always miss
  }

  fn get_entity<T: Cacheable>(&self, _entity_key: &str) -> Result<Option<CachedEntity<T>>> {
    Ok(None) // Always miss
  }

  fn store_entity<T: Cacheable>(&self, _entity: &T) -> Result<()> {
    Ok(()) // Discard
  }

  fn get_max_updated(&self, _key: &str) -> Result<Option<String>> {
    Ok(None) // No cached data
  }

  fn merge_query_result<T: Cacheable>(&self, _key: &str, _new_entities: &[T]) -> Result<()> {
    Ok(()) // Discard
  }
}

/// SQLite-based cache storage implementation.
pub struct SqliteStorage {
  conn: Mutex<Connection>,
}

impl SqliteStorage {
  /// Create a new SQLite storage at the default location.
  pub fn open() -> Result<Self> {
    let path = Self::default_path()?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)
        .map_err(|e| eyre!("Failed to create cache directory: {}", e))?;
    }

    let conn = Connection::open(&path)
      .map_err(|e| eyre!("Failed to open cache database at {}: {}", path.display(), e))?;

    let storage = Self {
      conn: Mutex::new(conn),
    };
    storage.run_migrations()?;

    Ok(storage)
  }

  /// Get the default database path.
  fn default_path() -> Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir()
      .or_else(|| dirs::home_dir().map(|p| p.join(".local/share")))
      .ok_or_else(|| eyre!("Could not determine data directory"))?;

    Ok(data_dir.join("j9s").join("cache.db"))
  }

  /// Run database migrations for cache tables.
  fn run_migrations(&self) -> Result<()> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;

    conn
      .execute_batch(CACHE_SCHEMA)
      .map_err(|e| eyre!("Failed to run cache migrations: {}", e))?;

    Ok(())
  }
}

/// Schema for cache tables.
const CACHE_SCHEMA: &str = r#"
-- Generic entity cache (stores serialized JSON)
CREATE TABLE IF NOT EXISTS entity_cache (
    entity_type TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    data BLOB NOT NULL,
    updated_at TEXT,
    cached_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (entity_type, entity_key)
);

CREATE INDEX IF NOT EXISTS idx_entity_cache_updated
    ON entity_cache(entity_type, updated_at);

-- Query result tracking
CREATE TABLE IF NOT EXISTS query_cache (
    query_hash TEXT PRIMARY KEY,
    query_description TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    max_updated TEXT,
    cached_at TEXT NOT NULL DEFAULT (datetime('now')),
    result_count INTEGER NOT NULL
);

-- Query to entity mapping (preserves order)
CREATE TABLE IF NOT EXISTS query_results (
    query_hash TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (query_hash, entity_key),
    FOREIGN KEY (query_hash) REFERENCES query_cache(query_hash) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_query_results_hash ON query_results(query_hash);
"#;

impl CacheStorage for SqliteStorage {
  fn store_query_result<T: Cacheable>(&self, key: &str, entities: &[T]) -> Result<()> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;
    let entity_type = T::entity_type();

    // Calculate max_updated from entities
    let max_updated = entities
      .iter()
      .filter_map(|e| e.updated_at())
      .max()
      .map(String::from);

    // Start transaction
    conn
      .execute("BEGIN TRANSACTION", [])
      .map_err(|e| eyre!("Failed to begin transaction: {}", e))?;

    // Delete existing query results
    conn
      .execute(
        "DELETE FROM query_results WHERE query_hash = ?",
        params![key],
      )
      .map_err(|e| eyre!("Failed to delete old query results: {}", e))?;

    // Insert/update query cache
    conn
      .execute(
        "INSERT OR REPLACE INTO query_cache (query_hash, query_description, entity_type, max_updated, cached_at, result_count)
         VALUES (?, ?, ?, ?, datetime('now'), ?)",
        params![key, key, entity_type, max_updated, entities.len()],
      )
      .map_err(|e| eyre!("Failed to update query cache: {}", e))?;

    // Store entities and query results
    for (position, entity) in entities.iter().enumerate() {
      let entity_key = entity.cache_key();
      let data =
        serde_json::to_vec(entity).map_err(|e| eyre!("Failed to serialize entity: {}", e))?;
      let updated_at = entity.updated_at();

      // Store entity
      conn
        .execute(
          "INSERT OR REPLACE INTO entity_cache (entity_type, entity_key, data, updated_at, cached_at)
           VALUES (?, ?, ?, ?, datetime('now'))",
          params![entity_type, entity_key, data, updated_at],
        )
        .map_err(|e| eyre!("Failed to store entity: {}", e))?;

      // Store query result mapping
      conn
        .execute(
          "INSERT OR REPLACE INTO query_results (query_hash, entity_key, position)
           VALUES (?, ?, ?)",
          params![key, entity_key, position],
        )
        .map_err(|e| eyre!("Failed to store query result: {}", e))?;
    }

    conn
      .execute("COMMIT", [])
      .map_err(|e| eyre!("Failed to commit transaction: {}", e))?;

    Ok(())
  }

  fn get_query_result<T: Cacheable>(
    &self,
    query_hash: &str,
  ) -> Result<Option<CachedQueryResult<T>>> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;
    let entity_type = T::entity_type();

    // Get query metadata
    let mut stmt = conn
      .prepare(
        "SELECT cached_at, max_updated FROM query_cache
         WHERE query_hash = ? AND entity_type = ?",
      )
      .map_err(|e| eyre!("Failed to prepare query: {}", e))?;

    let query_info: Option<(String, Option<String>)> = stmt
      .query_row(params![query_hash, entity_type], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })
      .ok();

    let (cached_at_str, max_updated) = match query_info {
      Some(info) => info,
      None => return Ok(None),
    };

    let cached_at = parse_datetime(&cached_at_str)?;

    // Get entities in order
    let mut stmt = conn
      .prepare(
        "SELECT ec.data FROM entity_cache ec
         INNER JOIN query_results qr ON ec.entity_type = ? AND ec.entity_key = qr.entity_key
         WHERE qr.query_hash = ?
         ORDER BY qr.position",
      )
      .map_err(|e| eyre!("Failed to prepare entity query: {}", e))?;

    let entities: Vec<T> = stmt
      .query_map(params![entity_type, query_hash], |row| {
        let data: Vec<u8> = row.get(0)?;
        Ok(data)
      })
      .map_err(|e| eyre!("Failed to query entities: {}", e))?
      .filter_map(|r| r.ok())
      .filter_map(|data| serde_json::from_slice(&data).ok())
      .collect();

    Ok(Some(CachedQueryResult {
      entities,
      cached_at,
      max_updated,
    }))
  }

  fn get_entity<T: Cacheable>(&self, entity_key: &str) -> Result<Option<CachedEntity<T>>> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;
    let entity_type = T::entity_type();

    let mut stmt = conn
      .prepare(
        "SELECT data, cached_at FROM entity_cache
         WHERE entity_type = ? AND entity_key = ?",
      )
      .map_err(|e| eyre!("Failed to prepare query: {}", e))?;

    let result: Option<(Vec<u8>, String)> = stmt
      .query_row(params![entity_type, entity_key], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })
      .ok();

    match result {
      Some((data, cached_at_str)) => {
        let entity: T = serde_json::from_slice(&data)
          .map_err(|e| eyre!("Failed to deserialize entity: {}", e))?;
        let cached_at = parse_datetime(&cached_at_str)?;
        Ok(Some(CachedEntity { entity, cached_at }))
      }
      None => Ok(None),
    }
  }

  fn store_entity<T: Cacheable>(&self, entity: &T) -> Result<()> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;
    let entity_type = T::entity_type();
    let key = entity.cache_key();
    let data =
      serde_json::to_vec(entity).map_err(|e| eyre!("Failed to serialize entity: {}", e))?;
    let updated_at = entity.updated_at();

    conn
      .execute(
        "INSERT OR REPLACE INTO entity_cache (entity_type, entity_key, data, updated_at, cached_at)
         VALUES (?, ?, ?, ?, datetime('now'))",
        params![entity_type, key, data, updated_at],
      )
      .map_err(|e| eyre!("Failed to store entity: {}", e))?;

    Ok(())
  }

  fn get_max_updated(&self, query_hash: &str) -> Result<Option<String>> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;

    let mut stmt = conn
      .prepare("SELECT max_updated FROM query_cache WHERE query_hash = ?")
      .map_err(|e| eyre!("Failed to prepare query: {}", e))?;

    let result: Option<Option<String>> = stmt.query_row(params![query_hash], |row| row.get(0)).ok();

    Ok(result.flatten())
  }

  fn merge_query_result<T: Cacheable>(&self, key: &str, new_entities: &[T]) -> Result<()> {
    let conn = self
      .conn
      .lock()
      .map_err(|e| eyre!("Lock poisoned: {}", e))?;
    let entity_type = T::entity_type();

    // Get existing entities for this query
    let mut existing_entities: Vec<T> = Vec::new();
    let mut existing_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    {
      let mut stmt = conn
        .prepare(
          "SELECT ec.data FROM entity_cache ec
           INNER JOIN query_results qr ON ec.entity_type = ? AND ec.entity_key = qr.entity_key
           WHERE qr.query_hash = ?
           ORDER BY qr.position",
        )
        .map_err(|e| eyre!("Failed to prepare entity query: {}", e))?;

      let entities: Vec<Vec<u8>> = stmt
        .query_map(params![entity_type, key], |row| row.get(0))
        .map_err(|e| eyre!("Failed to query entities: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

      for data in entities {
        if let Ok(entity) = serde_json::from_slice::<T>(&data) {
          existing_keys.insert(entity.cache_key());
          existing_entities.push(entity);
        }
      }
    }

    // Merge: update existing entities, add new ones
    for new_entity in new_entities {
      let entity_key = new_entity.cache_key();
      if existing_keys.contains(&entity_key) {
        // Update existing entity in place
        for existing in &mut existing_entities {
          if existing.cache_key() == entity_key {
            *existing = new_entity.clone();
            break;
          }
        }
      } else {
        // Add new entity at the beginning (most recent)
        existing_entities.insert(0, new_entity.clone());
        existing_keys.insert(entity_key);
      }
    }

    // Drop the lock before calling store_query_result
    drop(conn);

    // Store the merged result
    self.store_query_result(key, &existing_entities)
  }
}

/// Parse a datetime string from SQLite format.
fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
  // SQLite stores as "YYYY-MM-DD HH:MM:SS"
  chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
    .map(|dt| dt.and_utc())
    .map_err(|e| eyre!("Failed to parse datetime '{}': {}", s, e))
}
