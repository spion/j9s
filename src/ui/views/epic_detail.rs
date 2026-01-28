use crate::jira::types::{BoardColumn, IssueSummary, StatusInfo};
use crate::jira::JiraClient;
use crate::query::Query;
use crate::ui::components::{IssueFilterField, KeyResult, TicketPanel, TicketPanelEvent};
use crate::ui::view::{ShortcutInfo, ShortcutProvider, View, ViewAction};
use crate::ui::views::IssueDetailView;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::collections::BTreeMap;

/// View for displaying epic details and its child issues
pub struct EpicDetailView {
  jira: JiraClient,
  epic: IssueSummary,
  query: Query<Vec<IssueSummary>>,
  panel: TicketPanel<IssueFilterField>,
}

impl EpicDetailView {
  pub fn new(epic: IssueSummary, jira: JiraClient) -> Self {
    let epic_key = epic.key.clone();
    let jira_for_query = jira.clone();

    let mut query = Query::new(move || {
      let jira = jira_for_query.clone();
      let epic_key = epic_key.clone();
      async move {
        jira
          .get_epic_issues(&epic_key)
          .await
          .map_err(|e| e.to_string())
      }
    });

    query.fetch();

    Self {
      jira,
      epic,
      query,
      panel: TicketPanel::new(Vec::new()), // Will set columns when data loads
    }
  }

  /// Derive columns from unique statuses in the issues
  fn derive_columns(issues: &[IssueSummary]) -> Vec<BoardColumn> {
    // Collect unique statuses preserving order of first occurrence
    let mut seen: BTreeMap<String, StatusInfo> = BTreeMap::new();
    for issue in issues {
      seen
        .entry(issue.status_id.clone())
        .or_insert_with(|| StatusInfo {
          id: issue.status_id.clone(),
          name: issue.status.clone(),
        });
    }

    // Create a column for each unique status
    seen
      .into_values()
      .map(|status| BoardColumn {
        name: status.name.clone(),
        statuses: vec![status],
      })
      .collect()
  }
}

impl View for EpicDetailView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    let items = self.query.data().map(|v| v.as_slice()).unwrap_or(&[]);

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
      KeyResult::NotHandled => ViewAction::None,
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    // Split area: epic info (2 lines) + child issues
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Length(2), Constraint::Min(0)])
      .split(area);

    // Epic info header (borderless, styled text)
    let line1 = Line::from(vec![
      Span::styled(
        format!(" {} ", self.epic.key),
        Style::default().fg(Color::Cyan).bold(),
      ),
      Span::styled(&self.epic.summary, Style::default().fg(Color::White)),
    ]);

    let line2 = Line::from(vec![
      Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
      Span::styled(&self.epic.status, Style::default().fg(Color::Yellow)),
      Span::styled("  Type: ", Style::default().fg(Color::DarkGray)),
      Span::styled(&self.epic.issue_type, Style::default().fg(Color::Magenta)),
    ]);

    let text = Text::from(vec![line1, line2]);
    let para = Paragraph::new(text);
    frame.render_widget(para, chunks[0]);

    // Child issues
    let items = self.query.data().map(|v| v.as_slice()).unwrap_or(&[]);
    let is_loading = self.query.is_loading();
    self
      .panel
      .render(frame, chunks[1], items, "Child Issues", is_loading);
  }

  fn breadcrumb_label(&self) -> String {
    self.epic.key.clone()
  }

  fn tick(&mut self) {
    let was_loading = self.query.is_loading();
    self.query.poll();

    if was_loading && !self.query.is_loading() {
      if let Some(data) = self.query.data() {
        // Derive columns from child issues' statuses
        let columns = Self::derive_columns(data);
        self.panel.set_columns(columns);
        self.panel.update_filter_values(data);
      }
    }
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
