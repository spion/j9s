use crate::cache::{CacheLayer, SqliteStorage};
use crate::config::{AuthType, Config};
use crate::jira::api_types::{
  reserialize, ApiBoardConfigResponse, ApiBoardIssuesResponse, ApiIssue, ApiIssueFields,
  ApiTransitionsResponse,
};
use crate::jira::types::{Board, BoardConfiguration, Issue, IssueSummary};
use color_eyre::{eyre::eyre, Result};
use serde_json::Value;
use url::form_urlencoded;

/// Jira API client with transparent caching support.
///
/// This client provides the Jira API and automatically caches results
/// for offline support and improved performance.
#[derive(Clone)]
pub struct JiraClient {
  client: gouqi::r#async::Jira,
  epic_field: Option<String>,
  cache: CacheLayer<SqliteStorage>,
}

fn get_issue_fields(epic_field: Option<&str>) -> Vec<&str> {
  let mut fields = vec![
    "summary",
    "status",
    "issuetype",
    "assignee",
    "priority",
    "updated",
  ];
  if let Some(epic_field) = epic_field {
    fields.push(epic_field);
  }
  fields
}
impl JiraClient {
  /// Resolve auth type based on config and URL
  fn resolve_auth_type(auth_type: AuthType, url: &str) -> AuthType {
    match auth_type {
      AuthType::Auto => {
        if url.contains(".atlassian.net") {
          AuthType::Cloud
        } else {
          AuthType::Onpremise
        }
      }
      other => other,
    }
  }

  fn get_credentials(auth_type: AuthType, username: &str) -> Result<gouqi::Credentials> {
    let token = Config::get_api_token().ok();
    let password = Config::get_password().ok();

    match auth_type {
      AuthType::Cloud => {
        // Cloud uses Basic auth with email + API token (or password)
        let secret = token.or(password).ok_or_else(|| {
          eyre!("Jira Cloud requires J9S_JIRA_TOKEN or J9S_JIRA_PASSWORD to be set")
        })?;
        Ok(gouqi::Credentials::Basic(username.to_string(), secret))
      }
      AuthType::Onpremise => {
        // On-premise prefers Bearer token, falls back to Basic auth with password
        if let Some(token) = token {
          Ok(gouqi::Credentials::Bearer(token))
        } else if let Some(password) = password {
          Ok(gouqi::Credentials::Basic(username.to_string(), password))
        } else {
          Err(eyre!(
            "Jira On-premise requires J9S_JIRA_TOKEN (for PAT/Bearer) or J9S_JIRA_PASSWORD (for Basic auth)"
          ))
        }
      }
      AuthType::Auto => unreachable!("Auth type should be resolved before calling get_credentials"),
    }
  }

  pub fn new(config: &Config, cache: CacheLayer<SqliteStorage>) -> Result<Self> {
    let auth_type = Self::resolve_auth_type(config.jira.auth_type, &config.jira.url);
    let credentials = Self::get_credentials(auth_type, &config.jira.email)?;

    let http_client = reqwest::Client::builder()
      .tcp_nodelay(true)
      .pool_max_idle_per_host(10)
      .build()
      .map_err(|e| eyre!("Failed to create HTTP client: {}", e))?;

    let client = gouqi::r#async::Jira::from_client(&config.jira.url, credentials, http_client)
      .map_err(|e| eyre!("Failed to create Jira client: {}", e))?;

    Ok(Self {
      client,
      epic_field: config.jira.epic_field.clone(),
      cache,
    })
  }

