use crate::jira::types::{BoardColumn, IssueSummary};
use crate::jira::JiraClient;
use crate::query::{Fetched, Query, QueryState};
use crate::ui::components::{
  IssueFilterField, KeyResult, StatusPicker, StatusPickerEvent, TicketPanel, TicketPanelEvent,
};
use crate::ui::view::{ShortcutInfo, ShortcutProvider, View, ViewAction};
use crate::ui::views::{IssueDetailView, IssueEditorView};
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use std::collections::BTreeSet;
use tracing::info;

/// Combined board data fetched in parallel
#[derive(Clone)]
struct BoardData {
  issues: Vec<IssueSummary>,
  columns: Vec<BoardColumn>,
}

/// View for displaying a single board with its issues
pub struct BoardView {
  #[allow(dead_code)]
  board_id: u64,
  board_name: String,

  jira: JiraClient,

  // Editor context (for create/edit)
  project: String,
  default_labels: Vec<String>,
  assignee_presets: Vec<String>,

  // Config: column names to hide (lowercase for case-insensitive matching)
  hide_columns: BTreeSet<String>,

  // Data + UI
  query: Query<BoardData>,
  panel: TicketPanel<IssueFilterField>,
  columns_set: bool,

  // Status mutation flow (Shift+h/l → optional StatusPicker → API call)
  status_picker: StatusPicker,
  pending_issue_key: Option<String>,
  status_mutation: Option<Query<()>>,
  error_message: Option<String>,
}

impl BoardView {
  pub fn new(
    board_id: u64,
    board_name: String,
    project: String,
    default_labels: Vec<String>,
    assignee_presets: Vec<String>,
    jira: JiraClient,
    hide_columns: BTreeSet<String>,
  ) -> Self {
    let jira_for_query = jira.clone();
    let mut query = Query::new(move || {
      let jira = jira_for_query.clone();
      async move {
        // Filter: unresolved issues + resolved in past 2 weeks
        let jql = "resolution IS EMPTY OR resolved >= -2w";
        let (issues, config_result) = tokio::join!(
          jira.get_board_issues(board_id, Some(jql)),
          jira.get_board_configuration(board_id),
        );

        match config_result {
          Ok(config) => issues.map(|issues| BoardData {
            issues,
            columns: config.columns,
          }),
          Err(e) => Fetched::Error(e.to_string()),
        }
      }
    });

    query.fetch();

    Self {
      board_id,
      board_name,
      jira,
      project,
      default_labels,
      assignee_presets,
      hide_columns,
      query,
      panel: TicketPanel::new(Vec::new()),
      columns_set: false,
      status_picker: StatusPicker::new(),
      pending_issue_key: None,
      status_mutation: None,
      error_message: None,
    }
  }

  fn visible_columns(&self) -> Vec<BoardColumn> {
    self
      .query
      .data()
      .map(|d| {
        d.columns
          .iter()
          .filter(|col| !self.hide_columns.contains(&col.name.to_lowercase()))
          .cloned()
          .collect()
      })
      .unwrap_or_default()
  }

  fn title(&self) -> String {
    match self.query.state() {
      QueryState::Loading => format!("{} (loading...)", self.board_name),
      QueryState::Error(e) => format!("{} (error: {})", self.board_name, e),
      _ => self.board_name.clone(),
    }
  }

  /// Begin a status transition: directly mutate if single-status, otherwise show picker.
  fn begin_transition(
    &mut self,
    issue_key: String,
    target_statuses: Vec<crate::jira::types::StatusInfo>,
  ) {
    self.error_message = None;
    if target_statuses.is_empty() {
      self.error_message = Some("Target column has no statuses".to_string());
      return;
    }
    if target_statuses.len() == 1 {
      info!(
        "Updating issue {} to status {}",
        issue_key, target_statuses[0].name
      );
      self.update_issue_status(&issue_key, &target_statuses[0].id);
    } else {
      self.pending_issue_key = Some(issue_key);
      self
        .status_picker
        .show("Select Status".to_string(), target_statuses);
    }
  }

  fn update_issue_status(&mut self, issue_key: &str, status_id: &str) {
    let jira = self.jira.clone();
    let key = issue_key.to_string();
    let sid = status_id.to_string();
    let mut query = Query::new(move || {
      let jira = jira.clone();
      let key = key.clone();
      let sid = sid.clone();
      async move {
        jira
          .update_issue_status(&key, &sid)
          .await
          .map_err(|e| e.to_string())
      }
    });
    query.fetch();
    self.status_mutation = Some(query);
  }

