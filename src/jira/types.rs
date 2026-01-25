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

/// Board summary
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
  pub id: u64,
  pub name: String,
  pub board_type: String, // "scrum" or "kanban"
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
