use super::input::{InputResult, TextInput};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Result of handling a key in search mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResult {
  /// Still in search mode, key was handled
  Active,
  /// Search submitted with query
  Submitted(String),
  /// Search cancelled
  Cancelled,
  /// Key not handled (pass to parent)
  NotHandled,
}

/// Search input component with activation/deactivation
#[derive(Debug, Clone, Default)]
pub struct SearchInput {
  input: TextInput,
  active: bool,
}

impl SearchInput {
  pub fn new() -> Self {
    Self::default()
  }

  /// Check if search is currently active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Get the current search query
  pub fn query(&self) -> &str {
    self.input.value()
  }

  /// Activate search mode
  pub fn activate(&mut self) {
    self.active = true;
    self.input.clear();
  }

  /// Handle a key event
  /// Call this regardless of active state - it handles activation too
  pub fn handle_key(&mut self, key: KeyEvent) -> SearchResult {
    // If not active, check for activation key
    if !self.active {
      if key.code == KeyCode::Char('/') {
        self.activate();
        return SearchResult::Active;
      }
      return SearchResult::NotHandled;
    }

    // Active - delegate to TextInput
    match self.input.handle_key(key) {
      InputResult::Submitted(query) => {
        self.active = false;
        SearchResult::Submitted(query)
      }
      InputResult::Cancelled => {
        self.active = false;
        self.input.clear();
        SearchResult::Cancelled
      }
      InputResult::Consumed => SearchResult::Active,
      InputResult::NotHandled => SearchResult::NotHandled,
    }
  }

  /// Render the search overlay if active
  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active {
      return;
    }

    // Calculate overlay dimensions - simpler than command overlay (no suggestions)
    let width = (area.width * 60 / 100).min(60).max(30);
    let height = 3; // Just input line with borders

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
      .title(" Search ");

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    if inner.height == 0 {
      return;
    }

    // Draw input line
    let input_line = Line::from(vec![
      Span::styled("/", Style::default().fg(Color::Yellow)),
      Span::raw(self.input.value()),
      Span::styled("_", Style::default().fg(Color::Yellow)), // Cursor
    ]);
    let input_para = Paragraph::new(input_line);
    frame.render_widget(input_para, inner);
  }
}
