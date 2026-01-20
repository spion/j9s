use crate::config::Config;
use crate::jira::api_types::{
  reserialize, ApiBoardConfigResponse, ApiBoardIssuesResponse, ApiIssue, ApiIssueFields,
  ApiQuickFiltersResponse, ApiTransitionsResponse,
};
use crate::jira::types::{Board, BoardConfiguration, Issue, IssueSummary, QuickFilter};
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
    use futures::{StreamExt, TryStreamExt};

    let search = self.client.search();
    let options = gouqi::SearchOptions::default();

    let stream = search
      .stream(jql, &options)
      .await
      .map_err(|e| eyre!("Failed to search issues: {}", e))?;

    let issues: Vec<IssueSummary> = stream
      .map(|issue| {
        let fields: ApiIssueFields = reserialize(&issue.fields)?;
        Ok(
          ApiIssue {
            key: issue.key,
            fields,
          }
          .into_summary(),
        )
      })
      .try_collect()
      .await
      .map_err(|e: serde_json::Error| eyre!("Failed to parse issue: {}", e))?;

    Ok(issues)
  }

  /// Get a single issue by key
  pub async fn get_issue(&self, key: &str) -> Result<Issue> {
    let issues = self.client.issues();

    let issue = issues
      .get(key)
      .await
      .map_err(|e| eyre!("Failed to get issue {}: {}", key, e))?;

    let fields: ApiIssueFields =
      reserialize(&issue.fields).map_err(|e| eyre!("Failed to parse issue {}: {}", key, e))?;

    Ok(
      ApiIssue {
        key: issue.key,
        fields,
      }
      .into_full(),
    )
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

      let response: ApiBoardIssuesResponse = self
        .client
        .get("agile", &endpoint)
        .await
        .map_err(|e| eyre!("Failed to get board issues: {}", e))?;

      let issues_count = response.issues.len() as u64;
      let issues: Vec<IssueSummary> = response
        .issues
        .into_iter()
        .map(|issue| issue.into_summary())
        .collect();

      all_issues.extend(issues);

      // Check if we've fetched all issues
      if start_at + issues_count >= response.total {
        break;
      }
      start_at += max_results;
    }

    Ok(all_issues)
  }

  /// Get board configuration (columns)
  pub async fn get_board_configuration(&self, board_id: u64) -> Result<BoardConfiguration> {
    let endpoint = format!("/board/{}/configuration", board_id);

    let response: ApiBoardConfigResponse = self
      .client
      .get("agile", &endpoint)
      .await
      .map_err(|e| eyre!("Failed to get board configuration: {}", e))?;

    Ok(response.into())
  }

  /// Update issue status by finding and executing the appropriate transition
  pub async fn update_issue_status(&self, issue_key: &str, status_id: &str) -> Result<()> {
    // Get available transitions
    let endpoint = format!("/issue/{}/transitions", issue_key);

    let response: ApiTransitionsResponse = self
      .client
      .get("api", &endpoint)
      .await
      .map_err(|e| eyre!("Failed to get transitions: {}", e))?;

    // Find transition that leads to target status
    let transition_id = response
      .transitions
      .iter()
      .find(|t| t.to.id == status_id)
      .map(|t| t.id.clone())
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

      let response: ApiQuickFiltersResponse = self
        .client
        .get("agile", &endpoint)
        .await
        .map_err(|e| eyre!("Failed to get board quick filters: {}", e))?;

      let is_last = response.is_last;
      let filters: Vec<QuickFilter> = response.values.into_iter().map(QuickFilter::from).collect();

      all_filters.extend(filters);

      if is_last {
        break;
      }
      start_at += max_results;
    }

    Ok(all_filters)
  }
}