  fn process_status_mutation(&mut self) {
    let Some(query) = &self.status_mutation else {
      return;
    };
    if query.is_loading() {
      return;
    }
    if let Some(err) = query.error() {
      self.error_message = Some(format!("Status update failed: {}", err));
    } else {
      self.query.refetch();
    }
    self.status_mutation = None;
  }

  fn render_error(&self, frame: &mut Frame, area: Rect) {
    let Some(msg) = &self.error_message else {
      return;
    };

    let max_width = (area.width * 80 / 100).min(70).max(40);
    let inner_width = max_width.saturating_sub(2) as usize;
    let line_count = msg.lines().count();
    let estimated_lines = (msg.len() / inner_width).max(line_count) + 1;
    let height = (estimated_lines as u16 + 2).min(area.height - 4).max(5);

    let x = area.x + (area.width.saturating_sub(max_width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let error_area = Rect::new(x, y, max_width, height);
    frame.render_widget(Clear, error_area);

    let block = Block::bordered()
      .border_style(Color::Red)
      .title(" Error - press any key to dismiss ");

    let paragraph = Paragraph::new(msg.as_str())
      .block(block)
      .fg(Color::Red)
      .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, error_area);
  }
}

impl View for BoardView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    self.error_message = None;

    // Status picker overlay takes priority over panel keys
    match self.status_picker.handle_key(key) {
      KeyResult::Handled => return ViewAction::None,
      KeyResult::Event(StatusPickerEvent::Selected(status_id)) => {
        if let Some(issue_key) = self.pending_issue_key.take() {
          self.update_issue_status(&issue_key, &status_id);
        }
        return ViewAction::None;
      }
      KeyResult::Event(StatusPickerEvent::Cancelled) => {
        self.pending_issue_key = None;
        return ViewAction::None;
      }
      KeyResult::NotHandled => {}
    }

    let items = self
      .query
      .data()
      .map(|d| d.issues.as_slice())
      .unwrap_or(&[]);
    match self.panel.handle_key(key, items) {
      KeyResult::Handled => ViewAction::None,
      KeyResult::Event(TicketPanelEvent::Selected(issue)) => {
        ViewAction::Push(Box::new(IssueDetailView::new(issue.key, self.jira.clone())))
      }
      KeyResult::Event(TicketPanelEvent::RefreshRequested) => {
        self.query.refetch();
        ViewAction::None
      }
      KeyResult::Event(TicketPanelEvent::Back) => ViewAction::Pop,
      KeyResult::Event(TicketPanelEvent::FilterChanged) => ViewAction::None,
      KeyResult::Event(TicketPanelEvent::CreateRequested) => {
        ViewAction::Push(Box::new(IssueEditorView::new_create(
          self.project.clone(),
          None,
          self.default_labels.clone(),
          self.assignee_presets.clone(),
          self.jira.clone(),
        )))
      }
      KeyResult::Event(TicketPanelEvent::EditRequested(issue)) => ViewAction::Push(Box::new(
        IssueEditorView::new_edit(issue, self.assignee_presets.clone(), self.jira.clone()),
      )),
      KeyResult::Event(TicketPanelEvent::StatusTransitionRequested {
        issue,
        target_statuses,
      }) => {
        self.begin_transition(issue.key, target_statuses);
        ViewAction::None
      }
      KeyResult::NotHandled => ViewAction::None,
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    let title = self.title();
    let is_loading = self.query.is_loading();
    let items = self
      .query
      .data()
      .map(|d| d.issues.as_slice())
      .unwrap_or(&[]);

    self.panel.render(frame, area, items, &title, is_loading);
    self.status_picker.render_overlay(frame, area);
    self.render_error(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    self.board_name.clone()
  }

  fn project(&self) -> Option<&str> {
    if self.project.is_empty() {
      None
    } else {
      Some(&self.project)
    }
  }

  fn tick(&mut self) -> ViewAction {
    let was_loading = self.query.is_loading();
    self.query.poll();

    if was_loading && !self.query.is_loading() {
      if let Some(data) = self.query.data() {
        let items = data.issues.clone();
        self.panel.update_filter_values(&items);
        if !self.columns_set {
          self.panel.set_columns(self.visible_columns());
          self.columns_set = true;
        }
      }
    }

    if let Some(ref mut query) = self.status_mutation {
      if query.poll() {
        self.process_status_mutation();
      }
    }

    ViewAction::None
  }

  fn shortcuts(&self) -> Vec<ShortcutInfo> {
    let mut shortcuts = vec![
      ShortcutInfo::new(":", "command").with_priority(10),
      ShortcutInfo::new("/", "search").with_priority(20),
      ShortcutInfo::new("q", "back").with_priority(30),
    ];
    shortcuts.extend(self.panel.shortcuts());
    shortcuts
  }
}
