use crossterm::event::KeyEvent;
use ratatui::prelude::*;

/// A keyboard shortcut hint for display in the header
#[derive(Debug, Clone)]
pub struct Shortcut {
  pub key: &'static str,
  pub label: &'static str,
}

impl Shortcut {
  pub const fn new(key: &'static str, label: &'static str) -> Self {
    Self { key, label }
  }
}

/// Actions that a view can request in response to user input
pub enum ViewAction {
  /// No action needed
  None,
  /// Push a new view onto the stack
  Push(Box<dyn View>),
  /// Pop current view from stack (go back)
  Pop,
}

/// Trait for view behavior
///
/// Views handle their own input modes (search, edit, etc.) and return
/// actions for the App to execute. This creates a clean delegation chain:
/// App → View → Components
///
/// Views that load data asynchronously should use Query<T> internally and
/// poll it in the tick() method.
pub trait View {
  /// Handle a key event, returning an action for App to execute
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction;

  /// Render the view to the frame
  fn render(&mut self, frame: &mut Frame, area: Rect);

  /// Get the breadcrumb label for this view
  fn breadcrumb_label(&self) -> String;

  /// Get the project key if this view has one (for header display)
  fn project(&self) -> Option<&str> {
    None
  }

  /// Called on each tick to allow views to poll async queries
  fn tick(&mut self) {}

  /// Get keyboard shortcuts to display in the header
  /// Override this to provide view-specific shortcuts
  fn shortcuts(&self) -> Vec<Shortcut> {
    vec![
      Shortcut::new(":", "command"),
      Shortcut::new("/", "filter"),
      Shortcut::new("q", "back"),
    ]
  }
}
