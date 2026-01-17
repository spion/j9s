use crate::jira::types::Issue;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw_issue_detail(frame: &mut Frame, area: Rect, issue: &Issue, loading: bool) {
  let title = if loading {
    format!(" {} (loading...) ", issue.key)
  } else {
    format!(" {} ", issue.key)
  };

  let block = Block::default()
    .title(title)
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
