mod components;
mod views;

use crate::app::{App, Mode, ViewState};
use components::{draw_command_overlay, draw_footer, draw_header};
use ratatui::prelude::*;

/// Main draw function
pub fn draw(frame: &mut Frame, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1), // Header
      Constraint::Min(1),    // Main content
      Constraint::Length(1), // Footer (breadcrumb)
    ])
    .split(frame.area());

  // Draw header
  draw_header(frame, chunks[0], app.jira_url(), app.current_project());

  // Draw current view
  if let Some(view) = app.current_view() {
    match view {
      ViewState::IssueList {
        issues,
        selected,
        project,
        loading,
      } => {
        views::issues::draw_issue_list(frame, chunks[1], issues, *selected, project, *loading);
      }
      ViewState::BoardList {
        boards,
        selected,
        loading,
      } => {
        views::boards::draw_board_list(frame, chunks[1], boards, *selected, *loading);
      }
      ViewState::IssueDetail { issue, loading } => {
        views::issue_detail::draw_issue_detail(frame, chunks[1], issue, *loading);
      }
    }
  }

  // Draw command/search overlay if in command or search mode
  match app.mode() {
    Mode::Command => {
      let suggestions = app.autocomplete_suggestions();
      draw_command_overlay(
        frame,
        chunks[1],
        ":",
        app.command_input(),
        &suggestions,
        app.selected_suggestion(),
      );
    }
    Mode::Search => {
      draw_command_overlay(
        frame,
        chunks[1],
        "/",
        app.search_filter(),
        &[], // No autocomplete for search
        0,
      );
    }
    Mode::Normal => {}
  }

  // Draw footer breadcrumb
  let breadcrumb = app.view_breadcrumb();
  draw_footer(frame, chunks[2], &breadcrumb);
}
