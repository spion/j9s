use super::filter_bar::{FilterBar, FilterBarEvent};
use super::filter_field_picker::{FilterFieldPicker, FilterFieldPickerEvent};
use super::filter_source::FilterSource;
use super::key_result::KeyResult;
use super::search_input::{SearchEvent, SearchInput};
use crate::jira::types::{BoardColumn, IssueSummary};
use crate::ui::ensure_valid_selection;
use crate::ui::renderfns::{status_color, truncate};
use crate::ui::view::{ShortcutInfo, ShortcutProvider};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// Events emitted by TicketPanel that parent view needs to handle
#[derive(Debug, Clone)]
pub enum TicketPanelEvent {
  /// User pressed Enter on a ticket
  Selected(IssueSummary),
  /// User requested a refresh (r key)
  RefreshRequested,
  /// User wants to go back (q/Esc)
  Back,
  /// Filter selection changed
  FilterChanged,
}

/// Reusable ticket panel component combining:
/// - List/column (swimlane) view modes
/// - Filter bar with tabs
/// - Filter field picker overlay
/// - Search overlay
/// - Selection state management
pub struct TicketPanel<F: FilterSource<IssueSummary>> {
  // View mode
  column_mode: bool,
  columns: Vec<BoardColumn>,

  // Selection state
  list_state: ListState,
  column_selected: usize,
  row_selected: usize,

  // Filtering
  filter_bar: FilterBar<F, IssueSummary>,
  filter_field_picker: FilterFieldPicker<F, IssueSummary>,

  // Search
  search: SearchInput,
  search_filter: Option<String>,
}

impl<F: FilterSource<IssueSummary>> TicketPanel<F> {
  /// Create a new TicketPanel with given columns (empty for list-only mode)
  pub fn new(columns: Vec<BoardColumn>) -> Self {
    Self {
      column_mode: false,
      columns,
      list_state: ListState::default(),
      column_selected: 0,
      row_selected: 0,
      filter_bar: FilterBar::new(),
      filter_field_picker: FilterFieldPicker::new(),
      search: SearchInput::new(),
      search_filter: None,
    }
  }

  /// Create a list-only panel (no column mode)
  pub fn list_only() -> Self {
    Self::new(Vec::new())
  }

  /// Update the columns (for boards with dynamic column configuration)
  pub fn set_columns(&mut self, columns: Vec<BoardColumn>) {
    self.columns = columns;
    // If no columns, disable column mode
    if self.columns.is_empty() {
      self.column_mode = false;
    }
  }

  /// Check if column mode is available
  pub fn has_columns(&self) -> bool {
    !self.columns.is_empty()
  }

  /// Toggle between list and column modes
  pub fn toggle_column_mode(&mut self) {
    if self.has_columns() {
      self.column_mode = !self.column_mode;
      self.reset_selection();
    }
  }

  /// Update filter values from data
  pub fn update_filter_values(&mut self, items: &[IssueSummary]) {
    let field = self.filter_bar.field();
    let values = field.unique_values(items);
    self.filter_bar.update_values(values);
  }

  /// Set a new filter field
  fn set_filter_field(&mut self, field: F, items: &[IssueSummary]) {
    let values = field.unique_values(items);
    self.filter_bar.set_field_and_values(field, values);
    self.reset_selection();
  }

  /// Reset selection to beginning
  fn reset_selection(&mut self) {
    self.list_state.select(Some(0));
    self.column_selected = 0;
    self.row_selected = 0;
  }

