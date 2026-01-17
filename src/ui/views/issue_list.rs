use crate::event::JiraEvent;
use crate::jira::types::IssueSummary;
use crate::ui::components::{SearchInput, SearchResult};
use crate::ui::view::{View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// View for displaying a list of issues
#[derive(Debug)]
pub struct IssueListView {
  pub issues: Vec<IssueSummary>,
  pub selected: usize,
  pub project: String,
  pub loading: bool,
  search: SearchInput,
}

impl IssueListView {
  pub fn new(project: String) -> Self {
    Self {
      issues: Vec::new(),
      selected: 0,
      project,
      loading: true,
      search: SearchInput::new(),
    }
  }

  fn render_list(&self, frame: &mut Frame, area: Rect) {
    let title = if self.loading {
      format!(" Issues [{}] (loading...) ", self.project)
    } else {
      format!(" Issues [{}] ({}) ", self.project, self.issues.len())
    };

    let block = Block::default()
      .title(title)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if self.issues.is_empty() && !self.loading {
      let content = if self.project.is_empty() {
        "No project configured. Set default_project in config or use -p flag."
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
      .issues
      .iter()
      .map(|issue| {
        let status_color = match issue.status.as_str() {
          "Done" | "Closed" | "Resolved" => Color::Green,
          "In Progress" | "In Review" => Color::Yellow,
          _ => Color::White,
        };

        let line = Line::from(vec![
          Span::styled(
            format!("{:<12}", issue.key),
            Style::default().fg(Color::Cyan),
          ),
          Span::raw(" "),
          Span::styled(
            format!("{:<12}", truncate(&issue.status, 12)),
            Style::default().fg(status_color),
          ),
          Span::raw(" "),
          Span::raw(truncate(&issue.summary, 60)),
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

    let mut state = ListState::default();
    state.select(Some(self.selected));

    frame.render_stateful_widget(list, area, &mut state);
  }
}

impl View for IssueListView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    // Let search component try to handle first
    match self.search.handle_key(key) {
      SearchResult::Active => return ViewAction::None,
      SearchResult::Submitted(_query) => {
        // TODO: Apply filter
        return ViewAction::None;
      }
      SearchResult::Cancelled => return ViewAction::None,
      SearchResult::NotHandled => {}
    }

    // Normal mode key handling
    match key.code {
      KeyCode::Char('j') | KeyCode::Down => {
        let len = self.issues.len();
        if len > 0 {
          self.selected = (self.selected + 1) % len;
        }
      }
      KeyCode::Char('k') | KeyCode::Up => {
        let len = self.issues.len();
        if len > 0 {
          self.selected = self.selected.checked_sub(1).unwrap_or(len - 1);
        }
      }
      KeyCode::Enter => {
        if let Some(issue) = self.issues.get(self.selected) {
          return ViewAction::LoadIssue {
            key: issue.key.clone(),
          };
        }
      }
      KeyCode::Char('q') | KeyCode::Esc => return ViewAction::Quit,
      _ => {}
    }
    ViewAction::None
  }

  fn render(&self, frame: &mut Frame, area: Rect) {
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

  fn set_loading(&mut self, loading: bool) {
    self.loading = loading;
  }

  fn receive_data(&mut self, event: &JiraEvent) -> bool {
    match event {
      JiraEvent::IssuesLoaded(issues) => {
        self.issues = issues.clone();
        self.loading = false;
        true
      }
      _ => false,
    }
  }
}

fn truncate(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    s.to_string()
  } else {
    format!("{}...", &s[..max_len.saturating_sub(3)])
  }
}
