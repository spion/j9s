mod views;

use crate::app::{App, Mode, ViewState};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Main draw function
pub fn draw(frame: &mut Frame, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Min(1),    // Main content
      Constraint::Length(1), // Status bar
    ])
    .split(frame.area());

  // Draw current view
  if let Some(view) = app.current_view() {
    match view {
      ViewState::IssueList {
        issues,
        selected,
        project,
        loading,
      } => {
        views::issues::draw_issue_list(frame, chunks[0], issues, *selected, project, *loading);
      }
      ViewState::BoardList {
        boards,
        selected,
        loading,
      } => {
        views::boards::draw_board_list(frame, chunks[0], boards, *selected, *loading);
      }
      ViewState::IssueDetail { issue, loading } => {
        views::issue_detail::draw_issue_detail(frame, chunks[0], issue, *loading);
      }
    }
  }

  // Draw status bar
  draw_status_bar(frame, chunks[1], app);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
  let (content, style) = match app.mode() {
    Mode::Normal => {
      let hint = " :command  /search  j/k:nav  Enter:select  q:back  Ctrl-C:quit";
      (hint.to_string(), Style::default().fg(Color::DarkGray))
    }
    Mode::Command => {
      let cmd = format!(":{}", app.command_input());
      (cmd, Style::default().fg(Color::Yellow))
    }
    Mode::Search => {
      let search = format!("/{}", app.search_filter());
      (search, Style::default().fg(Color::Cyan))
    }
  };

  let paragraph = Paragraph::new(content).style(style);
  frame.render_widget(paragraph, area);
}
