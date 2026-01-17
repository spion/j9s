use crate::event::JiraEvent;
use crate::jira::types::Issue;
use crate::ui::view::{View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// View for displaying issue details
#[derive(Debug)]
pub struct IssueDetailView {
  pub issue: Box<Issue>,
  pub loading: bool,
}

impl IssueDetailView {
  pub fn new(issue: Issue) -> Self {
    Self {
      issue: Box::new(issue),
      loading: false,
    }
  }

  fn render_detail(&self, frame: &mut Frame, area: Rect) {
    let title = if self.loading {
      format!(" {} (loading...) ", self.issue.key)
    } else {
      format!(" {} ", self.issue.key)
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    frame.render_widget(block, area);

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
        Span::raw(&self.issue.summary),
      ]),
      Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&self.issue.status, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled("Assignee: ", Style::default().fg(Color::DarkGray)),
        Span::raw(self.issue.assignee.as_deref().unwrap_or("Unassigned")),
      ]),
    ];
    let header_para = Paragraph::new(header);
    frame.render_widget(header_para, chunks[0]);

    // Separator
    let sep = Paragraph::new("─".repeat(chunks[1].width as usize))
      .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, chunks[1]);

    // Description
    let desc = self
      .issue
      .description
      .as_deref()
      .unwrap_or("No description");
    let desc_para = Paragraph::new(desc)
      .wrap(Wrap { trim: true })
      .style(Style::default());
    frame.render_widget(desc_para, chunks[2]);
  }
}

impl View for IssueDetailView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    match key.code {
      KeyCode::Char('q') | KeyCode::Esc => ViewAction::Pop,
      _ => ViewAction::None,
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    self.render_detail(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    self.issue.key.clone()
  }

  fn set_loading(&mut self, loading: bool) {
    self.loading = loading;
  }

  fn receive_data(&mut self, _event: &JiraEvent) -> bool {
    // Issue detail doesn't receive data updates (yet)
    false
  }
}
