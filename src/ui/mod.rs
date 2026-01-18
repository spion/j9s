pub mod components;
pub mod renderfns;
pub mod view;
pub mod views;

use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use renderfns::{draw_footer, draw_header};

/// Ensure ListState selection is valid for the given item count.
/// - No items → no selection
/// - Has items but no selection → select first
/// - Selection out of bounds → clamp to last item
pub fn ensure_valid_selection(state: &mut ListState, len: usize) {
  match (state.selected(), len) {
    (_, 0) => state.select(None),
    (None, _) => state.select(Some(0)),
    (Some(i), len) if i >= len => state.select(Some(len - 1)),
    _ => {}
  }
}

/// Main draw function
pub fn draw(frame: &mut Frame, app: &mut App) {
  // Header is always 2 lines
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(2), // Header (always 2 lines)
      Constraint::Min(1),    // Main content
      Constraint::Length(1), // Footer (breadcrumb)
    ])
    .split(frame.area());

  // Draw header with dynamic shortcuts
  let shortcuts = app.current_shortcuts();
  draw_header(
    frame,
    chunks[0],
    app.jira_url(),
    app.current_project(),
    &shortcuts,
  );

  // Draw current view (view handles its own overlays like search)
  if let Some(view) = app.current_view_mut() {
    view.render(frame, chunks[1]);
  }

  // Let command component render its overlay if active
  app.render_command_overlay(frame, chunks[1]);

  // Draw footer breadcrumb
  let breadcrumb = app.view_breadcrumb();
  draw_footer(frame, chunks[2], &breadcrumb);
}
