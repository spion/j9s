use crate::jira::types::Board;
use crate::jira::CachedJiraClient;
use crate::query::{Query, QueryState};
use crate::ui::components::{KeyResult, SearchEvent, SearchInput};
use crate::ui::ensure_valid_selection;
use crate::ui::view::{View, ViewAction};
use crate::ui::views::BoardView;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::collections::BTreeSet;

/// View for displaying a list of boards
pub struct BoardListView {
  jira: CachedJiraClient,
  hide_swimlanes: BTreeSet<String>,
  query: Query<Vec<Board>>,
  list_state: ListState,
  search: SearchInput,
  search_filter: Option<String>,
}

impl BoardListView {
  pub fn new(
    project: Option<String>,
    jira: CachedJiraClient,
    hide_swimlanes: BTreeSet<String>,
  ) -> Self {
    let jira_for_query = jira.clone();
    let mut query = Query::new(move || {
      let jira = jira_for_query.clone();
      let project = project.clone();
      async move {
        jira
          .get_boards(project.as_deref())
          .await
          .map_err(|e| e.to_string())
      }
    });

    // Start fetching immediately
    query.fetch();

    Self {
      jira,
      hide_swimlanes,
      query,
      list_state: ListState::default(),
      search: SearchInput::new(),
      search_filter: None,
    }
  }

  fn boards(&self) -> &[Board] {
    self.query.data().map(|v| v.as_slice()).unwrap_or(&[])
  }

  fn filtered_boards(&self) -> Vec<&Board> {
    let boards = self.boards();
    let Some(query) = &self.search_filter else {
      return boards.iter().collect();
    };
    let query_lower = query.to_lowercase();
    boards
      .iter()
      .filter(|board| {
        board.name.to_lowercase().contains(&query_lower)
          || board.board_type.to_lowercase().contains(&query_lower)
      })
      .collect()
  }

  fn is_loading(&self) -> bool {
    self.query.is_loading()
  }

  fn render_list(&mut self, frame: &mut Frame, area: Rect) {
    let len = self.filtered_boards().len();
    ensure_valid_selection(&mut self.list_state, len);

    let search_indicator = self
      .search_filter
      .as_ref()
      .map(|q| format!(" [/{}]", q))
      .unwrap_or_default();

    let title = match self.query.state() {
      QueryState::Loading => " Boards (loading...) ".to_string(),
      QueryState::Error(e) => format!(" Boards (error: {}) ", e),
      _ => format!(" Boards ({}){} ", len, search_indicator),
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
      .filtered_boards()
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

  // Key handling helpers for or_else chain pattern
  fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match self.search.handle_key(key) {
      KeyResult::Handled => Some(ViewAction::None),
      KeyResult::Event(SearchEvent::Changed(query)) => {
        self.search_filter = if query.is_empty() { None } else { Some(query) };
        self.list_state.select(Some(0));
        Some(ViewAction::None)
      }
      KeyResult::Event(SearchEvent::Submitted) => Some(ViewAction::None),
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
          if let Some(board) = self.filtered_boards().get(idx) {
            return Some(ViewAction::Push(Box::new(BoardView::new(
              board.id,
              board.name.clone(),
              self.jira.clone(),
              self.hide_swimlanes.clone(),
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

impl View for BoardListView {
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
    "Boards".to_string()
  }

  fn tick(&mut self) {
    self.query.poll();
  }
}
