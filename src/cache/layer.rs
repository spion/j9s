//! Cache layer that orchestrates caching logic with network fetching.

use color_eyre::Result;
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, warn};

use super::storage::CacheStorage;
use super::traits::Cacheable;
use crate::query::Fetched;

/// Cache layer that manages caching logic and network fetching.
///
/// Always attempts a network fetch. On failure, falls back to cached data
/// (returning `Stale`) or propagates the error (returning `Error`).
pub struct CacheLayer<S: CacheStorage> {
  storage: Arc<S>,
}

impl<S: CacheStorage> CacheLayer<S> {
  pub fn new(storage: S) -> Self {
    Self {
      storage: Arc::new(storage),
    }
  }

  /// Fetch a list, falling back to cache on error.
  pub async fn fetch_list<T, F, Fut>(&self, key: &str, fetcher: F) -> Fetched<Vec<T>>
  where
    T: Cacheable,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
  {
    debug!(
      key,
      entity_type = T::entity_type(),
      "fetch_list: calling fetcher"
    );
    match fetcher().await {
      Ok(data) => {
        debug!(key, count = data.len(), "fetch_list: fetcher returned OK");
        if let Err(e) = self.storage.store_query_result(key, &data) {
          warn!("Cache store failed: {}", e);
        }
        Fetched::Fresh(data)
      }
      Err(e) => {
        debug!(key, error = %e, "fetch_list: fetcher failed, trying cache fallback");
        self.fallback_list(key, e.to_string())
      }
    }
  }

  /// Fetch with incremental update support, falling back to cache on error.
  pub async fn fetch_incremental<T, F, Fut>(&self, key: &str, fetcher: F) -> Fetched<Vec<T>>
  where
    T: Cacheable,
    F: FnOnce(Option<&str>) -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
  {
    let max_updated = self.storage.get_max_updated(key).ok().flatten();
    debug!(
      key,
      entity_type = T::entity_type(),
      max_updated = max_updated.as_deref().unwrap_or("(none)"),
      "fetch_incremental: starting"
    );

    match fetcher(max_updated.as_deref()).await {
      Ok(new_entities) => {
        debug!(
          key,
          new_count = new_entities.len(),
          incremental = max_updated.is_some(),
          "fetch_incremental: fetcher returned OK"
        );
        if max_updated.is_some() {
          // Incremental: merge new entities into cache
          if !new_entities.is_empty() {
            if let Err(e) = self.storage.merge_query_result(key, &new_entities) {
              warn!("Cache merge failed: {}", e);
            }
          }
          // Return full cached set (includes merged data)
          match self.storage.get_query_result::<T>(key) {
            Ok(Some(cached)) => {
              debug!(
                key,
                cached_count = cached.entities.len(),
                "fetch_incremental: returning cached set"
              );
              Fetched::Fresh(cached.entities)
            }
            Ok(None) => {
              debug!(
                key,
                "fetch_incremental: no cache found, returning fetcher result"
              );
              Fetched::Fresh(new_entities)
            }
            Err(e) => {
              warn!(key, error = %e, "fetch_incremental: cache read failed, returning fetcher result");
              Fetched::Fresh(new_entities)
            }
          }
        } else {
          // Full fetch: store and return
          if let Err(e) = self.storage.store_query_result(key, &new_entities) {
            warn!("Cache store failed: {}", e);
          }
          Fetched::Fresh(new_entities)
        }
      }
      Err(e) => {
        debug!(key, error = %e, "fetch_incremental: fetcher failed, trying cache fallback");
        self.fallback_list(key, e.to_string())
      }
    }
  }

  /// Fetch a single entity, falling back to cache on error.
  pub async fn fetch_one<T, F, Fut>(&self, entity_key: &str, fetcher: F) -> Fetched<T>
  where
    T: Cacheable,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
  {
    debug!(
      entity_key,
      entity_type = T::entity_type(),
      "fetch_one: calling fetcher"
    );
    match fetcher().await {
      Ok(data) => {
        debug!(entity_key, "fetch_one: fetcher returned OK");
        if let Err(e) = self.storage.store_entity(&data) {
          warn!("Cache store failed: {}", e);
        }
        Fetched::Fresh(data)
      }
      Err(e) => {
        debug!(entity_key, error = %e, "fetch_one: fetcher failed, trying cache fallback");
        match self.storage.get_entity::<T>(entity_key) {
          Ok(Some(cached)) => Fetched::Stale(cached.entity, e.to_string()),
          _ => Fetched::Error(e.to_string()),
        }
      }
    }
  }

  fn fallback_list<T: Cacheable>(&self, key: &str, error: String) -> Fetched<Vec<T>> {
    match self.storage.get_query_result::<T>(key) {
      Ok(Some(cached)) => {
        debug!(
          key,
          count = cached.entities.len(),
          "fallback_list: returning stale cache"
        );
        Fetched::Stale(cached.entities, error)
      }
      Ok(None) => {
        debug!(key, "fallback_list: no cache available");
        Fetched::Error(error)
      }
      Err(e) => {
        warn!(key, cache_error = %e, "fallback_list: cache read failed");
        Fetched::Error(error)
      }
    }
  }
}

impl<S: CacheStorage> Clone for CacheLayer<S> {
  fn clone(&self) -> Self {
    Self {
      storage: Arc::clone(&self.storage),
    }
  }
}
