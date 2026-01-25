//! Cached Jira client that wraps JiraClient with transparent caching.

use color_eyre::Result;

use crate::cache::{CacheLayer, SqliteStorage};
use crate::config::Config;

use super::cache::JiraQueryKey;
use super::client::JiraClient;
use super::types::{Board, BoardConfiguration, Issue, IssueSummary};

/// Jira client with transparent caching support.
///
/// This wraps the underlying JiraClient and provides the same API,
/// but automatically caches results and supports offline mode.
#[derive(Clone)]
pub struct CachedJiraClient {
  inner: JiraClient,
  cache: CacheLayer<SqliteStorage>,
}

impl CachedJiraClient {
  /// Create a new cached Jira client.
  pub fn new(config: &Config) -> Result<Self> {
    let inner = JiraClient::new(config)?;
    let storage = SqliteStorage::open()?;
    let cache = CacheLayer::new(storage);

    Ok(Self { inner, cache })
  }

  /// Search for issues using JQL with caching and incremental updates.
  pub async fn search_issues(&self, jql: &str) -> Result<Vec<IssueSummary>> {
    let query_key = JiraQueryKey::IssueSearch {
      jql: jql.to_string(),
    };

    let result = self
      .cache
      .fetch_incremental(&query_key, |updated_since| {
        let inner = self.inner.clone();
        let jql = if let Some(since) = updated_since {
          // Add updated filter for incremental fetch
          format!("({}) AND updated > '{}'", jql, since)
        } else {
          jql.to_string()
        };
        async move { inner.search_issues(&jql).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Get a single issue by key with caching.
  pub async fn get_issue(&self, key: &str) -> Result<Issue> {
    let result = self
      .cache
      .fetch_one(key, || {
        let inner = self.inner.clone();
        let key = key.to_string();
        async move { inner.get_issue(&key).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Get all boards with caching.
  pub async fn get_boards(&self, project: Option<&str>) -> Result<Vec<Board>> {
    let query_key = JiraQueryKey::Boards {
      project: project.map(String::from),
    };

    let result = self
      .cache
      .fetch(&query_key, || {
        let inner = self.inner.clone();
        let project = project.map(String::from);
        async move { inner.get_boards(project.as_deref()).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Get issues for a specific board with caching and incremental updates.
  pub async fn get_board_issues(
    &self,
    board_id: u64,
    jql: Option<&str>,
  ) -> Result<Vec<IssueSummary>> {
    let query_key = JiraQueryKey::BoardIssues {
      board_id,
      jql: jql.map(String::from),
    };

    let result = self
      .cache
      .fetch_incremental(&query_key, |updated_since| {
        let inner = self.inner.clone();
        let base_jql = jql.map(String::from);
        let updated_since = updated_since.map(String::from);

        async move {
          let effective_jql = match (base_jql, updated_since) {
            (Some(base), Some(since)) => Some(format!("({}) AND updated > '{}'", base, since)),
            (Some(base), None) => Some(base),
            (None, Some(since)) => Some(format!("updated > '{}'", since)),
            (None, None) => None,
          };
          inner
            .get_board_issues(board_id, effective_jql.as_deref())
            .await
        }
      })
      .await?;

    Ok(result.data)
  }

  /// Get board configuration (not cached - changes rarely and is small).
  pub async fn get_board_configuration(&self, board_id: u64) -> Result<BoardConfiguration> {
    self.inner.get_board_configuration(board_id).await
  }

  /// Update issue status (not cached - write operation).
  pub async fn update_issue_status(&self, issue_key: &str, status_id: &str) -> Result<()> {
    self.inner.update_issue_status(issue_key, status_id).await
  }
}
