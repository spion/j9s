use super::KeyResult;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};

#[derive(Debug, Clone)]
pub struct PickerOption {
  pub id: String,
  pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPickerEvent {
  Selected { id: String, label: String },
  Cancelled,
}

/// Generic picker overlay for selecting from a list of options.
#[derive(Debug, Clone, Default)]
pub struct FieldPicker {
  active: bool,
  options: Vec<PickerOption>,
  selected_idx: usize,
  title: String,
  current_id: String,
  current_label: String,
  allow_none: bool,
}

impl FieldPicker {
  pub fn new(title: &str) -> Self {
    Self {
      title: title.to_string(),
      ..Default::default()
    }
  }

  pub fn with_allow_none(mut self) -> Self {
    self.allow_none = true;
    self
  }

  pub fn set_options(&mut self, options: Vec<PickerOption>) {
    self.options = options;
    self.selected_idx = 0;
  }

  pub fn set_value(&mut self, id: &str, label: &str) {
    self.current_id = id.to_string();
    self.current_label = label.to_string();
  }

  pub fn clear_value(&mut self) {
    self.current_id.clear();
    self.current_label.clear();
  }

  pub fn current_id(&self) -> &str {
    &self.current_id
  }

  pub fn current_label(&self) -> &str {
    &self.current_label
  }

  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn show(&mut self) {
    if self.options.is_empty() && !self.allow_none {
      return;
    }
    self.active = true;
    self.selected_idx = if self.allow_none {
      if self.current_id.is_empty() {
        0
      } else {
        self
          .options
          .iter()
          .position(|o| o.id == self.current_id)
          .map(|i| i + 1)
          .unwrap_or(0)
      }
    } else {
      self
        .options
        .iter()
        .position(|o| o.id == self.current_id)
        .unwrap_or(0)
    };
  }

  fn total_items(&self) -> usize {
    self.options.len() + if self.allow_none { 1 } else { 0 }
  }

  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<FieldPickerEvent> {
    if !self.active {
      return KeyResult::NotHandled;
    }

    let total = self.total_items();
    match key.code {
      KeyCode::Esc | KeyCode::Char('q') => {
        self.active = false;
        KeyResult::Event(FieldPickerEvent::Cancelled)
      }
      KeyCode::Enter => {
        self.active = false;
        if self.allow_none && self.selected_idx == 0 {
          self.clear_value();
          KeyResult::Event(FieldPickerEvent::Selected {
            id: String::new(),
            label: String::new(),
          })
        } else {
          let idx = if self.allow_none {
            self.selected_idx - 1
          } else {
            self.selected_idx
          };
          if let Some(opt) = self.options.get(idx) {
            let id = opt.id.clone();
            let label = opt.label.clone();
            self.set_value(&id, &label);
            KeyResult::Event(FieldPickerEvent::Selected { id, label })
          } else {
            KeyResult::Event(FieldPickerEvent::Cancelled)
          }
        }
      }
      KeyCode::Char('j') | KeyCode::Down => {
        if total > 0 {
          self.selected_idx = (self.selected_idx + 1) % total;
        }
        KeyResult::Handled
      }
      KeyCode::Char('k') | KeyCode::Up => {
        if total > 0 {
          self.selected_idx = if self.selected_idx == 0 {
            total - 1
          } else {
            self.selected_idx - 1
          };
        }
        KeyResult::Handled
      }
      _ => KeyResult::Handled,
    }
  }

  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active {
      return;
    }

    let total = self.total_items();
    if total == 0 {
      return;
    }

    let max_label = self
      .options
      .iter()
      .map(|o| o.label.len())
      .max()
      .unwrap_or(10)
      .max(if self.allow_none { 6 } else { 0 });

    let width = (max_label as u16 + 6).min(area.width - 4).max(20);
    let height = (total as u16 + 2).min(area.height - 4).max(3);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, overlay_area);
    let block = Block::bordered()
      .border_style(Color::Yellow)
      .title(format!(" {} ", self.title));
    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    if inner.height == 0 {
      return;
    }

    let mut items: Vec<ListItem> = Vec::with_capacity(total);
    if self.allow_none {
      items.push(ListItem::new("(None)".dark_gray()));
    }
    for opt in &self.options {
      items.push(ListItem::new(opt.label.as_str().cyan()));
    }

    let list = List::new(items).highlight_style(Style::new().on_dark_gray().white());
    let mut state = ListState::default();
    state.select(Some(self.selected_idx));
    frame.render_stateful_widget(list, inner, &mut state);
  }
}
