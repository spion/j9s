use crate::commands::Command;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

/// Draw the command/search overlay with autocomplete
pub fn draw_command_overlay(
  frame: &mut Frame,
  area: Rect,
  prefix: &str, // ":" or "/"
  input: &str,
  suggestions: &[&Command],
  selected_suggestion: usize,
) {
  // Calculate overlay dimensions
  let width = (area.width * 60 / 100).min(60).max(30);
  let suggestion_count = suggestions.len().min(8);
  let height = if suggestions.is_empty() {
    3 // Just input line with borders
  } else {
    3 + suggestion_count as u16 // Input + suggestions
  };

  // Position at top-left of content area with small margin
  let x = area.x + 1;
  let y = area.y + 1;

  let overlay_area = Rect::new(x, y, width, height);

  // Clear the area behind the overlay
  frame.render_widget(Clear, overlay_area);

  // Draw the border/block
  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Yellow))
    .title(if prefix == ":" {
      " Command "
    } else {
      " Search "
    });

  let inner = block.inner(overlay_area);
  frame.render_widget(block, overlay_area);

  if inner.height == 0 {
    return;
  }

  // Split inner area: input line + suggestions
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1), // Input line
      Constraint::Min(0),    // Suggestions
    ])
    .split(inner);

  // Draw input line
  let input_line = Line::from(vec![
    Span::styled(prefix, Style::default().fg(Color::Yellow)),
    Span::raw(input),
    Span::styled("_", Style::default().fg(Color::Yellow)), // Cursor
  ]);
  let input_para = Paragraph::new(input_line);
  frame.render_widget(input_para, chunks[0]);

  // Draw suggestions if any
  if !suggestions.is_empty() && chunks[1].height > 0 {
    let items: Vec<ListItem> = suggestions
      .iter()
      .take(8)
      .map(|cmd| {
        let line = Line::from(vec![
          Span::styled(
            format!("{:<12}", cmd.name),
            Style::default().fg(Color::Cyan),
          ),
          Span::styled(cmd.description, Style::default().fg(Color::DarkGray)),
        ]);
        ListItem::new(line)
      })
      .collect();

    let list =
      List::new(items).highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let mut state = ListState::default();
    state.select(Some(selected_suggestion));

    frame.render_stateful_widget(list, chunks[1], &mut state);
  }
}
