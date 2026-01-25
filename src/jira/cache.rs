//! Caching implementations for Jira types.

use sha2::{Digest, Sha256};

use crate::cache::{Cacheable, QueryKey};

use super::types::{Board, Issue, IssueSummary};

// ============================================================================
// Cacheable implementations
// ============================================================================

impl Cacheable for IssueSummary {
  fn cache_key(&self) -> String {
    self.key.clone()
  }

  fn updated_at(&self) -> Option<&str> {
    Some(&self.updated)
  }

  fn entity_type() -> &'static str {
    "issue_summary"
  }
}

impl Cacheable for Issue {
  fn cache_key(&self) -> String {
    self.key.clone()
  }

  fn updated_at(&self) -> Option<&str> {
    Some(&self.updated)
  }

  fn entity_type() -> &'static str {
    "issue"
  }
}

impl Cacheable for Board {
  fn cache_key(&self) -> String {
    self.id.to_string()
  }

  fn updated_at(&self) -> Option<&str> {
    // Boards don't have an updated_at field
    None
  }

  fn entity_type() -> &'static str {
    "board"
  }
}

// ============================================================================
// Query key types
// ============================================================================

/// Query key types for Jira API calls.
#[derive(Clone, Debug)]
pub enum JiraQueryKey {
  /// Search issues with JQL
  IssueSearch { jql: String },
  /// Get issues for a specific board
  BoardIssues { board_id: u64, jql: Option<String> },
  /// List boards, optionally filtered by project
  Boards { project: Option<String> },
  /// Get a single issue by key
  IssueDetail { key: String },
}

impl QueryKey for JiraQueryKey {
  fn cache_hash(&self) -> String {
    let input = match self {
      Self::IssueSearch { jql } => format!("issue_search:{}", normalize_jql(jql)),
      Self::BoardIssues { board_id, jql } => {
        format!(
          "board_issues:{}:{}",
          board_id,
          jql.as_ref().map(|j| normalize_jql(j)).unwrap_or_default()
        )
      }
      Self::Boards { project } => {
        format!("boards:{}", project.as_deref().unwrap_or(""))
      }
      Self::IssueDetail { key } => format!("issue_detail:{}", key),
    };

    // SHA256 hash for stable, fixed-length keys
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
  }

  fn description(&self) -> String {
    match self {
      Self::IssueSearch { jql } => format!("issues: {}", jql),
      Self::BoardIssues { board_id, jql } => {
        if let Some(j) = jql {
          format!("board {} issues: {}", board_id, j)
        } else {
          format!("board {} issues", board_id)
        }
      }
      Self::Boards { project } => {
        if let Some(p) = project {
          format!("boards for project {}", p)
        } else {
          "all boards".to_string()
        }
      }
      Self::IssueDetail { key } => format!("issue {}", key),
    }
  }
}

/// Normalize JQL for consistent hashing.
/// Trims whitespace and lowercases for case-insensitive matching.
fn normalize_jql(jql: &str) -> String {
  jql.trim().to_lowercase()
}
