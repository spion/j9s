use crate::jira::client::JiraClient;
use crate::jira::types::Issue;
use crate::query::{Query, QueryState};
use crate::ui::view::{Shortcut, View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// View for displaying issue details
pub struct IssueDetailView {
  key: String,
  query: Query<Issue>,
}

impl IssueDetailView {
  pub fn new(key: String, jira: JiraClient) -> Self {
    let issue_key = key.clone();
    let mut query = Query::new(move || {
      let jira = jira.clone();
      let key = issue_key.clone();
      async move { jira.get_issue(&key).await.map_err(|e| e.to_string()) }
    });

    // Start fetching immediately
    query.fetch();

    Self { key, query }
  }

  fn render_detail(&self, frame: &mut Frame, area: Rect) {
    let title = match self.query.state() {
      QueryState::Loading => format!(" {} (loading...) ", self.key),
      QueryState::Error(e) => format!(" {} (error: {}) ", self.key, e),
      _ => format!(" {} ", self.key),
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Show loading or error state
    if self.query.is_loading() {
      let paragraph =
        Paragraph::new("Loading issue details...").style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, inner);
      return;
    }

    if let Some(error) = self.query.error() {
      let paragraph = Paragraph::new(format!("Error: {}\n\nPress 'r' to retry.", error))
        .style(Style::default().fg(Color::Red));
      frame.render_widget(paragraph, inner);
      return;
    }

    let issue = match self.query.data() {
      Some(issue) => issue,
      None => return,
    };

    // Layout for issue details
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([
        Constraint::Length(3), // Header (summary, status, assignee)
        Constraint::Length(1), // Separator
        Constraint::Min(1),    // Description
      ])
      .split(inner);

    // Header
    let header = vec![
      Line::from(vec![
        Span::styled("Summary: ", Style::default().fg(Color::DarkGray)),
        Span::raw(&issue.summary),
      ]),
      Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&issue.status, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled("Assignee: ", Style::default().fg(Color::DarkGray)),
        Span::raw(issue.assignee.as_deref().unwrap_or("Unassigned")),
      ]),
    ];
    let header_para = Paragraph::new(header);
    frame.render_widget(header_para, chunks[0]);

    // Separator
    let sep = Paragraph::new("─".repeat(chunks[1].width as usize))
      .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, chunks[1]);

    // Description
    let desc = issue.description.as_deref().unwrap_or("No description");
    let desc_para = Paragraph::new(desc)
      .wrap(Wrap { trim: true })
      .style(Style::default());
    frame.render_widget(desc_para, chunks[2]);
  }
}

impl View for IssueDetailView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    match key.code {
      KeyCode::Char('r') => {
        self.query.refetch();
        ViewAction::None
      }
      KeyCode::Char('q') | KeyCode::Esc => ViewAction::Pop,
      _ => ViewAction::None,
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    self.render_detail(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    self.key.clone()
  }

  fn tick(&mut self) {
    self.query.poll();
  }

  fn shortcuts(&self) -> Vec<Shortcut> {
    vec![Shortcut::new("r", "refresh"), Shortcut::new("q", "back")]
  }
}
