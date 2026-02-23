//! Core traits for the caching system.

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
