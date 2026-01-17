use crate::jira::types::IssueSummary;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw_issue_list(
  frame: &mut Frame,
  area: Rect,
  issues: &[IssueSummary],
  selected: usize,
  project: &str,
  loading: bool,
) {
  let title = if loading {
    format!(" Issues [{}] (loading...) ", project)
  } else {
    format!(" Issues [{}] ({}) ", project, issues.len())
  };

  let block = Block::default()
    .title(title)
    .borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Blue));

  if issues.is_empty() && !loading {
    let content = if project.is_empty() {
      "No project configured. Set default_project in config or use -p flag."
    } else {
      "No issues found."
    };
    let paragraph = ratatui::widgets::Paragraph::new(content)
      .block(block)
      .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
    return;
  }

  let items: Vec<ListItem> = issues
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
  state.select(Some(selected));

  frame.render_stateful_widget(list, area, &mut state);
}

fn truncate(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    s.to_string()
  } else {
    format!("{}...", &s[..max_len.saturating_sub(3)])
  }
}