  /// Search for issues using JQL with caching and incremental updates.
  pub async fn search_issues(&self, jql: &str) -> Result<Vec<IssueSummary>> {
    let cache_key = format!("search:{}", jql.trim().to_lowercase());
    let base_jql = jql.to_string();
    let client = self.clone();

    let result = self
      .cache
      .fetch_incremental(&cache_key, move |updated_since| {
        let effective_jql = if let Some(since) = updated_since {
          format!("({}) AND updated > '{}'", base_jql, since)
        } else {
          base_jql.clone()
        };
        let client = client.clone();
        async move { client.search_issues_raw(&effective_jql).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Raw search without caching
  async fn search_issues_raw(&self, jql: &str) -> Result<Vec<IssueSummary>> {
    use futures::{StreamExt, TryStreamExt};

    let search = self.client.search();

    let options = gouqi::SearchOptions::builder()
      .fields(get_issue_fields(self.epic_field.as_deref()))
      .max_results(100)
      .build();

    let stream = search
      .stream(jql, &options)
      .await
      .map_err(|e| eyre!("Failed to search issues: {}", e))?;

    let epic_field = self.epic_field.as_deref();
    let issues: Vec<IssueSummary> = stream
      .map(|issue| {
        let fields: ApiIssueFields = reserialize(&issue.fields)?;
        Ok(
          ApiIssue {
            key: issue.key,
            fields,
          }
          .into_summary_with_epic(epic_field),
        )
      })
      .try_collect()
      .await
      .map_err(|e: serde_json::Error| eyre!("Failed to parse issue: {}", e))?;

    Ok(issues)
  }

  /// Get a single issue by key with caching.
  pub async fn get_issue(&self, key: &str) -> Result<Issue> {
    let key_owned = key.to_string();
    let client = self.clone();

    let result = self
      .cache
      .fetch_one(key, move || {
        let key = key_owned.clone();
        let client = client.clone();
        async move { client.get_issue_raw(&key).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Raw get issue without caching
  async fn get_issue_raw(&self, key: &str) -> Result<Issue> {
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

  /// Get all boards with caching, optionally filtered by project.
  pub async fn get_boards(&self, project: Option<&str>) -> Result<Vec<Board>> {
    let cache_key = format!("boards:{}", project.unwrap_or(""));
    let project_owned = project.map(String::from);
    let client = self.clone();

    let result = self
      .cache
      .fetch_list(&cache_key, move || {
        let project = project_owned.clone();
        let client = client.clone();
        async move { client.get_boards_raw(project.as_deref()).await }
      })
      .await?;

    Ok(result.data)
  }

  /// Raw get boards without caching
  async fn get_boards_raw(&self, project: Option<&str>) -> Result<Vec<Board>> {
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

  /// Get issues for a specific board with caching and incremental updates.
  pub async fn get_board_issues(
    &self,
    board_id: u64,
    jql: Option<&str>,
  ) -> Result<Vec<IssueSummary>> {
    let cache_key = format!(
      "board_issues:{}:{}",
      board_id,
      jql.map(|j| j.trim().to_lowercase()).unwrap_or_default()
    );
    let base_jql = jql.map(String::from);
    let client = self.clone();

    let result = self
      .cache
      .fetch_incremental(&cache_key, move |updated_since| {
        let effective_jql = match (&base_jql, updated_since) {
          (Some(base), Some(since)) => Some(format!("({}) AND updated > '{}'", base, since)),
          (Some(base), None) => Some(base.clone()),
          (None, Some(since)) => Some(format!("updated > '{}'", since)),
          (None, None) => None,
        };
        let client = client.clone();
        async move {
          client
            .get_board_issues_raw(board_id, effective_jql.as_deref())
            .await
        }
      })
      .await?;

    Ok(result.data)
  }

  /// Raw get board issues without caching
  async fn get_board_issues_raw(
    &self,
    board_id: u64,
    jql: Option<&str>,
  ) -> Result<Vec<IssueSummary>> {
    let mut all_issues = Vec::new();
    let mut start_at = 0u64;
    let max_results = 100u64;

    let fields = get_issue_fields(self.epic_field.as_deref()).join(",");

    loop {
      let mut endpoint = format!(
        "/board/{}/issue?startAt={}&maxResults={}&fields={}",
        board_id, start_at, max_results, fields
      );

      if let Some(jql) = jql {
        let encoded: String = form_urlencoded::byte_serialize(jql.as_bytes()).collect();
        endpoint.push_str(&format!("&jql={}", encoded));
      }

      let response: ApiBoardIssuesResponse = self
        .client
        .get("agile", &endpoint)
        .await
        .map_err(|e| eyre!("Failed to get board issues: {}", e))?;

      let epic_field = self.epic_field.as_deref();
      let issues_count = response.issues.len() as u64;
      let issues: Vec<IssueSummary> = response
        .issues
        .into_iter()
        .map(|issue| issue.into_summary_with_epic(epic_field))
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

  /// Get epics for a project
  pub async fn get_epics(&self, project: &str) -> Result<Vec<IssueSummary>> {
    let jql = format!(
      "project = {} AND issuetype = Epic ORDER BY updated DESC",
      project
    );
    self.search_issues(&jql).await
  }

  /// Get issues that belong to an epic
  pub async fn get_epic_issues(&self, epic_key: &str) -> Result<Vec<IssueSummary>> {
    // Use the Epic Link field if configured, otherwise try "Epic Link"
    let epic_field = self.epic_field.as_deref().unwrap_or("Epic Link");
    let jql = format!("\"{}\" = {} ORDER BY updated DESC", epic_field, epic_key);
    self.search_issues(&jql).await
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
}
