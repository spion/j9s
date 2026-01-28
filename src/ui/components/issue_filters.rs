use super::filter_source::FilterSource;
use crate::jira::types::IssueSummary;
use std::collections::BTreeSet;

/// Field to filter issues by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueFilterField {
  #[default]
  None,
  Assignee,
  Epic,
  Status,
  Priority,
}

impl IssueFilterField {
  /// Extract the value of this filter field from an issue
  fn extract(&self, issue: &IssueSummary) -> Option<String> {
    match self {
      IssueFilterField::None => None,
      IssueFilterField::Assignee => issue.assignee.clone(),
      IssueFilterField::Epic => issue.epic.clone(),
      IssueFilterField::Status => Some(issue.status.clone()),
      IssueFilterField::Priority => issue.priority.clone(),
    }
  }
}

impl FilterSource<IssueSummary> for IssueFilterField {
  fn label(&self) -> &'static str {
    match self {
      IssueFilterField::None => "None",
      IssueFilterField::Assignee => "Assignee",
      IssueFilterField::Epic => "Epic",
      IssueFilterField::Status => "Status",
      IssueFilterField::Priority => "Priority",
    }
  }

  fn unique_values(&self, items: &[IssueSummary]) -> Vec<Option<String>> {
    if !self.is_active() {
      return Vec::new();
    }

    let mut values: BTreeSet<Option<String>> = BTreeSet::new();
    for item in items {
      values.insert(self.extract(item));
    }

    // Convert to Vec, with None (unassigned) first if present
    let mut result: Vec<Option<String>> = Vec::new();
    if values.contains(&None) {
      result.push(None);
    }
    for v in values.into_iter().flatten() {
      result.push(Some(v));
    }
    result
  }

  fn filter<'a>(
    &self,
    items: &'a [IssueSummary],
    value: Option<&Option<String>>,
  ) -> Vec<&'a IssueSummary> {
    match value {
      None => items.iter().collect(), // "All" - no filtering
      Some(filter_value) => items
        .iter()
        .filter(|item| {
          let item_value = self.extract(item);
          item_value == *filter_value
        })
        .collect(),
    }
  }

  fn is_active(&self) -> bool {
    !matches!(self, IssueFilterField::None)
  }

  fn all_variants() -> &'static [Self] {
    &[
      IssueFilterField::None,
      IssueFilterField::Assignee,
      IssueFilterField::Epic,
      IssueFilterField::Status,
      IssueFilterField::Priority,
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_issues() -> Vec<IssueSummary> {
    vec![
      IssueSummary {
        key: "TEST-1".to_string(),
        summary: "First issue".to_string(),
        status: "To Do".to_string(),
        status_id: "1".to_string(),
        issue_type: "Bug".to_string(),
        assignee: Some("Alice".to_string()),
        priority: Some("High".to_string()),
        epic: Some("Epic-1".to_string()),
        updated: "2024-01-01".to_string(),
      },
      IssueSummary {
        key: "TEST-2".to_string(),
        summary: "Second issue".to_string(),
        status: "In Progress".to_string(),
        status_id: "2".to_string(),
        issue_type: "Task".to_string(),
        assignee: Some("Bob".to_string()),
        priority: Some("Low".to_string()),
        epic: None,
        updated: "2024-01-02".to_string(),
      },
      IssueSummary {
        key: "TEST-3".to_string(),
        summary: "Third issue".to_string(),
        status: "To Do".to_string(),
        status_id: "1".to_string(),
        issue_type: "Task".to_string(),
        assignee: None,
        priority: None,
        epic: Some("Epic-1".to_string()),
        updated: "2024-01-03".to_string(),
      },
    ]
  }

  #[test]
  fn test_unique_values_assignee() {
    let issues = test_issues();
    let filter = IssueFilterField::Assignee;
    let values = filter.unique_values(&issues);

    // Should have: None (unassigned), Alice, Bob - with None first
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], None);
    assert!(values.contains(&Some("Alice".to_string())));
    assert!(values.contains(&Some("Bob".to_string())));
  }

  #[test]
  fn test_unique_values_status() {
    let issues = test_issues();
    let filter = IssueFilterField::Status;
    let values = filter.unique_values(&issues);

    // Should have: In Progress, To Do (alphabetically sorted)
    assert_eq!(values.len(), 2);
    assert!(values.contains(&Some("To Do".to_string())));
    assert!(values.contains(&Some("In Progress".to_string())));
  }

  #[test]
  fn test_filter_by_assignee() {
    let issues = test_issues();
    let filter = IssueFilterField::Assignee;

    // Filter to Alice
    let alice = Some("Alice".to_string());
    let filtered = filter.filter(&issues, Some(&alice));
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].key, "TEST-1");

    // Filter to unassigned
    let filtered = filter.filter(&issues, Some(&None));
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].key, "TEST-3");
  }

  #[test]
  fn test_filter_all() {
    let issues = test_issues();
    let filter = IssueFilterField::Assignee;

    // "All" returns everything
    let filtered = filter.filter(&issues, None);
    assert_eq!(filtered.len(), 3);
  }

  #[test]
  fn test_none_filter_is_inactive() {
    let filter = IssueFilterField::None;
    assert!(!filter.is_active());

    let issues = test_issues();
    assert!(filter.unique_values(&issues).is_empty());
  }
}
