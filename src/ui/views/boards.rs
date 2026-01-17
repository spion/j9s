use crate::jira::types::Board;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw_board_list(
  frame: &mut Frame,
  area: Rect,
  boards: &[Board],
  selected: usize,
  loading: bool,
) {
  let title = if loading {
    " Boards (loading...) ".to_string()
  } else {
    format!(" Boards ({}) ", boards.len())
  };

  let block = Block::default()
    .title(title)
    .borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Blue));

  if boards.is_empty() && !loading {
    let paragraph = ratatui::widgets::Paragraph::new("No boards found.")
      .block(block)
      .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
    return;
  }

  let items: Vec<ListItem> = boards
    .iter()
    .map(|board| {
      let line = Line::from(vec![
        Span::styled(format!("{:<8}", board.id), Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(
          format!("{:<10}", board.board_type),
          Style::default().fg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::raw(&board.name),
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
