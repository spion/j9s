use crate::jira::client::JiraClient;
use crate::jira::types::IssueSummary;
use crate::query::{Query, QueryState};
use crate::ui::components::{KeyResult, SearchEvent, SearchInput};
use crate::ui::ensure_valid_selection;
use crate::ui::renderfns::{status_color, truncate};
use crate::ui::view::{View, ViewAction};
use crate::ui::views::IssueDetailView;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// View for displaying a list of issues
pub struct IssueListView {
  jira: JiraClient,
  project: String,
  query: Query<Vec<IssueSummary>>,
  list_state: ListState,
  search: SearchInput,
}

impl IssueListView {
  pub fn new(project: String, jira: JiraClient) -> Self {
    let jql = if project.is_empty() {
      String::new()
    } else {
      format!(
        "project = {} AND resolution = unresolved ORDER BY updated DESC",
        project
      )
    };

    let mut query = if jql.is_empty() {
      // No project configured - create a query that returns empty results
      Query::new(|| async { Ok(Vec::new()) })
    } else {
      // Create query with the JiraClient
      let jira_for_query = jira.clone();
      Query::new(move || {
        let jira = jira_for_query.clone();
        let jql = jql.clone();
        async move { jira.search_issues(&jql).await.map_err(|e| e.to_string()) }
      })
    };

    // Start fetching immediately
    query.fetch();

    Self {
      jira,
      project,
      query,
      list_state: ListState::default(),
      search: SearchInput::new(),
    }
  }

  fn issues(&self) -> &[IssueSummary] {
    self.query.data().map(|v| v.as_slice()).unwrap_or(&[])
  }

  fn is_loading(&self) -> bool {
    self.query.is_loading()
  }

  fn render_list(&mut self, frame: &mut Frame, area: Rect) {
    let len = self.issues().len();
    ensure_valid_selection(&mut self.list_state, len);

    let title = match self.query.state() {
      QueryState::Loading => format!(" Issues [{}] (loading...) ", self.project),
      QueryState::Error(e) => format!(" Issues [{}] (error: {}) ", self.project, e),
      _ => format!(" Issues [{}] ({}) ", self.project, self.issues().len()),
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if self.issues().is_empty() && !self.is_loading() {
      let content = if self.project.is_empty() {
        "No project configured. Set default_project in config or use -p flag."
      } else if self.query.is_error() {
        "Failed to load issues. Press 'r' to retry."
      } else {
        "No issues found."
      };
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    let items: Vec<ListItem> = self
      .issues()
      .iter()
      .map(|issue| {
        let color = status_color(&issue.status);

        let line = Line::from(vec![
          Span::styled(
            format!("{:<12}", issue.key),
            Style::default().fg(Color::Cyan),
          ),
          Span::raw(" "),
          Span::styled(
            format!("{:<15}", truncate(&issue.status, 15)),
            Style::default().fg(color),
          ),
          Span::raw(" "),
          Span::raw(issue.summary.clone()),
        ]);
        ListItem::new(line)
      })
      .collect();

    let list = List::new(items)
      .block(block)
      .highlight_style(
        Style::default()
          .bg(Color::DarkGray)
          .add_modifier(Modifier::BOLD),
      )
      .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut self.list_state);
  }

  // Key handling helpers for or_else chain pattern
  fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match self.search.handle_key(key) {
      KeyResult::Handled => Some(ViewAction::None),
      KeyResult::Event(SearchEvent::Submitted(_query)) => {
        // TODO: Apply filter
        Some(ViewAction::None)
      }
      KeyResult::Event(SearchEvent::Cancelled) => Some(ViewAction::None),
      KeyResult::NotHandled => None,
    }
  }

  fn handle_navigation(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match key.code {
      KeyCode::Char('j') | KeyCode::Down => {
        self.list_state.select_next();
        Some(ViewAction::None)
      }
      KeyCode::Char('k') | KeyCode::Up => {
        self.list_state.select_previous();
        Some(ViewAction::None)
      }
      _ => None,
    }
  }

  fn handle_actions(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match key.code {
      KeyCode::Char('r') => {
        self.query.refetch();
        Some(ViewAction::None)
      }
      KeyCode::Enter => {
        if let Some(idx) = self.list_state.selected() {
          if let Some(issue) = self.issues().get(idx) {
            return Some(ViewAction::Push(Box::new(IssueDetailView::new(
              issue.key.clone(),
              self.jira.clone(),
            ))));
          }
        }
        None
      }
      KeyCode::Char('q') | KeyCode::Esc => Some(ViewAction::Pop),
      _ => None,
    }
  }
}

impl View for IssueListView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    self
      .handle_overlays(key)
      .or_else(|| self.handle_navigation(key))
      .or_else(|| self.handle_actions(key))
      .unwrap_or(ViewAction::None)
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    self.render_list(frame, area);
    // Let search component render its overlay
    self.search.render_overlay(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    if self.project.is_empty() {
      "Issues".to_string()
    } else {
      format!("Issues [{}]", self.project)
    }
  }

  fn project(&self) -> Option<&str> {
    Some(&self.project)
  }

  fn tick(&mut self) {
    self.query.poll();
  }
}
