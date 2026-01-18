use crate::jira::client::JiraClient;
use crate::jira::types::Board;
use crate::query::{Query, QueryState};
use crate::ui::components::{SearchInput, SearchResult};
use crate::ui::ensure_valid_selection;
use crate::ui::view::{View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// View for displaying a list of boards
pub struct BoardListView {
  query: Query<Vec<Board>>,
  list_state: ListState,
  search: SearchInput,
}

impl BoardListView {
  pub fn new(jira: JiraClient) -> Self {
    let mut query = Query::new(move || {
      let jira = jira.clone();
      async move { jira.get_boards().await.map_err(|e| e.to_string()) }
    });

    // Start fetching immediately
    query.fetch();

    Self {
      query,
      list_state: ListState::default(),
      search: SearchInput::new(),
    }
  }

  fn boards(&self) -> &[Board] {
    self.query.data().map(|v| v.as_slice()).unwrap_or(&[])
  }

  fn is_loading(&self) -> bool {
    self.query.is_loading()
  }

  fn render_list(&mut self, frame: &mut Frame, area: Rect) {
    let len = self.boards().len();
    ensure_valid_selection(&mut self.list_state, len);

    let title = match self.query.state() {
      QueryState::Loading => " Boards (loading...) ".to_string(),
      QueryState::Error(e) => format!(" Boards (error: {}) ", e),
      _ => format!(" Boards ({}) ", self.boards().len()),
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if self.boards().is_empty() && !self.is_loading() {
      let content = if self.query.is_error() {
        "Failed to load boards. Press 'r' to retry."
      } else {
        "No boards found."
      };
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    // Collect items first to avoid borrow conflicts with list_state
    let items: Vec<ListItem> = self
      .boards()
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
          Span::raw(board.name.clone()),
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
        self.list_state.select_next();
      }
      KeyCode::Char('k') | KeyCode::Up => {
        self.list_state.select_previous();
      }
      KeyCode::Char('r') => {
        // Refresh
        self.query.refetch();
      }
      KeyCode::Enter => {
        if let Some(idx) = self.list_state.selected() {
          if let Some(board) = self.boards().get(idx) {
            return ViewAction::PushBoard {
              id: board.id,
              name: board.name.clone(),
            };
          }
        }
      }
      KeyCode::Char('q') | KeyCode::Esc => return ViewAction::Quit,
      _ => {}
    }
    ViewAction::None
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    self.render_list(frame, area);
    // Let search component render its overlay
    self.search.render_overlay(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    "Boards".to_string()
  }

  fn tick(&mut self) {
    self.query.poll();
  }
}
