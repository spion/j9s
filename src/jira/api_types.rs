//! Serde-deserializable types matching Jira API responses.
//!
//! These types are separate from domain types to allow clean deserialization
//! while keeping domain types focused on application needs.

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Re-serialize a value through JSON to convert between compatible types.
/// Useful for converting gouqi's BTreeMap fields to our typed structs.
pub fn reserialize<T: DeserializeOwned>(value: impl Serialize) -> serde_json::Result<T> {
  serde_json::from_value(serde_json::to_value(value)?)
}

// ============================================================================
// Common nested field types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ApiStatus {
  pub id: String,
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiIssueType {
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiUser {
  #[serde(rename = "displayName")]
  pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiPriority {
  pub name: String,
}

// ============================================================================
// Issue fields - used by both search and board issues endpoints
// ============================================================================

#[derive(Debug, Deserialize, Default)]
pub struct ApiIssueFields {
  #[serde(default)]
  pub summary: String,
  pub status: Option<ApiStatus>,
  #[serde(rename = "issuetype")]
  pub issue_type: Option<ApiIssueType>,
  pub assignee: Option<ApiUser>,
  pub reporter: Option<ApiUser>,
  pub priority: Option<ApiPriority>,
  #[serde(default)]
  pub labels: Vec<String>,
  #[serde(default)]
  pub created: String,
  #[serde(default)]
  pub updated: String,
  // Description is complex (can be string or ADF), handled separately
  pub description: Option<serde_json::Value>,
  // Catch-all for custom fields (like epic)
  #[serde(flatten)]
  pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ApiIssue {
  pub key: String,
  #[serde(default)]
  pub fields: ApiIssueFields,
}

// ============================================================================
// Board issues endpoint response
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ApiBoardIssuesResponse {
  #[serde(default)]
  pub issues: Vec<ApiIssue>,
  #[serde(rename = "startAt", default)]
  pub start_at: u64,
  #[serde(rename = "maxResults", default)]
  pub max_results: u64,
  #[serde(default)]
  pub total: u64,
}

// ============================================================================
// Board configuration endpoint response
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ApiStatusRef {
  pub id: String,
  #[serde(default)]
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiColumn {
  pub name: String,
  #[serde(default)]
  pub statuses: Vec<ApiStatusRef>,
}

#[derive(Debug, Deserialize)]
pub struct ApiColumnConfig {
  #[serde(default)]
  pub columns: Vec<ApiColumn>,
}

#[derive(Debug, Deserialize)]
pub struct ApiBoardConfigResponse {
  #[serde(rename = "columnConfig")]
  pub column_config: Option<ApiColumnConfig>,
}

// ============================================================================
// Transitions endpoint response
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ApiTransitionTo {
  pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiTransition {
  pub id: String,
  pub to: ApiTransitionTo,
}

#[derive(Debug, Deserialize)]
pub struct ApiTransitionsResponse {
  #[serde(default)]
  pub transitions: Vec<ApiTransition>,
}

// ============================================================================
// Conversions to domain types
// ============================================================================

use super::types::{BoardColumn, BoardConfiguration, Issue, IssueSummary, StatusInfo};

impl ApiIssue {
  pub fn into_summary(self) -> IssueSummary {
    self.into_summary_with_epic(None)
  }

  pub fn into_summary_with_epic(self, epic_field: Option<&str>) -> IssueSummary {
    let f = self.fields;
    let epic = epic_field.and_then(|field_name| extract_epic_value(f.extra.get(field_name)));
    IssueSummary {
      key: self.key,
      summary: f.summary,
      status: f
        .status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default(),
      status_id: f.status.map(|s| s.id).unwrap_or_default(),
      issue_type: f.issue_type.map(|t| t.name).unwrap_or_default(),
      assignee: f.assignee.map(|u| u.display_name),
      priority: f.priority.map(|p| p.name),
      epic,
    }
  }

  pub fn into_full(self) -> Issue {
    let f = self.fields;
    Issue {
      key: self.key,
      summary: f.summary,
      description: f.description.as_ref().and_then(extract_description),
      status: f
        .status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default(),
      status_id: f.status.map(|s| s.id).unwrap_or_default(),
      issue_type: f.issue_type.map(|t| t.name).unwrap_or_default(),
      assignee: f.assignee.map(|u| u.display_name),
      reporter: f.reporter.map(|u| u.display_name),
      priority: f.priority.map(|p| p.name),
      labels: f.labels,
      created: f.created,
      updated: f.updated,
    }
  }
}

impl From<ApiColumn> for BoardColumn {
  fn from(col: ApiColumn) -> Self {
    BoardColumn {
      name: col.name,
      statuses: col
        .statuses
        .into_iter()
        .map(|s| StatusInfo {
          name: if s.name.is_empty() {
            s.id.clone()
          } else {
            s.name
          },
          id: s.id,
        })
        .collect(),
    }
  }
}

impl From<ApiBoardConfigResponse> for BoardConfiguration {
  fn from(resp: ApiBoardConfigResponse) -> Self {
    BoardConfiguration {
      columns: resp
        .column_config
        .map(|cc| cc.columns.into_iter().map(BoardColumn::from).collect())
        .unwrap_or_default(),
    }
  }
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract epic value from a custom field
/// Epic fields can be:
/// - A string (epic key like "PROJ-123")
/// - An object with "key" or "name" field
/// - null
fn extract_epic_value(value: Option<&serde_json::Value>) -> Option<String> {
  let value = value?;

  // If it's a string, return it directly
  if let Some(s) = value.as_str() {
    return Some(s.to_string());
  }

  // If it's an object, try to get key or name
  if let Some(obj) = value.as_object() {
    // Try "key" first (standard epic link format)
    if let Some(key) = obj.get("key").and_then(|v| v.as_str()) {
      return Some(key.to_string());
    }
    // Try "name" as fallback
    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
      return Some(name.to_string());
    }
  }

  None
}

/// Extract plain text description from Jira's ADF or plain text format
fn extract_description(value: &serde_json::Value) -> Option<String> {
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
fn extract_adf_text(content: &[serde_json::Value], output: &mut String) {
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
