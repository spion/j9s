use crate::config::Config;
use crate::jira::types::{
  Board, BoardColumn, BoardConfiguration, Issue, IssueSummary, QuickFilter, StatusInfo,
};
use color_eyre::{eyre::eyre, Result};
use serde_json::Value;

/// Jira API client wrapper
#[derive(Clone)]
pub struct JiraClient {
  client: gouqi::r#async::Jira,
}

impl JiraClient {
  pub fn new(config: &Config) -> Result<Self> {
    let token = Config::get_api_token()?;

    let credentials = gouqi::Credentials::Basic(config.jira.email.clone(), token);

    let client = gouqi::r#async::Jira::new(&config.jira.url, credentials)
      .map_err(|e| eyre!("Failed to create Jira client: {}", e))?;

    Ok(Self { client })
  }

  /// Search for issues using JQL
  pub async fn search_issues(&self, jql: &str) -> Result<Vec<IssueSummary>> {
    use futures::StreamExt;

    let search = self.client.search();
    let options = gouqi::SearchOptions::default();

    let stream = search
      .stream(jql, &options)
      .await
      .map_err(|e| eyre!("Failed to search issues: {}", e))?;

    let issues: Vec<IssueSummary> = stream
      .map(|issue| {
        let fields = issue.fields;
        IssueSummary {
          key: issue.key,
          summary: fields
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
          status: fields
            .get("status")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string(),
          status_id: fields
            .get("status")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .expect("Status ID should be present")
            .to_string(),
          issue_type: fields
            .get("issuetype")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
          assignee: fields
            .get("assignee")
            .and_then(|v| v.get("displayName"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
          priority: fields
            .get("priority")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        }
      })
      .collect()
      .await;

    Ok(issues)
  }

  /// Get a single issue by key
  pub async fn get_issue(&self, key: &str) -> Result<Issue> {
    let issues = self.client.issues();

    let issue = issues
      .get(key)
      .await
      .map_err(|e| eyre!("Failed to get issue {}: {}", key, e))?;

    let fields = issue.fields;

    Ok(Issue {
      key: issue.key,
      summary: fields
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string(),
      description: fields.get("description").and_then(extract_description),
      status: fields
        .get("status")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string(),
      status_id: fields
        .get("status")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .expect("Status ID should be present")
        .to_string(),
      issue_type: fields
        .get("issuetype")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string(),
      assignee: fields
        .get("assignee")
        .and_then(|v| v.get("displayName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()),
      reporter: fields
        .get("reporter")
        .and_then(|v| v.get("displayName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()),
      priority: fields
        .get("priority")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()),
      labels: fields
        .get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| {
          arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
        })
        .unwrap_or_default(),
      created: fields
        .get("created")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string(),
      updated: fields
        .get("updated")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string(),
    })
  }

  /// Get all boards, optionally filtered by project
  pub async fn get_boards(&self, project: Option<&str>) -> Result<Vec<Board>> {
    use futures::StreamExt;

    let boards_api = self.client.boards();
    let options = match project {
      Some(p) => gouqi::SearchOptions::builder().project_key_or_id(p).build(),
      None => gouqi::SearchOptions::default(),
    };

    let stream = boards_api
      .stream(&options)
      .await
      .map_err(|e| eyre!("Failed to get boards: {}", e))?;

    let boards: Vec<Board> = stream
      .filter_map(|result| async move { result.ok() })
      .map(|board| Board {
        id: board.id,
        name: board.name,
        board_type: board.type_name,
      })
      .collect()
      .await;

    Ok(boards)
  }

  /// Get issues for a specific board
  pub async fn get_board_issues(&self, board_id: u64) -> Result<Vec<IssueSummary>> {
    let mut all_issues = Vec::new();
    let mut start_at = 0u64;
    let max_results = 50u64;

    loop {
      let endpoint = format!(
        "/board/{}/issue?startAt={}&maxResults={}",
        board_id, start_at, max_results
      );
      let response: Value = self
        .client
        .get("agile", &endpoint)
        .await
        .map_err(|e| eyre!("Failed to get board issues: {}", e))?;

      let issues: Vec<IssueSummary> = response
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|arr| {
          arr
            .iter()
            .filter_map(|issue| {
              let key = issue.get("key")?.as_str()?.to_string();
              let fields = issue.get("fields")?;

              Some(IssueSummary {
                key,
                summary: fields
                  .get("summary")
                  .and_then(|v| v.as_str())
                  .unwrap_or("")
                  .to_string(),
                status: fields
                  .get("status")
                  .and_then(|v| v.get("name"))
                  .and_then(|v| v.as_str())
                  .unwrap_or("Unknown")
                  .to_string(),
                status_id: fields
                  .get("status")
                  .and_then(|v| v.get("id"))
                  .and_then(|v| v.as_str())
                  .expect("Status ID should be present")
                  .to_string(),
                issue_type: fields
                  .get("issuetype")
                  .and_then(|v| v.get("name"))
                  .and_then(|v| v.as_str())
                  .unwrap_or("")
                  .to_string(),
                assignee: fields
                  .get("assignee")
                  .and_then(|v| v.get("displayName"))
                  .and_then(|v| v.as_str())
                  .map(|s| s.to_string()),
                priority: fields
                  .get("priority")
                  .and_then(|v| v.get("name"))
                  .and_then(|v| v.as_str())
                  .map(|s| s.to_string()),
              })
            })
            .collect()
        })
        .unwrap_or_default();

      all_issues.extend(issues);

      let is_last = response
        .get("isLast")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
      if is_last {
        break;
      }
      start_at += max_results;
    }

    Ok(all_issues)
  }

  /// Get board configuration (columns)
  pub async fn get_board_configuration(&self, board_id: u64) -> Result<BoardConfiguration> {
    let endpoint = format!("/board/{}/configuration", board_id);
    let response: Value = self
      .client
      .get("agile", &endpoint)
      .await
      .map_err(|e| eyre!("Failed to get board configuration: {}", e))?;

    let columns = response
      .get("columnConfig")
      .and_then(|v| v.get("columns"))
      .and_then(|v| v.as_array())
      .map(|arr| {
        arr
          .iter()
          .filter_map(|col| {
            let name = col.get("name")?.as_str()?.to_string();
            let statuses = col
              .get("statuses")
              .and_then(|v| v.as_array())
              .map(|statuses| {
                statuses
                  .iter()
                  .filter_map(|s| {
                    let id = s.get("id").and_then(|v| v.as_str())?.to_string();
                    // Get status name, fall back to id if not present
                    let name = s
                      .get("name")
                      .and_then(|v| v.as_str())
                      .map(|n| n.to_string())
                      .unwrap_or_else(|| id.clone());
                    Some(StatusInfo { id, name })
                  })
                  .collect()
              })
              .unwrap_or_default();

            Some(BoardColumn { name, statuses })
          })
          .collect()
      })
      .unwrap_or_default();

    Ok(BoardConfiguration { columns })
  }

  /// Update issue status by finding and executing the appropriate transition
  pub async fn update_issue_status(&self, issue_key: &str, status_id: &str) -> Result<()> {
    // Get available transitions
    let endpoint = format!("/issue/{}/transitions", issue_key);
    let response: Value = self
      .client
      .get("api", &endpoint)
      .await
      .map_err(|e| eyre!("Failed to get transitions: {}", e))?;

    // Find transition that leads to target status
    let transition_id = response
      .get("transitions")
      .and_then(|v| v.as_array())
      .and_then(|arr| {
        arr.iter().find_map(|t| {
          let to_status_id = t.get("to")?.get("id")?.as_str()?;
          if to_status_id == status_id {
            t.get("id")?.as_str().map(|s| s.to_string())
          } else {
            None
          }
        })
      })
      .ok_or_else(|| eyre!("No transition available to status {}", status_id))?;

    // Execute the transition
    let body = serde_json::json!({
      "transition": {
        "id": transition_id
      }
    });

    self
      .client
      .post::<Value, _>("api", &endpoint, body)
      .await
      .map_err(|e| eyre!("Failed to execute transition: {}", e))?;

    Ok(())
  }

  /// Get quick filters for a board
  pub async fn get_board_quick_filters(&self, board_id: u64) -> Result<Vec<QuickFilter>> {
    let mut all_filters = Vec::new();
    let mut start_at = 0u64;
    let max_results = 50u64;

    loop {
      let endpoint = format!(
        "/board/{}/quickfilter?startAt={}&maxResults={}",
        board_id, start_at, max_results
      );
      let response: Value = self
        .client
        .get("agile", &endpoint)
        .await
        .map_err(|e| eyre!("Failed to get board quick filters: {}", e))?;

      let filters: Vec<QuickFilter> = response
        .get("values")
        .and_then(|v| v.as_array())
        .map(|arr| {
          arr
            .iter()
            .filter_map(|f| {
              let id = f.get("id")?.as_u64()?;
              let name = f.get("name")?.as_str()?.to_string();
              let jql = f
                .get("jql")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

              Some(QuickFilter { id, name, jql })
            })
            .collect()
        })
        .unwrap_or_default();

      all_filters.extend(filters);

      let is_last = response
        .get("isLast")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
      if is_last {
        break;
      }
      start_at += max_results;
    }

    Ok(all_filters)
  }
}

/// Extract plain text description from Jira's ADF or plain text format
fn extract_description(value: &Value) -> Option<String> {
  // If it's a string, return it directly (API v2)
  if let Some(s) = value.as_str() {
    return Some(s.to_string());
  }

  // If it's an ADF document (API v3), extract text content
  if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
    let mut text = String::new();
    extract_adf_text(content, &mut text);
    if !text.is_empty() {
      return Some(text);
    }
  }

  None
}

/// Recursively extract text from ADF content
fn extract_adf_text(content: &[Value], output: &mut String) {
  for node in content {
    if let Some(node_type) = node.get("type").and_then(|v| v.as_str()) {
      match node_type {
        "text" => {
          if let Some(text) = node.get("text").and_then(|v| v.as_str()) {
            output.push_str(text);
          }
        }
        "paragraph" | "heading" | "bulletList" | "orderedList" | "listItem" => {
          if let Some(children) = node.get("content").and_then(|v| v.as_array()) {
            extract_adf_text(children, output);
          }
          if node_type == "paragraph" || node_type == "heading" {
            output.push('\n');
          }
        }
        "hardBreak" => {
          output.push('\n');
        }
        _ => {
          // Try to extract from children anyway
          if let Some(children) = node.get("content").and_then(|v| v.as_array()) {
            extract_adf_text(children, output);
          }
        }
      }
    }
  }
}
