use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Result of handling a key event in an input component
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
  /// Key was handled, continue input mode
  Consumed,
  /// Enter pressed, here's the submitted value
  Submitted(String),
  /// Escape pressed, input cancelled
  Cancelled,
  /// Key not handled, pass to next handler
  NotHandled,
}

/// Reusable text input component
#[derive(Debug, Clone, Default)]
pub struct TextInput {
  buffer: String,
  cursor: usize,
}

impl TextInput {
  pub fn new() -> Self {
    Self::default()
  }

  /// Get the current input value
  pub fn value(&self) -> &str {
    &self.buffer
  }

  /// Check if the input is empty
  pub fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  /// Clear the input
  pub fn clear(&mut self) {
    self.buffer.clear();
    self.cursor = 0;
  }

  /// Handle a key event, returning the result
  pub fn handle_key(&mut self, key: KeyEvent) -> InputResult {
    match key.code {
      KeyCode::Esc => InputResult::Cancelled,
      KeyCode::Enter => InputResult::Submitted(self.buffer.clone()),
      KeyCode::Backspace => {
        if self.cursor > 0 {
          self.cursor -= 1;
          self.buffer.remove(self.cursor);
        }
        InputResult::Consumed
      }
      KeyCode::Delete => {
        if self.cursor < self.buffer.len() {
          self.buffer.remove(self.cursor);
        }
        InputResult::Consumed
      }
      KeyCode::Left => {
        if self.cursor > 0 {
          self.cursor -= 1;
        }
        InputResult::Consumed
      }
      KeyCode::Right => {
        if self.cursor < self.buffer.len() {
          self.cursor += 1;
        }
        InputResult::Consumed
      }
      KeyCode::Home | KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.cursor = 0;
        InputResult::Consumed
      }
      KeyCode::End | KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.cursor = self.buffer.len();
        InputResult::Consumed
      }
      KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        // Clear line before cursor
        self.buffer = self.buffer[self.cursor..].to_string();
        self.cursor = 0;
        InputResult::Consumed
      }
      KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        // Delete word before cursor
        if self.cursor > 0 {
          let before = &self.buffer[..self.cursor];
          let new_cursor = before.trim_end().rfind(' ').map(|i| i + 1).unwrap_or(0);
          self.buffer = format!(
            "{}{}",
            &self.buffer[..new_cursor],
            &self.buffer[self.cursor..]
          );
          self.cursor = new_cursor;
        }
        InputResult::Consumed
      }
      KeyCode::Char(c) => {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
        InputResult::Consumed
      }
      _ => InputResult::NotHandled,
    }
  }

  /// Get cursor position for rendering
  pub fn cursor_position(&self) -> usize {
    self.cursor
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
  }

  fn ctrl_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
  }

  #[test]
  fn test_basic_input() {
    let mut input = TextInput::new();
    assert!(input.is_empty());

    input.handle_key(key(KeyCode::Char('h')));
    input.handle_key(key(KeyCode::Char('i')));
    assert_eq!(input.value(), "hi");
  }

  #[test]
  fn test_submit() {
    let mut input = TextInput::new();
    input.handle_key(key(KeyCode::Char('t')));
    input.handle_key(key(KeyCode::Char('e')));
    input.handle_key(key(KeyCode::Char('s')));
    input.handle_key(key(KeyCode::Char('t')));

    let result = input.handle_key(key(KeyCode::Enter));
    assert_eq!(result, InputResult::Submitted("test".to_string()));
  }

  #[test]
  fn test_cancel() {
    let mut input = TextInput::new();
    input.handle_key(key(KeyCode::Char('x')));

    let result = input.handle_key(key(KeyCode::Esc));
    assert_eq!(result, InputResult::Cancelled);
  }

  #[test]
  fn test_backspace() {
    let mut input = TextInput::new();
    input.handle_key(key(KeyCode::Char('a')));
    input.handle_key(key(KeyCode::Char('b')));
    input.handle_key(key(KeyCode::Char('c')));
    input.handle_key(key(KeyCode::Backspace));
    assert_eq!(input.value(), "ab");
  }

  #[test]
  fn test_cursor_movement() {
    let mut input = TextInput::new();
    input.handle_key(key(KeyCode::Char('a')));
    input.handle_key(key(KeyCode::Char('c')));
    input.handle_key(key(KeyCode::Left));
    input.handle_key(key(KeyCode::Char('b')));
    assert_eq!(input.value(), "abc");
  }

  #[test]
  fn test_ctrl_u_clear_before_cursor() {
    let mut input = TextInput::new();
    for c in "hello world".chars() {
      input.handle_key(key(KeyCode::Char(c)));
    }
    input.handle_key(key(KeyCode::Left));
    input.handle_key(key(KeyCode::Left));
    input.handle_key(key(KeyCode::Left));
    input.handle_key(key(KeyCode::Left));
    input.handle_key(key(KeyCode::Left));
    input.handle_key(ctrl_key(KeyCode::Char('u')));
    assert_eq!(input.value(), "world");
  }
}
