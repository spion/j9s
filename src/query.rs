//! Async query abstraction for data fetching with caching support.
//!
//! Inspired by TanStack Query, this module provides a `Query<T>` type that
//! encapsulates async data fetching, loading states, and error handling.
//!
//! # Example
//!
//! ```ignore
//! let jira = jira_client.clone();
//! let mut query = Query::new(move || {
//!     let jira = jira.clone();
//!     async move { jira.search_issues("project = PROJ").await }
//! });
//!
//! // Start fetching
//! query.fetch();
//!
//! // In event loop tick
//! if query.poll() {
//!     // State changed, trigger re-render
//! }
//!
//! // In render
//! match query.state() {
//!     QueryState::Loading => render_spinner(),
//!     QueryState::Success(data) => render_data(data),
//!     QueryState::Error(e) => render_error(e),
//!     QueryState::Idle => {}
//! }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// The state of a query
#[derive(Debug, Clone)]
pub enum QueryState<T> {
  /// Query has not been started
  Idle,
  /// Query is currently fetching data
  Loading,
  /// Query completed successfully
  Success(T),
  /// Query failed with an error
  Error(String),
}

impl<T> QueryState<T> {
  pub fn is_loading(&self) -> bool {
    matches!(self, QueryState::Loading)
  }

  pub fn is_success(&self) -> bool {
    matches!(self, QueryState::Success(_))
  }

  pub fn is_error(&self) -> bool {
    matches!(self, QueryState::Error(_))
  }

  pub fn data(&self) -> Option<&T> {
    match self {
      QueryState::Success(data) => Some(data),
      _ => None,
    }
  }

  pub fn error(&self) -> Option<&str> {
    match self {
      QueryState::Error(e) => Some(e),
      _ => None,
    }
  }
}

/// A boxed future that returns a Result<T, String>
type BoxFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

/// A factory function that creates futures for fetching data
type FetcherFn<T> = Box<dyn Fn() -> BoxFuture<T> + Send + Sync>;

/// Async query for data fetching with state management.
///
/// Query<T> encapsulates:
/// - The fetching logic (via a closure)
/// - Loading/success/error states
/// - Async result handling via channels
/// - Optional stale time tracking for cache invalidation
pub struct Query<T> {
  state: QueryState<T>,
  fetcher: FetcherFn<T>,
  receiver: Option<mpsc::UnboundedReceiver<Result<T, String>>>,
  fetched_at: Option<Instant>,
  stale_time: Duration,
}

