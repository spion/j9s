use crate::event::JiraEvent;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

/// Actions that a view can request in response to user input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewAction {
  /// No action needed
  None,
  /// Load full issue details
  LoadIssue { key: String },
  /// Load board details
  LoadBoard { id: u64 },
  /// Pop current view from stack (go back)
  Pop,
  /// Quit the application
  Quit,
}

/// Trait for view behavior
///
/// Views handle their own input modes (search, edit, etc.) and return
/// actions for the App to execute. This creates a clean delegation chain:
/// App → View → Components
pub trait View {
  /// Handle a key event, returning an action for App to execute
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction;

  /// Render the view to the frame
  fn render(&self, frame: &mut Frame, area: Rect);

  /// Get the breadcrumb label for this view
  fn breadcrumb_label(&self) -> String;

  /// Get the project key if this view has one (for header display)
  fn project(&self) -> Option<&str> {
    None
  }

  /// Set the loading state
  fn set_loading(&mut self, loading: bool);

  /// Receive data from async operations
  /// Returns true if the event was handled by this view
  fn receive_data(&mut self, event: &JiraEvent) -> bool {
    let _ = event;
    false
  }
}
