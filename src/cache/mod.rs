//! Generic caching layer for data persistence and offline support.
//!
//! This module provides a Jira-agnostic caching mechanism that:
//! - Caches entities with key + updated_at fields
//! - Handles full lists, partial lists, and individual item queries
//! - Supports incremental fetching via `updated_at > last_fetched_updated_at`
//! - Provides basic offline mode (serve stale cache when network unavailable)

mod layer;
mod storage;
mod traits;

pub use layer::CacheLayer;
pub use storage::SqliteStorage;
pub use traits::Cacheable;
