use crate::event::JiraEvent;
use crate::jira::types::Board;
use crate::ui::components::{SearchInput, SearchResult};
use crate::ui::view::{View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// View for displaying a list of boards
#[derive(Debug)]
pub struct BoardListView {
  pub boards: Vec<Board>,
  pub selected: usize,
  pub loading: bool,
  search: SearchInput,
}

impl BoardListView {
  pub fn new() -> Self {
    Self {
      boards: Vec::new(),
      selected: 0,
      loading: true,
      search: SearchInput::new(),
    }
  }

  fn render_list(&self, frame: &mut Frame, area: Rect) {
    let title = if self.loading {
      " Boards (loading...) ".to_string()
    } else {
      format!(" Boards ({}) ", self.boards.len())
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if self.boards.is_empty() && !self.loading {
      let paragraph = Paragraph::new("No boards found.")
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    let items: Vec<ListItem> = self
      .boards
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
    state.select(Some(self.selected));

    frame.render_stateful_widget(list, area, &mut state);
  }
}

impl Default for BoardListView {
  fn default() -> Self {
    Self::new()
  }
}

impl View for BoardListView {
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
        let len = self.boards.len();
        if len > 0 {
          self.selected = (self.selected + 1) % len;
        }
      }
      KeyCode::Char('k') | KeyCode::Up => {
        let len = self.boards.len();
        if len > 0 {
          self.selected = self.selected.checked_sub(1).unwrap_or(len - 1);
        }
      }
      KeyCode::Enter => {
        if let Some(board) = self.boards.get(self.selected) {
          return ViewAction::LoadBoard {
            id: board.id,
            name: board.name.clone(),
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
    "Boards".to_string()
  }

  fn set_loading(&mut self, loading: bool) {
    self.loading = loading;
  }

  fn receive_data(&mut self, event: &JiraEvent) -> bool {
    match event {
      JiraEvent::BoardsLoaded(boards) => {
        self.boards = boards.clone();
        self.loading = false;
        true
      }
      _ => false,
    }
  }
}
