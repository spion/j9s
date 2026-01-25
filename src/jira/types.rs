use crate::cache::Cacheable;

/// Summary of an issue for list views
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IssueSummary {
  pub key: String,
  pub summary: String,
  pub status: String,
  pub status_id: String,
  pub issue_type: String,
  pub assignee: Option<String>,
  pub priority: Option<String>,
  pub epic: Option<String>,
  pub updated: String,
}

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

/// Full issue details
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Issue {
  pub key: String,
  pub summary: String,
  pub description: Option<String>,
  pub status: String,
  pub status_id: String,
  pub issue_type: String,
  pub assignee: Option<String>,
  pub reporter: Option<String>,
  pub priority: Option<String>,
  pub labels: Vec<String>,
  pub created: String,
  pub updated: String,
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

/// Board summary
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
  pub id: u64,
  pub name: String,
  pub board_type: String, // "scrum" or "kanban"
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

/// Epic summary
#[derive(Debug, Clone)]
pub struct Epic {
  pub key: String,
  pub name: String,
  pub summary: String,
  pub status: String,
}

/// Status info with id and human-readable name
#[derive(Debug, Clone, PartialEq)]
pub struct StatusInfo {
  pub id: String,
  pub name: String,
}

/// Board column configuration
#[derive(Debug, Clone)]
pub struct BoardColumn {
  pub name: String,
  pub statuses: Vec<StatusInfo>, // Status info that maps to this column
}

/// Board configuration with columns
#[derive(Debug, Clone)]
pub struct BoardConfiguration {
  pub columns: Vec<BoardColumn>,
}
