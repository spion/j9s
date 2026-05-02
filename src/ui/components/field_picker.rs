use super::keyword_match::keyword_match;
use super::KeyResult;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

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
  searchable: bool,
  query: String,
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

  /// Enable type-to-filter mode. When enabled, character keys append to a
  /// query that filters options via `keyword_match`; `Esc` is the only close
  /// key (`q` types `q`).
  pub fn with_search(mut self) -> Self {
    self.searchable = true;
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
    self.query.clear();
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

  /// Indices into `self.options` that match the current query.
  /// When not searching or query is empty, returns 0..options.len().
  fn filtered_indices(&self) -> Vec<usize> {
    if !self.searchable || self.query.trim().is_empty() {
      return (0..self.options.len()).collect();
    }
    self
      .options
      .iter()
      .enumerate()
      .filter_map(|(i, opt)| {
        let haystack = format!("{} {}", opt.id, opt.label);
        keyword_match(&haystack, &self.query).then_some(i)
      })
      .collect()
  }

  /// Whether the "(None)" row should be visible right now.
  fn show_none_row(&self) -> bool {
    self.allow_none && (!self.searchable || self.query.trim().is_empty())
  }

  /// Total visible rows (None row + filtered options).
  fn visible_rows(&self) -> usize {
    self.filtered_indices().len() + if self.show_none_row() { 1 } else { 0 }
  }

  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<FieldPickerEvent> {
    if !self.active {
      return KeyResult::NotHandled;
    }

    match key.code {
      KeyCode::Esc => {
        self.active = false;
        return KeyResult::Event(FieldPickerEvent::Cancelled);
      }
      KeyCode::Enter => return self.commit_selection(),
      KeyCode::Up => {
        self.move_selection(-1);
        return KeyResult::Handled;
      }
      KeyCode::Down => {
        self.move_selection(1);
        return KeyResult::Handled;
      }
      _ => {}
    }

    if self.searchable {
      match key.code {
        KeyCode::Backspace => {
          self.query.pop();
          self.selected_idx = 0;
          KeyResult::Handled
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
          self.query.push(c);
          self.selected_idx = 0;
          KeyResult::Handled
        }
        _ => KeyResult::Handled,
      }
    } else {
      match key.code {
        KeyCode::Char('q') => {
          self.active = false;
          KeyResult::Event(FieldPickerEvent::Cancelled)
        }
        KeyCode::Char('j') => {
          self.move_selection(1);
          KeyResult::Handled
        }
        KeyCode::Char('k') => {
          self.move_selection(-1);
          KeyResult::Handled
        }
        _ => KeyResult::Handled,
      }
    }
  }

  fn move_selection(&mut self, delta: i32) {
    let total = self.visible_rows();
    if total == 0 {
      return;
    }
    let cur = self.selected_idx as i32;
    let next = (cur + delta).rem_euclid(total as i32);
    self.selected_idx = next as usize;
  }

  fn commit_selection(&mut self) -> KeyResult<FieldPickerEvent> {
    self.active = false;
    let show_none = self.show_none_row();
    if show_none && self.selected_idx == 0 {
      self.clear_value();
      return KeyResult::Event(FieldPickerEvent::Selected {
        id: String::new(),
        label: String::new(),
      });
    }
    let row_in_filtered = if show_none {
      self.selected_idx - 1
    } else {
      self.selected_idx
    };
    let indices = self.filtered_indices();
    if let Some(&opt_idx) = indices.get(row_in_filtered) {
      if let Some(opt) = self.options.get(opt_idx) {
        let id = opt.id.clone();
        let label = opt.label.clone();
        self.set_value(&id, &label);
        return KeyResult::Event(FieldPickerEvent::Selected { id, label });
      }
    }
    KeyResult::Event(FieldPickerEvent::Cancelled)
  }

  pub fn render_overlay(&self, frame: &mut Frame, area: Rect) {
    if !self.active {
      return;
    }

    let indices = self.filtered_indices();
    let show_none = self.show_none_row();
    let visible_rows = indices.len() + if show_none { 1 } else { 0 };

    let max_label = indices
      .iter()
      .map(|&i| self.options[i].label.len())
      .max()
      .unwrap_or(10)
      .max(if show_none { 6 } else { 0 });

    // Extra row for the search line when searchable & active.
    let search_row = if self.searchable { 1 } else { 0 };

    let width = (max_label as u16 + 6)
      .min(area.width.saturating_sub(4))
      .max(24);
    let height = (visible_rows as u16 + search_row + 2)
      .min(area.height.saturating_sub(4))
      .max(3);
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

    let (search_area, list_area) = if self.searchable && inner.height > 1 {
      let s = Rect::new(inner.x, inner.y, inner.width, 1);
      let l = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
      (Some(s), l)
    } else {
      (None, inner)
    };

    if let Some(area) = search_area {
      let line = Line::from(vec![
        "/".yellow(),
        Span::raw(self.query.as_str()),
        "_".yellow(),
      ]);
      frame.render_widget(Paragraph::new(line), area);
    }

    let mut items: Vec<ListItem> = Vec::with_capacity(visible_rows);
    if show_none {
      items.push(ListItem::new("(None)".dark_gray()));
    }
    for &i in &indices {
      items.push(ListItem::new(self.options[i].label.as_str().cyan()));
    }

    let list = List::new(items).highlight_style(Style::new().on_dark_gray().white());
    let mut state = ListState::default();
    state.select(Some(self.selected_idx.min(visible_rows.saturating_sub(1))));
    frame.render_stateful_widget(list, list_area, &mut state);
  }
}
