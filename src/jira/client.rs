use crate::cache::{CacheLayer, SqliteStorage};
use crate::config::{AuthType, Config};
use crate::jira::api_types::{
  reserialize, ApiBoardConfigResponse, ApiBoardIssuesResponse, ApiIssue, ApiIssueFields,
  ApiProjectIssueType, ApiTransitionsResponse,
};
use crate::jira::types::{Board, BoardConfiguration, Issue, IssueSummary, IssueTypeInfo};
use crate::query::Fetched;
use color_eyre::{
  eyre::{eyre, WrapErr},
  Result,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};
use url::form_urlencoded;

/// Strip ORDER BY clause from JQL for use in incremental update queries.
fn strip_order_by(jql: &str) -> &str {
  let lower = jql.to_ascii_lowercase();
  match lower.find("order by") {
    Some(pos) => jql[..pos].trim_end(),
    None => jql,
  }
}

/// Convert a Jira ISO timestamp to JQL date format.
/// "2026-01-21T22:44:02.902+0000" → "2026-01-21 22:44"
fn to_jql_date(iso: &str) -> String {
  iso.get(..16).unwrap_or(iso).replace('T', " ")
}

/// Compact, UTF-8-safe preview of a response body for inclusion in error
/// messages. Whitespace is collapsed so HTML pages stay on one line.
fn body_snippet(body: &str) -> String {
  const MAX: usize = 200;
  let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
  if collapsed.is_empty() {
    return "<empty>".to_string();
  }
  if collapsed.chars().count() <= MAX {
    collapsed
  } else {
    let truncated: String = collapsed.chars().take(MAX).collect();
    format!("{truncated}…")
  }
}

/// Jira API client with transparent caching support.
///
/// This client provides the Jira API and automatically caches results
/// for offline support and improved performance.
#[derive(Clone)]
pub struct JiraClient {
  client: gouqi::r#async::Jira,
  http: reqwest::Client,
  base_url: String,
  credentials: gouqi::Credentials,
  epic_field: Option<String>,
  cache: CacheLayer<SqliteStorage>,
  auth_type: AuthType,
  assignee_presets: Arc<Mutex<Vec<String>>>,
  /// None until first Cloud resolution attempt; Some(map) afterwards.
  assignee_id_cache: Arc<Mutex<Option<HashMap<String, String>>>>,
}