impl<T: Send + 'static> Query<T> {
  /// Create a new query with the given fetcher function.
  ///
  /// The fetcher is a closure that returns a future. It will be called
  /// each time `fetch()` or `refetch()` is invoked.
  ///
  /// # Example
  ///
  /// ```ignore
  /// let jira = jira_client.clone();
  /// let query = Query::new(move || {
  ///     let jira = jira.clone();
  ///     async move {
  ///         jira.search_issues("project = PROJ")
  ///             .await
  ///             .map_err(|e| e.to_string())
  ///     }
  /// });
  /// ```
  pub fn new<F, Fut>(fetcher: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
  {
    Self {
      state: QueryState::Idle,
      fetcher: Box::new(move || Box::pin(fetcher())),
      receiver: None,
      fetched_at: None,
      stale_time: Duration::from_secs(60), // Default 1 minute
    }
  }

  /// Set the stale time for this query.
  ///
  /// After this duration, the data is considered stale and `is_stale()` returns true.
  pub fn with_stale_time(mut self, duration: Duration) -> Self {
    self.stale_time = duration;
    self
  }

  /// Get the current state of the query.
  pub fn state(&self) -> &QueryState<T> {
    &self.state
  }

  /// Get the data if the query succeeded.
  pub fn data(&self) -> Option<&T> {
    self.state.data()
  }

  /// Check if the query is currently loading.
  pub fn is_loading(&self) -> bool {
    self.state.is_loading()
  }

  /// Check if the query succeeded.
  pub fn is_success(&self) -> bool {
    self.state.is_success()
  }

  /// Check if the query failed.
  pub fn is_error(&self) -> bool {
    self.state.is_error()
  }

  /// Get the error message if the query failed.
  pub fn error(&self) -> Option<&str> {
    self.state.error()
  }

  /// Check if the data is stale (older than stale_time).
  pub fn is_stale(&self) -> bool {
    match &self.state {
      QueryState::Success(_) => self
        .fetched_at
        .map(|t| t.elapsed() > self.stale_time)
        .unwrap_or(true),
      _ => false,
    }
  }

  /// Start fetching data if not already loading.
  ///
  /// This is a no-op if the query is already loading.
  pub fn fetch(&mut self) {
    if self.state.is_loading() {
      return;
    }
    self.start_fetch();
  }

  /// Force a refetch, even if already loading or data exists.
  pub fn refetch(&mut self) {
    // Cancel any pending fetch by dropping the receiver
    self.receiver = None;
    self.start_fetch();
  }

  /// Poll for results from a pending fetch.
  ///
  /// Returns `true` if the state changed (data arrived or error occurred).
  /// Call this in your event loop tick handler.
  pub fn poll(&mut self) -> bool {
    let receiver = match &mut self.receiver {
      Some(rx) => rx,
      None => return false,
    };

    // Try to receive without blocking
    match receiver.try_recv() {
      Ok(Ok(data)) => {
        self.state = QueryState::Success(data);
        self.fetched_at = Some(Instant::now());
        self.receiver = None;
        true
      }
      Ok(Err(error)) => {
        self.state = QueryState::Error(error);
        self.receiver = None;
        true
      }
      Err(mpsc::error::TryRecvError::Empty) => false,
      Err(mpsc::error::TryRecvError::Disconnected) => {
        // Sender dropped without sending - treat as error
        self.state = QueryState::Error("Query was cancelled".to_string());
        self.receiver = None;
        true
      }
    }
  }

  /// Internal: start the fetch operation
  fn start_fetch(&mut self) {
    let (tx, rx) = mpsc::unbounded_channel();
    self.receiver = Some(rx);
    self.state = QueryState::Loading;

    let future = (self.fetcher)();
    tokio::spawn(async move {
      let result = future.await;
      // Ignore send errors - receiver may have been dropped
      let _ = tx.send(result);
    });
  }
}

// Query is not Clone because the fetcher is boxed and receiver is owned.
// If you need to share a query, wrap it in Arc<Mutex<Query<T>>>.

impl<T: std::fmt::Debug> std::fmt::Debug for Query<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Query")
      .field("state", &self.state)
      .field("fetched_at", &self.fetched_at)
      .field("stale_time", &self.stale_time)
      .finish_non_exhaustive()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_query_success() {
    let mut query = Query::new(|| async { Ok::<_, String>(vec![1, 2, 3]) });

    assert!(matches!(query.state(), QueryState::Idle));

    query.fetch();
    assert!(query.is_loading());

    // Wait for the result
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(query.poll());
    assert!(query.is_success());
    assert_eq!(query.data(), Some(&vec![1, 2, 3]));
  }

  #[tokio::test]
  async fn test_query_error() {
    let mut query: Query<i32> = Query::new(|| async { Err("Something went wrong".to_string()) });

    query.fetch();
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(query.poll());
    assert!(query.is_error());
    assert_eq!(query.error(), Some("Something went wrong"));
  }

  #[tokio::test]
  async fn test_query_stale() {
    let mut query = Query::new(|| async { Ok::<_, String>(42) }).with_stale_time(Duration::ZERO);

    query.fetch();
    tokio::time::sleep(Duration::from_millis(10)).await;
    query.poll();

    // With zero stale time, should immediately be stale
    assert!(query.is_stale());
  }

  #[tokio::test]
  async fn test_fetch_while_loading_is_noop() {
    let mut query = Query::new(|| async {
      tokio::time::sleep(Duration::from_millis(100)).await;
      Ok::<_, String>(42)
    });

    query.fetch();
    assert!(query.is_loading());

    // Second fetch should be no-op
    query.fetch();
    assert!(query.is_loading());
  }

  #[tokio::test]
  async fn test_refetch_cancels_pending() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = counter.clone();

    let mut query = Query::new(move || {
      let counter = counter_clone.clone();
      async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok::<_, String>(counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
      }
    });

    query.fetch();
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Refetch should cancel the first and start a new one
    query.refetch();
    tokio::time::sleep(Duration::from_millis(100)).await;

    query.poll();
    // Only the second fetch should have completed and been received
    assert_eq!(query.data(), Some(&1));
  }
}