  /// Get filtered items based on active filter and search
  pub fn filtered_items<'a>(&self, items: &'a [IssueSummary]) -> Vec<&'a IssueSummary> {
    let field = self.filter_bar.field();

    // First apply field filter
    let filtered = field.filter(items, self.filter_bar.selected_value());

    // Then apply search filter
    let Some(query) = &self.search_filter else {
      return filtered;
    };
    let query_lower = query.to_lowercase();
    filtered
      .into_iter()
      .filter(|issue| {
        issue.key.to_lowercase().contains(&query_lower)
          || issue.summary.to_lowercase().contains(&query_lower)
          || issue.status.to_lowercase().contains(&query_lower)
          || issue
            .assignee
            .as_ref()
            .map_or(false, |a| a.to_lowercase().contains(&query_lower))
      })
      .collect()
  }

  /// Get items for a specific column (by status)
  fn items_for_column<'a>(
    &self,
    items: &'a [IssueSummary],
    column: &BoardColumn,
  ) -> Vec<&'a IssueSummary> {
    self
      .filtered_items(items)
      .into_iter()
      .filter(|issue| column.statuses.iter().any(|s| s.id == issue.status_id))
      .collect()
  }

  /// Get the currently selected item
  pub fn selected<'a>(&self, items: &'a [IssueSummary]) -> Option<&'a IssueSummary> {
    let filtered = self.filtered_items(items);

    if self.column_mode {
      if let Some(column) = self.columns.get(self.column_selected) {
        let column_items = self.items_for_column(items, column);
        return column_items.get(self.row_selected).copied();
      }
      None
    } else {
      self
        .list_state
        .selected()
        .and_then(|idx| filtered.get(idx).copied())
    }
  }

  /// Handle key events, returning an event for the parent view
  pub fn handle_key(
    &mut self,
    key: KeyEvent,
    items: &[IssueSummary],
  ) -> KeyResult<TicketPanelEvent> {
    // First try overlays
    if let Some(action) = self.handle_overlays(key, items) {
      return action;
    }

    // Then navigation
    if let Some(action) = self.handle_navigation(key, items) {
      return action;
    }

    // Then toggles and actions
    if let Some(action) = self.handle_actions(key, items) {
      return action;
    }

    KeyResult::NotHandled
  }

  fn handle_overlays(
    &mut self,
    key: KeyEvent,
    items: &[IssueSummary],
  ) -> Option<KeyResult<TicketPanelEvent>> {
    // Filter field picker
    match self.filter_field_picker.handle_key(key) {
      KeyResult::Handled => return Some(KeyResult::Handled),
      KeyResult::Event(FilterFieldPickerEvent::Selected(field)) => {
        self.set_filter_field(field, items);
        return Some(KeyResult::Event(TicketPanelEvent::FilterChanged));
      }
      KeyResult::Event(FilterFieldPickerEvent::Cancelled) => return Some(KeyResult::Handled),
      KeyResult::NotHandled => {}
    }

    // Filter bar (tab navigation)
    match self.filter_bar.handle_key(key) {
      KeyResult::Handled => return Some(KeyResult::Handled),
      KeyResult::Event(FilterBarEvent::SelectionChanged) => {
        self.reset_selection();
        return Some(KeyResult::Event(TicketPanelEvent::FilterChanged));
      }
      KeyResult::NotHandled => {}
    }

    // Search
    match self.search.handle_key(key) {
      KeyResult::Handled => return Some(KeyResult::Handled),
      KeyResult::Event(SearchEvent::Changed(query)) => {
        self.search_filter = if query.is_empty() { None } else { Some(query) };
        self.reset_selection();
        return Some(KeyResult::Handled);
      }
      KeyResult::Event(SearchEvent::Submitted) => return Some(KeyResult::Handled),
      KeyResult::NotHandled => {}
    }

    None
  }

  fn handle_navigation(
    &mut self,
    key: KeyEvent,
    items: &[IssueSummary],
  ) -> Option<KeyResult<TicketPanelEvent>> {
    if self.column_mode {
      match key.code {
        KeyCode::Char('h') | KeyCode::Left => {
          self.navigate_column(-1, items);
          Some(KeyResult::Handled)
        }
        KeyCode::Char('l') | KeyCode::Right => {
          self.navigate_column(1, items);
          Some(KeyResult::Handled)
        }
        KeyCode::Char('j') | KeyCode::Down => {
          self.navigate_row(1, items);
          Some(KeyResult::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.navigate_row(-1, items);
          Some(KeyResult::Handled)
        }
        _ => None,
      }
    } else {
      match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
          self.list_state.select_next();
          Some(KeyResult::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.list_state.select_previous();
          Some(KeyResult::Handled)
        }
        _ => None,
      }
    }
  }

  fn handle_actions(
    &mut self,
    key: KeyEvent,
    items: &[IssueSummary],
  ) -> Option<KeyResult<TicketPanelEvent>> {
    match key.code {
      KeyCode::Char('f') => {
        self.filter_field_picker.show();
        Some(KeyResult::Handled)
      }
      KeyCode::Char('s') if self.has_columns() => {
        self.toggle_column_mode();
        Some(KeyResult::Handled)
      }
      KeyCode::Char('r') => Some(KeyResult::Event(TicketPanelEvent::RefreshRequested)),
      KeyCode::Enter => {
        if let Some(issue) = self.selected(items) {
          Some(KeyResult::Event(TicketPanelEvent::Selected(issue.clone())))
        } else {
          Some(KeyResult::Handled)
        }
      }
      KeyCode::Char('q') | KeyCode::Esc => Some(KeyResult::Event(TicketPanelEvent::Back)),
      _ => None,
    }
  }

  fn navigate_column(&mut self, direction: i32, items: &[IssueSummary]) {
    let num_columns = self.columns.len();
    if num_columns == 0 {
      return;
    }

    if direction > 0 {
      self.column_selected = (self.column_selected + 1).min(num_columns - 1);
    } else {
      self.column_selected = self.column_selected.saturating_sub(1);
    }

    // Clamp row selection to new column
    if let Some(column) = self.columns.get(self.column_selected) {
      let col_items = self.items_for_column(items, column);
      self.row_selected = self.row_selected.min(col_items.len().saturating_sub(1));
    }
  }

  fn navigate_row(&mut self, direction: i32, items: &[IssueSummary]) {
    if let Some(column) = self.columns.get(self.column_selected) {
      let col_items = self.items_for_column(items, column);
      if col_items.is_empty() {
        return;
      }

      if direction > 0 {
        self.row_selected = (self.row_selected + 1).min(col_items.len() - 1);
      } else {
        self.row_selected = self.row_selected.saturating_sub(1);
      }
    }
  }

  /// Render the panel
  pub fn render(
    &mut self,
    frame: &mut Frame,
    area: Rect,
    items: &[IssueSummary],
    title: &str,
    is_loading: bool,
  ) {
    // Split area for filter bar (if active) and main content
    let (filter_area, content_area) = if self.filter_bar.is_active() {
      let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
      (Some(chunks[0]), chunks[1])
    } else {
      (None, area)
    };

    // Render filter bar when active
    if let Some(filter_area) = filter_area {
      self.filter_bar.render(frame, filter_area);
    }

    // Render main content
    if self.column_mode {
      self.render_columns(frame, content_area, items, title, is_loading);
    } else {
      self.render_list(frame, content_area, items, title, is_loading);
    }

    // Render overlays
    self.search.render_overlay(frame, area);
    self.filter_field_picker.render_overlay(frame, area);
  }

  fn render_list(
    &mut self,
    frame: &mut Frame,
    area: Rect,
    items: &[IssueSummary],
    title: &str,
    is_loading: bool,
  ) {
    let filtered = self.filtered_items(items);
    let len = filtered.len();
    ensure_valid_selection(&mut self.list_state, len);

    let search_indicator = self
      .search_filter
      .as_ref()
      .map(|q| format!(" [/{}]", q))
      .unwrap_or_default();

    let display_title = if is_loading {
      format!(" {} (loading...) ", title)
    } else {
      format!(" {} ({} issues){} ", title, len, search_indicator)
    };

    let block = Block::default()
      .title(display_title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if items.is_empty() && !is_loading {
      let content = "No issues found.";
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    let list_items: Vec<ListItem> = filtered
      .iter()
      .map(|issue| {
        let color = status_color(&issue.status);
        let line = Line::from(vec![
          Span::styled(
            format!("{:<15}", issue.key),
            Style::default().fg(Color::Cyan),
          ),
          Span::raw(" "),
          Span::styled(
            format!("{:<15}", truncate(&issue.status, 15)),
            Style::default().fg(color),
          ),
          Span::raw(" "),
          Span::raw(issue.summary.clone()),
        ]);
        ListItem::new(line)
      })
      .collect();

    let list = List::new(list_items)
      .block(block)
      .highlight_style(
        Style::default()
          .bg(Color::DarkGray)
          .add_modifier(Modifier::BOLD),
      )
      .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut self.list_state);
  }

  fn render_columns(
    &self,
    frame: &mut Frame,
    area: Rect,
    items: &[IssueSummary],
    title: &str,
    is_loading: bool,
  ) {
    if self.columns.is_empty() {
      let block = Block::default()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

      let content = if is_loading {
        "Loading..."
      } else {
        "No columns configured."
      };
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    // Distribute columns evenly
    let constraints: Vec<Constraint> = self
      .columns
      .iter()
      .map(|_| Constraint::Ratio(1, self.columns.len() as u32))
      .collect();
    let col_areas = Layout::horizontal(constraints).split(area);

    // Render each column
    for (col_idx, column) in self.columns.iter().enumerate() {
      let col_items = self.items_for_column(items, column);
      let is_selected_column = col_idx == self.column_selected;
      let col_area = col_areas[col_idx];

      let border_color = if is_selected_column {
        Color::Yellow
      } else {
        Color::Blue
      };

      let col_title = format!(" {} ({}) ", truncate(&column.name, 15), col_items.len());
      let block = Block::default()
        .title(col_title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

      let list_items: Vec<ListItem> = col_items
        .iter()
        .map(|issue| {
          let issue_id = Line::from(vec![Span::styled(
            &issue.key,
            Style::default().fg(Color::Cyan),
          )]);
          let issue_title = Line::from(vec![Span::raw(truncate(
            &issue.summary,
            col_area.width.saturating_sub(4) as usize,
          ))]);
          ListItem::new(vec![issue_id, issue_title])
        })
        .collect();

      let list = List::new(list_items)
        .block(block)
        .highlight_style(
          Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

      if is_selected_column {
        let mut state = ListState::default();
        state.select(Some(self.row_selected));
        frame.render_stateful_widget(list, col_area, &mut state);
      } else {
        frame.render_widget(list, col_area);
      }
    }
  }
}

impl<F: FilterSource<IssueSummary>> ShortcutProvider for TicketPanel<F> {
  fn shortcuts(&self) -> Vec<ShortcutInfo> {
    let mut shortcuts = vec![
      ShortcutInfo::new("r", "refresh").with_priority(100),
      ShortcutInfo::new("f", "filter").with_priority(101),
    ];

    // Filter tab navigation shortcuts
    if self.filter_bar.is_active() {
      shortcuts.push(ShortcutInfo::new("PgUp/Dn", "filter tab").with_priority(102));
    }

    // Column mode shortcuts
    if self.has_columns() {
      shortcuts.push(ShortcutInfo::new("s", "swimlane").with_priority(110));
    }

    shortcuts
  }
}
