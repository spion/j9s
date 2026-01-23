use super::KeyResult;
use crate::jira::types::StatusInfo;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

/// Events emitted by status picker that parent needs to handle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusPickerEvent {
  /// Status selected (returns status id)
  Selected(String),
  /// Picker cancelled
  Cancelled,
}

/// Status picker component for selecting target status in swimlane transitions
#[derive(Debug, Clone, Default)]
pub struct StatusPicker {
  active: bool,
  statuses: Vec<StatusInfo>,
  selected: usize,
  title: String,
}

impl StatusPicker {
  pub fn new() -> Self {
    Self::default()
  }

  /// Check if picker is currently active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Show the picker with the given statuses
  pub fn show(&mut self, title: String, statuses: Vec<StatusInfo>) {
    self.active = true;
    self.statuses = statuses;
    self.selected = 0;
    self.title = title;
  }

  /// Hide the picker
  pub fn hide(&mut self) {
    self.active = false;
    self.statuses.clear();
    self.selected = 0;
  }

  /// Handle a key event
  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<StatusPickerEvent> {
    if !self.active {
      return KeyResult::NotHandled;
    }

    match key.code {
      KeyCode::Esc | KeyCode::Char('q') => {
        self.hide();
        KeyResult::Event(StatusPickerEvent::Cancelled)
      }
      KeyCode::Enter => {
        if let Some(status) = self.statuses.get(self.selected) {
          let id = status.id.clone();
          self.hide();
          KeyResult::Event(StatusPickerEvent::Selected(id))
        } else {
          self.hide();
          KeyResult::Event(StatusPickerEvent::Cancelled)
        }
      }
      KeyCode::Char('j') | KeyCode::Down => {
        if !self.statuses.is_empty() {
          self.selected = (self.selected + 1) % self.statuses.len();
        }
        KeyResult::Handled
      }
      KeyCode::Char('k') | KeyCode::Up => {
        if !self.statuses.is_empty() {
          self.selected = if self.selected == 0 {
            self.statuses.len() - 1
          } else {
            self.selected - 1
          };
        }
        KeyResult::Handled
      }
      _ => KeyResult::Handled,
    }
  }

  /// Render the status picker overlay if active
  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active || self.statuses.is_empty() {
      return;
    }

    // Calculate overlay dimensions
    let max_name_len = self
      .statuses
      .iter()
      .map(|s| s.name.len())
      .max()
      .unwrap_or(10);
    let width = (max_name_len as u16 + 6).min(area.width - 4).max(20);
    let height = (self.statuses.len() as u16 + 2).min(area.height - 4).max(3);

    // Center the overlay
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let overlay_area = Rect::new(x, y, width, height);

    // Clear the area behind the overlay
    frame.render_widget(Clear, overlay_area);

    // Draw the border/block
    let block = Block::default()
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Yellow))
      .title(format!(" {} ", self.title));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    if inner.height == 0 {
      return;
    }

    // Draw status list
    let items: Vec<ListItem> = self
      .statuses
      .iter()
      .map(|status| {
        let line = Line::from(vec![Span::styled(
          &status.name,
          Style::default().fg(Color::Cyan),
        )]);
        ListItem::new(line)
      })
      .collect();

    let list =
      List::new(items).highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let mut state = ListState::default();
    state.select(Some(self.selected));

    frame.render_stateful_widget(list, inner, &mut state);
  }
}