fn get_issue_fields(epic_field: Option<&str>) -> Vec<&str> {
  let mut fields = vec![
    "summary",
    "status",
    "issuetype",
    "assignee",
    "priority",
    "updated",
    "created",
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

    let client =
      gouqi::r#async::Jira::from_client(&config.jira.url, credentials.clone(), http_client.clone())
        .map_err(|e| eyre!("Failed to create Jira client: {}", e))?;

    Ok(Self {
      client,
      http: http_client,
      base_url: config.jira.url.clone(),
      credentials,
      epic_field: config.jira.epic_field.clone(),
      cache,
      auth_type,
      assignee_presets: Arc::new(Mutex::new(Vec::new())),
      assignee_id_cache: Arc::new(Mutex::new(None)),
    })
  }

  /// Provide the preset list of assignee display names. On Cloud these are
  /// resolved to accountIds lazily on first `resolve_assignee` call.
  pub async fn set_assignee_presets(&self, names: Vec<String>) {
    *self.assignee_presets.lock().await = names;
  }

  /// Resolve a display name to the value that should be sent in the Jira API
  /// "assignee" field. Cloud: looks up accountId via /user/search (cached for
  /// the session). Server/on-prem: returns the name as-is. Returns None when
  /// Cloud lookup finds no match.
  pub async fn resolve_assignee(&self, name: &str) -> Option<String> {
    if name.is_empty() {
      return None;
    }
    match self.auth_type {
      AuthType::Cloud => {
        let mut cache_guard = self.assignee_id_cache.lock().await;
        if cache_guard.is_none() {
          let presets = self.assignee_presets.lock().await.clone();
          let mut map = HashMap::new();
          for preset in &presets {
            match self.search_user_account_id(preset).await {
              Ok(Some(id)) => {
                map.insert(preset.clone(), id);
              }
              Ok(None) => {
                warn!(name = %preset, "no Cloud user matched assignee preset");
              }
              Err(e) => {
                warn!(name = %preset, error = %e, "failed to resolve assignee preset");
              }
            }
          }
          *cache_guard = Some(map);
        }
        cache_guard.as_ref().and_then(|m| m.get(name).cloned())
      }
      _ => Some(name.to_string()),
    }
  }

  /// Authenticated GET that mirrors gouqi's URL scheme but reads the response
  /// body as text first, so parse / non-2xx errors include a snippet of the
  /// body. Use this when a `serde_json` "expected value" error would otherwise
  /// hide whether the server returned HTML, an empty body, etc.
  async fn json_get<T: DeserializeOwned>(&self, api: &str, endpoint: &str) -> Result<T> {
    let url = format!(
      "{}/rest/{}/latest{}",
      self.base_url.trim_end_matches('/'),
      api,
      endpoint
    );
    let req = self.http.get(&url).header("Accept", "application/json");
    let req = match &self.credentials {
      gouqi::Credentials::Bearer(t) => req.bearer_auth(t),
      gouqi::Credentials::Basic(u, p) => req.basic_auth(u, Some(p)),
      gouqi::Credentials::Cookie(c) => req.header(reqwest::header::COOKIE, c),
      gouqi::Credentials::Anonymous => req,
    };
    let resp = req
      .send()
      .await
      .wrap_err_with(|| format!("HTTP request to {url} failed"))?;
    let status = resp.status();
    let body = resp
      .text()
      .await
      .wrap_err_with(|| format!("reading response body from {url} (HTTP {status})"))?;
    if !status.is_success() {
      return Err(eyre!(
        "HTTP {} from {}: {}",
        status,
        url,
        body_snippet(&body)
      ));
    }
    serde_json::from_str::<T>(&body).wrap_err_with(|| {
      format!(
        "parsing JSON from {} (HTTP {}) — body: {}",
        url,
        status,
        body_snippet(&body)
      )
    })
  }

  /// Cloud-only: query /user/search and return the first match's accountId.
  async fn search_user_account_id(&self, name: &str) -> Result<Option<String>> {
    let encoded: String = form_urlencoded::byte_serialize(name.as_bytes()).collect();
    let endpoint = format!("/user/search?query={}", encoded);
    let response: Value = self
      .client
      .get("api", &endpoint)
      .await
      .map_err(|e| eyre!("user search for {}: {}", name, e))?;

    Ok(
      response
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|user| user.get("accountId"))
        .and_then(|v| v.as_str())
        .map(String::from),
    )
  }

  /// Search for issues using JQL with caching and incremental updates.
  pub async fn search_issues(&self, jql: &str) -> Fetched<Vec<IssueSummary>> {
    let cache_key = format!("search:{}", jql.trim().to_lowercase());
    let full_jql = jql.to_string();
    let base_jql = strip_order_by(jql).to_string();
    let client = self.clone();

    self
      .cache
      .fetch_incremental(&cache_key, move |updated_since| {
        let effective_jql = if let Some(since) = updated_since {
          format!("({}) AND updated >= '{}'", base_jql, to_jql_date(since))
        } else {
          full_jql.clone()
        };
        let client = client.clone();
        async move { client.search_issues_raw(&effective_jql).await }
      })
      .await
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

    debug!(jql, count = issues.len(), "search_issues_raw: completed");
    Ok(issues)
  }

  /// Get a single issue by key with caching.
  pub async fn get_issue(&self, key: &str) -> Fetched<Issue> {
    let key_owned = key.to_string();
    let client = self.clone();

    self
      .cache
      .fetch_one(key, move || {
        let key = key_owned.clone();
        let client = client.clone();
        async move { client.get_issue_raw(&key).await }
      })
      .await
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
  pub async fn get_boards(&self, project: Option<&str>) -> Fetched<Vec<Board>> {
    let cache_key = format!("boards:{}", project.unwrap_or(""));
    let project_owned = project.map(String::from);
    let client = self.clone();

    self
      .cache
      .fetch_list(&cache_key, move || {
        let project = project_owned.clone();
        let client = client.clone();
        async move { client.get_boards_raw(project.as_deref()).await }
      })
      .await
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
  ) -> Fetched<Vec<IssueSummary>> {
    let cache_key = format!(
      "board_issues:{}:{}",
      board_id,
      jql.map(|j| j.trim().to_lowercase()).unwrap_or_default()
    );
    let full_jql = jql.map(String::from);
    let base_jql = jql.map(|j| strip_order_by(j).to_string());
    let client = self.clone();

    self
      .cache
      .fetch_incremental(&cache_key, move |updated_since| {
        let effective_jql = match (&base_jql, updated_since) {
          (Some(base), Some(since)) => Some(format!(
            "({}) AND updated >= '{}'",
            base,
            to_jql_date(since)
          )),
          (_, None) => full_jql.clone(),
          (None, Some(since)) => Some(format!("updated >= '{}'", to_jql_date(since))),
        };
        let client = client.clone();
        async move {
          client
            .get_board_issues_raw(board_id, effective_jql.as_deref())
            .await
        }
      })
      .await
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
  pub async fn get_epics(&self, project: &str) -> Fetched<Vec<IssueSummary>> {
    let jql = format!(
      "project = {} AND issuetype = Epic ORDER BY created DESC",
      project
    );
    self.search_issues(&jql).await
  }

  /// Get issues that belong to an epic
  pub async fn get_epic_issues(&self, epic_key: &str) -> Fetched<Vec<IssueSummary>> {
    // Use the Epic Link field if configured, otherwise try "Epic Link"
    let epic_field = self.epic_field.as_deref().unwrap_or("Epic Link");
    let jql = format!("\"{}\" = {} ORDER BY updated DESC", epic_field, epic_key);
    self.search_issues(&jql).await
  }

  /// Get issue types and their valid statuses for a project.
  /// Works on both Cloud and Server via REST API v2.
  pub async fn get_project_statuses(&self, project: &str) -> Result<Vec<IssueTypeInfo>> {
    let endpoint = format!("/project/{}/statuses", project);
    let response: Vec<ApiProjectIssueType> = self
      .json_get("api", &endpoint)
      .await
      .map_err(|e| eyre!("Failed to get project statuses: {}", e))?;
    Ok(response.into_iter().map(|t| t.into_domain()).collect())
  }

  /// Create a new issue. Returns the created issue key.
  /// Uses plain text description (API v2) for Cloud + Server compatibility.
  ///
  /// `assignee` is a display name from the user's config preset list. An empty
  /// string or None leaves the issue unassigned (Jira default).
  pub async fn create_issue(
    &self,
    project: &str,
    summary: &str,
    issue_type: &str,
    description: Option<&str>,
    labels: &[String],
    epic: Option<&str>,
    assignee: Option<&str>,
  ) -> Result<String> {
    let mut fields = serde_json::json!({
      "project": { "key": project },
      "summary": summary,
      "issuetype": { "name": issue_type },
      "labels": labels,
    });

    if let Some(desc) = description {
      fields["description"] = serde_json::Value::String(desc.to_string());
    }

    if let (Some(epic_key), Some(epic_field)) = (epic, self.epic_field.as_deref()) {
      fields[epic_field] = serde_json::Value::String(epic_key.to_string());
    }

    if let Some(name) = assignee.filter(|s| !s.is_empty()) {
      if let Some(value) = self.resolve_assignee(name).await {
        let key = match self.auth_type {
          AuthType::Cloud => "accountId",
          _ => "name",
        };
        fields["assignee"] = serde_json::json!({ key: value });
      }
    }

    let body = serde_json::json!({ "fields": fields });
    let response: serde_json::Value = self
      .client
      .post("api", "/issue", body)
      .await
      .map_err(|e| eyre!("Failed to create issue: {}", e))?;

    response["key"]
      .as_str()
      .map(String::from)
      .ok_or_else(|| eyre!("Create issue response missing key"))
  }

  /// Update an existing issue's fields.
  ///
  /// `assignee` semantics:
  ///   - `None`              → field omitted from PATCH (don't touch).
  ///   - `Some("")`          → send `null` (explicit unassign).
  ///   - `Some(display_name)` → resolve + send.
  pub async fn update_issue(
    &self,
    key: &str,
    summary: &str,
    description: Option<&str>,
    issue_type: &str,
    labels: &[String],
    epic: Option<&str>,
    assignee: Option<&str>,
  ) -> Result<()> {
    let mut fields = serde_json::json!({
      "summary": summary,
      "issuetype": { "name": issue_type },
      "labels": labels,
    });

    fields["description"] = match description {
      Some(desc) => serde_json::Value::String(desc.to_string()),
      None => serde_json::Value::Null,
    };

    if let Some(epic_field) = self.epic_field.as_deref() {
      fields[epic_field] = match epic {
        Some(epic_key) => serde_json::Value::String(epic_key.to_string()),
        None => serde_json::Value::Null,
      };
    }

    match assignee {
      None => {}
      Some("") => fields["assignee"] = serde_json::Value::Null,
      Some(name) => {
        if let Some(value) = self.resolve_assignee(name).await {
          let field_key = match self.auth_type {
            AuthType::Cloud => "accountId",
            _ => "name",
          };
          fields["assignee"] = serde_json::json!({ field_key: value });
        }
      }
    }

    let body = serde_json::json!({ "fields": fields });
    let endpoint = format!("/issue/{}", key);
    self
      .client
      .put::<Value, _>("api", &endpoint, body)
      .await
      .map_err(|e| eyre!("Failed to update issue {}: {}", key, e))?;
    Ok(())
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
