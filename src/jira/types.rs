/// Summary of an issue for list views
#[derive(Debug, Clone)]
pub struct IssueSummary {
  pub key: String,
  pub summary: String,
  pub status: String,
  pub status_id: String,
  pub issue_type: String,
  pub assignee: Option<String>,
  pub priority: Option<String>,
}

/// Full issue details
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// Board column configuration
#[derive(Debug, Clone)]
pub struct BoardColumn {
  pub name: String,
  pub statuses: Vec<String>, // Status ids that map to this column
}

/// Board configuration with columns
#[derive(Debug, Clone)]
pub struct BoardConfiguration {
  pub columns: Vec<BoardColumn>,
}

/// Quick filter for a board
#[derive(Debug, Clone)]
pub struct QuickFilter {
  pub id: u64,
  pub name: String,
  pub jql: String,
}
