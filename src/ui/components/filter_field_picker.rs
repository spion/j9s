use super::filter_source::FilterSource;
use super::KeyResult;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};
use std::marker::PhantomData;

/// Events emitted by filter field picker that parent needs to handle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterFieldPickerEvent<F> {
  /// Field selected
  Selected(F),
  /// Picker cancelled
  Cancelled,
}

/// Filter field picker component for selecting which field to group/filter by.
/// Generic over:
/// - `F`: The filter source type (e.g., IssueFilterField)
/// - `T`: The item type being filtered (e.g., IssueSummary)
#[derive(Debug, Clone)]
pub struct FilterFieldPicker<F, T>
where
  F: FilterSource<T>,
{
  active: bool,
  selected: usize,
  _phantom: PhantomData<(F, T)>,
}

impl<F, T> Default for FilterFieldPicker<F, T>
where
  F: FilterSource<T>,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<F, T> FilterFieldPicker<F, T>
where
  F: FilterSource<T>,
{
  pub fn new() -> Self {
    Self {
      active: false,
      selected: 0,
      _phantom: PhantomData,
    }
  }

  /// Check if picker is currently active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Show the picker
  pub fn show(&mut self) {
    self.active = true;
    self.selected = 0;
  }

  /// Hide the picker
  pub fn hide(&mut self) {
    self.active = false;
    self.selected = 0;
  }

  /// Handle a key event
  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<FilterFieldPickerEvent<F>> {
    if !self.active {
      return KeyResult::NotHandled;
    }

    let fields = F::all_variants();

    match key.code {
      KeyCode::Esc | KeyCode::Char('q') => {
        self.hide();
        KeyResult::Event(FilterFieldPickerEvent::Cancelled)
      }
      KeyCode::Enter => {
        if let Some(field) = fields.get(self.selected) {
          self.hide();
          KeyResult::Event(FilterFieldPickerEvent::Selected(field.clone()))
        } else {
          self.hide();
          KeyResult::Event(FilterFieldPickerEvent::Cancelled)
        }
      }
      KeyCode::Char('j') | KeyCode::Down => {
        if !fields.is_empty() {
          self.selected = (self.selected + 1) % fields.len();
        }
        KeyResult::Handled
      }
      KeyCode::Char('k') | KeyCode::Up => {
        if !fields.is_empty() {
          self.selected = if self.selected == 0 {
            fields.len() - 1
          } else {
            self.selected - 1
          };
        }
        KeyResult::Handled
      }
      _ => KeyResult::Handled,
    }
  }

  /// Render the filter field picker overlay if active
  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active {
      return;
    }

    let fields = F::all_variants();

    // Calculate overlay dimensions
    let max_name_len = fields.iter().map(|f| f.label().len()).max().unwrap_or(10);
    let width = (max_name_len as u16 + 6).min(area.width - 4).max(20);
    let height = (fields.len() as u16 + 2).min(area.height - 4).max(3);

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
      .title(" Filter By ");

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    if inner.height == 0 {
      return;
    }

    // Draw field list
    let items: Vec<ListItem> = fields
      .iter()
      .map(|field| {
        let line = Line::from(vec![Span::styled(
          field.label(),
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
