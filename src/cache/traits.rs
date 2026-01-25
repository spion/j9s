//! Core traits and types for the caching system.

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};

/// Trait for entities that can be cached.
///
/// Implementors must provide a unique cache key and optionally an updated_at timestamp
/// for incremental fetching.
pub trait Cacheable: Clone + Send + Sync + Serialize + DeserializeOwned {
  /// Unique identifier for this entity (e.g., issue key, board id)
  fn cache_key(&self) -> String;

  /// Last modification timestamp (ISO 8601).
  /// Returns None if the entity doesn't track modification time.
  fn updated_at(&self) -> Option<&str>;

  /// Entity type name for storage organization (e.g., "issue", "board")
  fn entity_type() -> &'static str;
}

/// Result from a cache operation, including data and metadata about the source.
#[derive(Debug, Clone)]
pub struct CacheResult<T> {
  /// The actual data
  pub data: T,
  /// Where the data came from
  pub source: CacheSource,
  /// When the data was cached (if from cache)
  pub cached_at: Option<DateTime<Utc>>,
}

impl<T> CacheResult<T> {
  /// Create a new cache result from fresh network data.
  pub fn from_network(data: T) -> Self {
    Self {
      data,
      source: CacheSource::Network,
      cached_at: None,
    }
  }

  /// Create a new cache result from cached data.
  pub fn from_cache(data: T, cached_at: DateTime<Utc>, is_stale: bool) -> Self {
    Self {
      data,
      source: if is_stale {
        CacheSource::CacheStale
      } else {
        CacheSource::CacheFresh
      },
      cached_at: Some(cached_at),
    }
  }

  /// Create a new cache result for offline mode.
  pub fn offline(data: T, cached_at: DateTime<Utc>) -> Self {
    Self {
      data,
      source: CacheSource::Offline,
      cached_at: Some(cached_at),
    }
  }
}

/// Indicates where cached data came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheSource {
  /// Fresh data from network
  Network,
  /// Data from cache, still considered fresh
  CacheFresh,
  /// Data from cache, considered stale but network fetch in progress or failed
  CacheStale,
  /// Offline mode - network unavailable, serving cached data
  Offline,
}
