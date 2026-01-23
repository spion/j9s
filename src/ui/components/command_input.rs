use super::input::{InputResult, TextInput};
use super::KeyResult;
use crate::commands::{self, Command};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

/// Events emitted by command input that parent needs to handle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandEvent {
  /// Command submitted
  Submitted(String),
  /// Command cancelled
  Cancelled,
}

/// Command input component with autocomplete
#[derive(Debug, Clone, Default)]
pub struct CommandInput {
  input: TextInput,
  active: bool,
  selected_suggestion: usize,
}

impl CommandInput {
  pub fn new() -> Self {
    Self::default()
  }

  /// Check if command mode is currently active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Get the current input value
  pub fn value(&self) -> &str {
    self.input.value()
  }

  /// Activate command mode
  pub fn activate(&mut self) {
    self.active = true;
    self.input.clear();
    self.selected_suggestion = 0;
  }

  /// Get autocomplete suggestions for current input
  pub fn suggestions(&self) -> Vec<&'static Command> {
    commands::get_suggestions(self.input.value())
  }

  /// Get the selected suggestion index
  pub fn selected_suggestion(&self) -> usize {
    self.selected_suggestion
  }

  /// Handle a key event
  /// Call this regardless of active state - it handles activation too
  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<CommandEvent> {
    // If not active, check for activation key
    if !self.active {
      if key.code == KeyCode::Char(':') {
        self.activate();
        return KeyResult::Handled;
      }
      return KeyResult::NotHandled;
    }

    // Active - handle command-specific keys first
    match key.code {
      KeyCode::Esc => {
        self.active = false;
        self.input.clear();
        self.selected_suggestion = 0;
        return KeyResult::Event(CommandEvent::Cancelled);
      }
      KeyCode::Enter => {
        self.active = false;
        let cmd = self.resolve_command();
        self.input.clear();
        self.selected_suggestion = 0;
        return KeyResult::Event(CommandEvent::Submitted(cmd));
      }
      KeyCode::Tab | KeyCode::Down => {
        let suggestions = self.suggestions();
        if !suggestions.is_empty() {
          self.selected_suggestion = (self.selected_suggestion + 1) % suggestions.len();
        }
        return KeyResult::Handled;
      }
      KeyCode::BackTab | KeyCode::Up => {
        let suggestions = self.suggestions();
        if !suggestions.is_empty() {
          self.selected_suggestion = if self.selected_suggestion == 0 {
            suggestions.len() - 1
          } else {
            self.selected_suggestion - 1
          };
        }
        return KeyResult::Handled;
      }
      _ => {}
    }

    // Delegate to TextInput for text editing
    match self.input.handle_key(key) {
      InputResult::Consumed => {
        self.selected_suggestion = 0; // Reset on input change
        KeyResult::Handled
      }
      InputResult::Submitted(_) | InputResult::Cancelled => {
        // Already handled above
        KeyResult::Handled
      }
      InputResult::NotHandled => KeyResult::NotHandled,
    }
  }

  /// Resolve the final command (from suggestion or direct input)
  fn resolve_command(&self) -> String {
    let suggestions = self.suggestions();
    if !suggestions.is_empty() && self.selected_suggestion < suggestions.len() {
      suggestions[self.selected_suggestion].name.to_string()
    } else {
      self.input.value().trim().to_lowercase()
    }
  }

  /// Render the command overlay if active
  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active {
      return;
    }

    let suggestions = self.suggestions();

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
      .title(" Command ");

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
      Span::styled(":", Style::default().fg(Color::Yellow)),
      Span::raw(self.input.value()),
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
      state.select(Some(self.selected_suggestion));

      frame.render_stateful_widget(list, chunks[1], &mut state);
    }
  }
}
