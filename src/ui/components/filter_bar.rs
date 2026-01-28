use super::filter_source::FilterSource;
use super::KeyResult;
use crate::ui::renderfns::truncate;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::marker::PhantomData;

/// Events emitted by filter bar that parent needs to handle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterBarEvent {
  /// User navigated to a different filter value
  SelectionChanged,
}

/// Filter bar component for filtering items by field values.
/// Generic over:
/// - `F`: The filter source type (e.g., IssueFilterField)
/// - `T`: The item type being filtered (e.g., IssueSummary)
#[derive(Debug, Clone)]
pub struct FilterBar<F, T>
where
  F: FilterSource<T>,
{
  active: bool,
  field: F,
  values: Vec<Option<String>>, // None = unassigned
  selected: usize,             // 0 = All, 1+ = index into values
  _phantom: PhantomData<T>,
}

impl<F, T> Default for FilterBar<F, T>
where
  F: FilterSource<T>,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<F, T> FilterBar<F, T>
where
  F: FilterSource<T>,
{
  pub fn new() -> Self {
    Self {
      active: false,
      field: F::default(),
      values: Vec::new(),
      selected: 0,
      _phantom: PhantomData,
    }
  }

  /// Check if filter bar is currently active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Get the current filter field
  pub fn field(&self) -> F {
    self.field.clone()
  }

  /// Get the currently selected filter value
  /// Returns None if "All" is selected or no filter active
  pub fn selected_value(&self) -> Option<&Option<String>> {
    if !self.active || self.selected == 0 {
      None
    } else {
      self.values.get(self.selected - 1)
    }
  }

  /// Set the filter field and values, resetting selection to "All"
  pub fn set_field_and_values(&mut self, field: F, values: Vec<Option<String>>) {
    self.field = field.clone();
    self.values = values;
    self.selected = 0;
    self.active = field.is_active() && !self.values.is_empty();
  }

  /// Update values without changing field, preserving selection if possible
  pub fn update_values(&mut self, values: Vec<Option<String>>) {
    self.values = values;
    // Clamp selection to valid range
    let max_selection = if self.values.is_empty() {
      0
    } else {
      self.values.len()
    };
    if self.selected > max_selection {
      self.selected = 0;
    }
    self.active = self.field.is_active() && !self.values.is_empty();
  }

  /// Reset filter to inactive state
  pub fn clear(&mut self) {
    self.active = false;
    self.field = F::default();
    self.values.clear();
    self.selected = 0;
  }

  /// Handle a key event
  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<FilterBarEvent> {
    if !self.active || self.values.is_empty() {
      return KeyResult::NotHandled;
    }

    match key.code {
      KeyCode::PageUp => {
        self.navigate(-1);
        KeyResult::Event(FilterBarEvent::SelectionChanged)
      }
      KeyCode::PageDown => {
        self.navigate(1);
        KeyResult::Event(FilterBarEvent::SelectionChanged)
      }
      _ => KeyResult::NotHandled,
    }
  }

  /// Navigate filter tabs with wrapping
  fn navigate(&mut self, direction: i32) {
    if self.values.is_empty() {
      return;
    }

    // Total tabs = "All" + values
    let total_tabs = self.values.len() + 1;

    self.selected = if direction > 0 {
      (self.selected + 1) % total_tabs
    } else if self.selected == 0 {
      total_tabs - 1
    } else {
      self.selected - 1
    };
  }

  /// Render the filter bar
  pub fn render(&self, frame: &mut Frame, area: Rect) {
    if !self.active || self.values.is_empty() {
      return;
    }

    let mut spans = Vec::new();

    // Show current filter field name
    spans.push(Span::styled(
      format!("[{}] ", self.field.label()),
      Style::default().fg(Color::Yellow),
    ));

    // "All" tab (index 0)
    let all_style = if self.selected == 0 {
      Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
      Style::default().fg(Color::Gray)
    };
    spans.push(Span::styled(" All ", all_style));

    // Individual filter tabs
    for (idx, value) in self.values.iter().enumerate() {
      spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
      let is_selected = self.selected == idx + 1;
      let style = if is_selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
      } else {
        Style::default().fg(Color::Gray)
      };
      let label = match value {
        Some(v) => format!(" {} ", truncate(v, 15)),
        None => " Unassigned ".to_string(),
      };
      spans.push(Span::styled(label, style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
  }
}
