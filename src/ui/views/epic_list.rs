use crate::jira::types::IssueSummary;
use crate::jira::JiraClient;
use crate::query::Query;
use crate::ui::components::{IssueFilterField, KeyResult, TicketPanel, TicketPanelEvent};
use crate::ui::view::{ShortcutInfo, ShortcutProvider, View, ViewAction};
use crate::ui::views::{EpicDetailView, IssueEditorView};
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

/// View for displaying a list of epics in a project
pub struct EpicListView {
  jira: JiraClient,
  project: String,
  default_labels: Vec<String>,
  query: Query<Vec<IssueSummary>>,
  panel: TicketPanel<IssueFilterField>,
}

impl EpicListView {
  pub fn new(project: String, jira: JiraClient, default_labels: Vec<String>) -> Self {
    let mut query = if project.is_empty() {
      // No project configured
      Query::new(|| async { Ok(Vec::new()) })
    } else {
      let jira_for_query = jira.clone();
      let project_for_query = project.clone();
      Query::new(move || {
        let jira = jira_for_query.clone();
        let project = project_for_query.clone();
        async move { jira.get_epics(&project).await }
      })
    };

    query.fetch();

    Self {
      jira,
      project,
      default_labels,
      query,
      panel: TicketPanel::list_only(),
    }
  }

  fn title(&self) -> String {
    format!("Epics [{}]", self.project)
  }
}

impl View for EpicListView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    let items = self.query.data().map(|v| v.as_slice()).unwrap_or(&[]);

    match self.panel.handle_key(key, items) {
      KeyResult::Handled => ViewAction::None,
      KeyResult::Event(TicketPanelEvent::Selected(epic)) => ViewAction::Push(Box::new(
        EpicDetailView::new(epic, self.jira.clone(), self.default_labels.clone()),
      )),
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
          self.jira.clone(),
        )))
      }
      KeyResult::Event(TicketPanelEvent::EditRequested(issue)) => ViewAction::Push(Box::new(
        IssueEditorView::new_edit(issue, self.jira.clone()),
      )),
      KeyResult::NotHandled => ViewAction::None,
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    let items = self.query.data().map(|v| v.as_slice()).unwrap_or(&[]);
    let title = self.title();
    let is_loading = self.query.is_loading();

    self.panel.render(frame, area, items, &title, is_loading);
  }

  fn breadcrumb_label(&self) -> String {
    if self.project.is_empty() {
      "Epics".to_string()
    } else {
      format!("Epics [{}]", self.project)
    }
  }

  fn project(&self) -> Option<&str> {
    Some(&self.project)
  }

  fn tick(&mut self) -> ViewAction {
    let was_loading = self.query.is_loading();
    self.query.poll();

    if was_loading && !self.query.is_loading() {
      if let Some(data) = self.query.data() {
        self.panel.update_filter_values(data);
      }
    }
    ViewAction::None
  }

  fn on_resume(&mut self) {
    self.query.refetch();
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
