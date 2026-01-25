//! Cache layer that orchestrates caching logic with network fetching.

use chrono::{Duration, Utc};
use color_eyre::Result;
use std::future::Future;
use std::sync::Arc;

use super::storage::CacheStorage;
use super::traits::{CacheResult, Cacheable};

/// Cache layer that manages caching logic and network fetching.
///
/// This layer sits between the application and the network client,
/// providing transparent caching with offline support.
pub struct CacheLayer<S: CacheStorage> {
  storage: Arc<S>,
  /// How long before cached data is considered stale
  stale_time: Duration,
}

impl<S: CacheStorage> CacheLayer<S> {
  /// Create a new cache layer with the given storage backend.
  pub fn new(storage: S) -> Self {
    Self {
      storage: Arc::new(storage),
      stale_time: Duration::minutes(5),
    }
  }

  /// Set the stale time for cached data.
  #[allow(dead_code)]
  pub fn with_stale_time(mut self, stale_time: Duration) -> Self {
    self.stale_time = stale_time;
    self
  }

  /// Check if cached data is stale based on cached_at timestamp.
  fn is_stale(&self, cached_at: chrono::DateTime<Utc>) -> bool {
    Utc::now() - cached_at > self.stale_time
  }

  /// Fetch a list with cache-first strategy.
  ///
  /// 1. Check cache - if fresh, return immediately
  /// 2. If stale/missing, fetch from network
  /// 3. On network failure, return stale cache (offline mode)
  /// 4. Update cache with new data
  ///
  /// The `key` parameter is used as the cache lookup key (e.g., "boards:PROJECT").
  pub async fn fetch_list<T, F, Fut>(&self, key: &str, fetcher: F) -> Result<CacheResult<Vec<T>>>
  where
    T: Cacheable,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
  {
    // Check cache first
    if let Some(cached) = self.storage.get_query_result::<T>(key)? {
      if !self.is_stale(cached.cached_at) {
        // Cache is fresh, return immediately
        return Ok(CacheResult::from_cache(
          cached.entities,
          cached.cached_at,
          false,
        ));
      }

      // Cache is stale, try to fetch from network
      match fetcher().await {
        Ok(data) => {
          // Update cache with fresh data
          self.storage.store_query_result(key, &data)?;
          Ok(CacheResult::from_network(data))
        }
        Err(_) => {
          // Network failed, return stale cache (offline mode)
          Ok(CacheResult::offline(cached.entities, cached.cached_at))
        }
      }
    } else {
      // No cache, must fetch from network
      let data = fetcher().await?;
      self.storage.store_query_result(key, &data)?;
      Ok(CacheResult::from_network(data))
    }
  }

  /// Fetch with incremental update support.
  ///
  /// If we have cached data, only fetch entities updated since max_updated.
  /// Merge new entities into existing cache.
  ///
  /// The `key` parameter is used as the cache lookup key (e.g., "search:jql_query").
  /// The fetcher receives `Option<&str>` containing the max updated_at timestamp from cache.
  pub async fn fetch_incremental<T, F, Fut>(
    &self,
    key: &str,
    fetcher: F,
  ) -> Result<CacheResult<Vec<T>>>
  where
    T: Cacheable,
    F: FnOnce(Option<&str>) -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
  {
    // Get max_updated from cache for incremental fetching
    let max_updated = self.storage.get_max_updated(key)?;

    // Check cache first
    if let Some(cached) = self.storage.get_query_result::<T>(key)? {
      if !self.is_stale(cached.cached_at) {
        // Cache is fresh, return immediately
        return Ok(CacheResult::from_cache(
          cached.entities,
          cached.cached_at,
          false,
        ));
      }

      // Cache is stale, try incremental fetch
      match fetcher(max_updated.as_deref()).await {
        Ok(new_entities) => {
          if new_entities.is_empty() {
            // No new data, but update cached_at timestamp
            self.storage.store_query_result(key, &cached.entities)?;
            return Ok(CacheResult::from_cache(cached.entities, Utc::now(), false));
          }

          // Merge new entities into cache
          self.storage.merge_query_result(key, &new_entities)?;

          // Get the merged result
          if let Some(merged) = self.storage.get_query_result::<T>(key)? {
            return Ok(CacheResult::from_network(merged.entities));
          }

          // Fallback if merge failed
          Ok(CacheResult::from_network(new_entities))
        }
        Err(_) => {
          // Network failed, return stale cache (offline mode)
          Ok(CacheResult::offline(cached.entities, cached.cached_at))
        }
      }
    } else {
      // No cache, must do full fetch (no updated_since filter)
      let data = fetcher(None).await?;
      self.storage.store_query_result(key, &data)?;
      Ok(CacheResult::from_network(data))
    }
  }

  /// Fetch a single entity with caching.
  pub async fn fetch_one<T, F, Fut>(&self, entity_key: &str, fetcher: F) -> Result<CacheResult<T>>
  where
    T: Cacheable,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
  {
    // Check cache first
    if let Some(cached) = self.storage.get_entity::<T>(entity_key)? {
      if !self.is_stale(cached.cached_at) {
        // Cache is fresh
        return Ok(CacheResult::from_cache(
          cached.entity,
          cached.cached_at,
          false,
        ));
      }

      // Cache is stale, try to fetch from network
      match fetcher().await {
        Ok(data) => {
          self.storage.store_entity(&data)?;
          Ok(CacheResult::from_network(data))
        }
        Err(_) => {
          // Network failed, return stale cache (offline mode)
          Ok(CacheResult::offline(cached.entity, cached.cached_at))
        }
      }
    } else {
      // No cache, must fetch from network
      let data = fetcher().await?;
      self.storage.store_entity(&data)?;
      Ok(CacheResult::from_network(data))
    }
  }
}

impl<S: CacheStorage> Clone for CacheLayer<S> {
  fn clone(&self) -> Self {
    Self {
      storage: Arc::clone(&self.storage),
      stale_time: self.stale_time,
    }
  }
}
