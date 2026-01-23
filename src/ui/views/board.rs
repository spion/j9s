use crate::jira::client::JiraClient;
use crate::jira::types::{BoardColumn, BoardConfiguration, IssueSummary, StatusInfo};
use crate::query::{Query, QueryState};
use crate::ui::components::{
  FilterBar, FilterBarEvent, FilterField, FilterFieldPicker, FilterFieldPickerEvent, KeyResult,
  SearchEvent, SearchInput, StatusPicker, StatusPickerEvent,
};
use crate::ui::ensure_valid_selection;
use crate::ui::renderfns::{status_color, truncate};
use crate::ui::view::{Shortcut, View, ViewAction};
use crate::ui::views::IssueDetailView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::collections::BTreeSet;
use tracing::info;

/// Combined board data fetched in parallel
#[derive(Clone)]
struct BoardData {
  issues: Vec<IssueSummary>,
  columns: Vec<BoardColumn>,
}

/// View for displaying a single board with its issues
pub struct BoardView {
  #[allow(dead_code)]
  board_id: u64,
  board_name: String,

  // Jira client for API calls
  jira: JiraClient,

  // Config: swimlane names to hide (lowercase for case-insensitive matching)
  hide_swimlanes: BTreeSet<String>,

  // Data query
  query: Query<BoardData>,

  // UI state
  list_state: ListState,    // Selection for list mode
  swimlane_selected: usize, // Selection within column for swimlane mode
  selected_column: usize,
  swimlane_mode: bool,

  // Filter state (client-side filtering by field values)
  filter_bar: FilterBar,
  filter_picker: FilterFieldPicker, // Picker for selecting filter field

  // Components
  search: SearchInput,
  status_picker: StatusPicker,

  // Status update state
  pending_issue_key: Option<String>, // Issue key waiting for status picker selection
  status_mutation: Option<Query<()>>,
  error_message: Option<String>,
}

impl BoardView {
  pub fn new(
    board_id: u64,
    board_name: String,
    jira: JiraClient,
    hide_swimlanes: BTreeSet<String>,
  ) -> Self {
    let jira_for_query = jira.clone();
    let mut query = Query::new(move || {
      let jira = jira_for_query.clone();
      async move {
        // Fetch all board data in parallel
        // Filter: unresolved issues + resolved in past 2 weeks
        let jql = "resolution IS EMPTY OR resolved >= -2w";
        let (issues_result, config_result) = tokio::join!(
          jira.get_board_issues(board_id, Some(jql)),
          jira.get_board_configuration(board_id),
        );

        let issues = issues_result.map_err(|e| e.to_string())?;
        let config = config_result.unwrap_or_else(|_| BoardConfiguration {
          columns: Vec::new(),
        });

        Ok(BoardData {
          issues,
          columns: config.columns,
        })
      }
    });

    // Start fetching immediately
    query.fetch();

    Self {
      board_id,
      board_name,
      jira,
      hide_swimlanes,
      query,
      list_state: ListState::default(),
      swimlane_selected: 0,
      selected_column: 0,
      swimlane_mode: false,
      filter_bar: FilterBar::new(),
      filter_picker: FilterFieldPicker::new(),
      search: SearchInput::new(),
      status_picker: StatusPicker::new(),
      pending_issue_key: None,
      status_mutation: None,
      error_message: None,
    }
  }

  fn data(&self) -> Option<&BoardData> {
    self.query.data()
  }

  fn issues(&self) -> &[IssueSummary] {
    self.data().map(|d| d.issues.as_slice()).unwrap_or(&[])
  }

  fn columns(&self) -> Vec<&BoardColumn> {
    self
      .data()
      .map(|d| {
        d.columns
          .iter()
          .filter(|col| !self.hide_swimlanes.contains(&col.name.to_lowercase()))
          .collect()
      })
      .unwrap_or_default()
  }

  fn is_loading(&self) -> bool {
    self.query.is_loading()
  }

  /// Extract the value of a filter field from an issue
  fn get_field_value(field: FilterField, issue: &IssueSummary) -> Option<String> {
    match field {
      FilterField::None => None,
      FilterField::Assignee => issue.assignee.clone(),
      FilterField::Epic => issue.epic.clone(),
    }
  }

  /// Extract unique values for a filter field from all issues
  fn extract_filter_values(&self, field: FilterField) -> Vec<Option<String>> {
    if matches!(field, FilterField::None) {
      return Vec::new();
    }

    let mut values: BTreeSet<Option<String>> = BTreeSet::new();
    for issue in self.issues() {
      values.insert(Self::get_field_value(field, issue));
    }

    // Convert to Vec, with None (unassigned) first if present
    let mut result: Vec<Option<String>> = Vec::new();
    if values.contains(&None) {
      result.push(None);
    }
    for v in values.into_iter().flatten() {
      result.push(Some(v));
    }
    result
  }

  /// Update filter bar values when data loads
  fn update_filter_values(&mut self) {
    let values = self.extract_filter_values(self.filter_bar.field());
    self.filter_bar.update_values(values);
  }

  /// Get issues filtered by active filter
  fn filtered_issues(&self) -> Vec<&IssueSummary> {
    let issues = self.issues();
    let field = self.filter_bar.field();

    // If no filter active, return all
    let Some(filter_value) = self.filter_bar.selected_value() else {
      return issues.iter().collect();
    };

    issues
      .iter()
      .filter(|issue| {
        let issue_value = Self::get_field_value(field, issue);
        issue_value == *filter_value
      })
      .collect()
  }

  /// Get issues for a specific column (by status)
  fn issues_for_column(&self, column: &BoardColumn) -> Vec<&IssueSummary> {
    self
      .filtered_issues()
      .into_iter()
      .filter(|issue| column.statuses.iter().any(|s| s.id == issue.status_id))
      .collect()
  }

  /// Render list mode
  fn render_list(&mut self, frame: &mut Frame, area: Rect) {
    let len = self.filtered_issues().len();
    ensure_valid_selection(&mut self.list_state, len);

    let title = match self.query.state() {
      QueryState::Loading => format!(" {} (loading...) ", self.board_name),
      QueryState::Error(e) => format!(" {} (error: {}) ", self.board_name, e),
      _ => format!(" {} ({} issues) ", self.board_name, self.issues().len()),
    };

    let block = Block::default()
      .title(title)
      .title_alignment(Alignment::Center)
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Blue));

    if self.issues().is_empty() && !self.is_loading() {
      let content = if self.query.is_error() {
        "Failed to load board. Press 'r' to retry."
      } else {
        "No issues found on this board."
      };
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    // Collect items to avoid borrow conflict
    let items: Vec<ListItem> = self
      .filtered_issues()
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

    let list = List::new(items)
      .block(block)
      .highlight_style(
        Style::default()
          .bg(Color::DarkGray)
          .add_modifier(Modifier::BOLD),
      )
      .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut self.list_state);
  }

  /// Render swimlane (kanban) mode
  fn render_swimlanes(&self, frame: &mut Frame, area: Rect) {
    let columns = self.columns();
    if columns.is_empty() {
      let block = Block::default()
        .title(format!(" {} ", self.board_name))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

      let content = if self.is_loading() {
        "Loading..."
      } else {
        "No columns configured for this board."
      };
      let paragraph = Paragraph::new(content)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
      frame.render_widget(paragraph, area);
      return;
    }

    // Use Layout to distribute columns evenly
    let constraints: Vec<Constraint> = columns
      .iter()
      .map(|_| Constraint::Ratio(1, columns.len() as u32))
      .collect();
    let col_areas = Layout::horizontal(constraints).split(area);

    // Render each column
    for (col_idx, column) in columns.iter().enumerate() {
      let issues = self.issues_for_column(column);
      let is_selected_column = col_idx == self.selected_column;
      let col_area = col_areas[col_idx];

      let border_color = if is_selected_column {
        Color::Yellow
      } else {
        Color::Blue
      };

      let title = format!(" {} ({}) ", truncate(&column.name, 15), issues.len());
      let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

      let items: Vec<ListItem> = issues
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

      let list = List::new(items)
        .block(block)
        .highlight_style(
          Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

      if is_selected_column {
        let mut state = ListState::default();
        state.select(Some(self.swimlane_selected));
        frame.render_stateful_widget(list, col_area, &mut state);
      } else {
        frame.render_widget(list, col_area);
      }
    }
  }

  /// Get the currently selected issue
  fn selected_issue(&self) -> Option<&IssueSummary> {
    if self.swimlane_mode {
      if let Some(column) = self.columns().get(self.selected_column) {
        let issues = self.issues_for_column(column);
        issues.get(self.swimlane_selected).copied()
      } else {
        None
      }
    } else {
      self
        .list_state
        .selected()
        .and_then(|idx| self.filtered_issues().get(idx).copied())
    }
  }

  /// Navigate in list mode (uses ListState)
  fn navigate_list(&mut self, direction: i32) {
    if direction > 0 {
      self.list_state.select_next();
    } else {
      self.list_state.select_previous();
    }
  }

  /// Navigate in swimlane mode
  fn navigate_swimlane(&mut self, direction: i32, horizontal: bool) {
    if horizontal {
      // Move between columns
      let num_columns = self.columns().len();
      if num_columns == 0 {
        return;
      }

      if direction > 0 {
        self.selected_column = (self.selected_column + 1).min(num_columns - 1);
      } else {
        self.selected_column = self.selected_column.checked_sub(1).unwrap_or(0);
      }
      // Reset selection within new column
      self.swimlane_selected = 0;
    } else {
      // Move within column
      if let Some(column) = self.columns().get(self.selected_column) {
        let len = self.issues_for_column(column).len();
        if len == 0 {
          return;
        }

        if direction > 0 {
          self.swimlane_selected = (self.swimlane_selected + 1).min(len - 1);
        } else {
          self.swimlane_selected = self.swimlane_selected.checked_sub(1).unwrap_or(0);
        }
      }
    }
  }

  /// Initiate a status change to the target column
  fn initiate_status_change(&mut self, target_col_idx: usize) {
    // Clear any previous error
    self.error_message = None;

    info!(
      "Initiating status change to column index {}",
      target_col_idx
    );
    let issue = match self.selected_issue() {
      Some(issue) => issue.clone(),
      None => {
        self.error_message = Some("No issue selected".to_string());
        return;
      }
    };

    // Get target column's statuses
    let target_statuses: Vec<StatusInfo> = self
      .columns()
      .get(target_col_idx)
      .map(|col| col.statuses.clone())
      .unwrap_or_default();

    if target_statuses.is_empty() {
      self.error_message = Some("Target column has no statuses".to_string());
      return;
    }

    if target_statuses.len() == 1 {
      // Single status - update directly
      info!(
        "Updating issue {} to status {}",
        issue.key, target_statuses[0].name
      );
      self.update_issue_status(&issue.key, &target_statuses[0].id);
    } else {
      // Multiple statuses - show picker
      self.pending_issue_key = Some(issue.key.clone());
      self
        .status_picker
        .show("Select Status".to_string(), target_statuses);
    }
  }

  /// Update an issue's status directly
  fn update_issue_status(&mut self, issue_key: &str, status_id: &str) {
    let jira = self.jira.clone();
    let key = issue_key.to_string();
    let sid = status_id.to_string();
    let mut query = Query::new(move || {
      let jira = jira.clone();
      let key = key.clone();
      let sid = sid.clone();
      async move {
        jira
          .update_issue_status(&key, &sid)
          .await
          .map_err(|e| e.to_string())
      }
    });
    query.fetch();
    self.status_mutation = Some(query);
  }

  /// Process the result of a status update mutation
  fn process_status_mutation(&mut self) {
    let query = match &self.status_mutation {
      Some(q) => q,
      None => return,
    };

    if query.is_loading() {
      return;
    }

    if let Some(err) = query.error() {
      self.error_message = Some(format!("Status update failed: {}", err));
    } else {
      // Success - refetch board data
      self.query.refetch();
    }

    self.status_mutation = None;
  }

  /// Render error message if present
  fn render_error(&self, frame: &mut Frame, area: Rect) {
    if let Some(msg) = &self.error_message {
      // Calculate dimensions - wider popup for detailed errors
      let max_width = (area.width * 80 / 100).min(70).max(40);
      let inner_width = max_width.saturating_sub(2) as usize;

      // Estimate height needed (rough approximation for wrapped text)
      let line_count = msg.lines().count();
      let char_count = msg.len();
      let estimated_lines = (char_count / inner_width).max(line_count) + 1;
      let height = (estimated_lines as u16 + 2).min(area.height - 4).max(5);

      // Center the popup
      let x = area.x + (area.width.saturating_sub(max_width)) / 2;
      let y = area.y + (area.height.saturating_sub(height)) / 2;

      let error_area = Rect::new(x, y, max_width, height);
      frame.render_widget(ratatui::widgets::Clear, error_area);

      let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" Error - press any key to dismiss ");

      let paragraph = Paragraph::new(msg.as_str())
        .block(block)
        .style(Style::default().fg(Color::Red))
        .wrap(ratatui::widgets::Wrap { trim: false });

      frame.render_widget(paragraph, error_area);
    }
  }

  // Key handling helpers for or_else chain pattern
  fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
    // Filter field picker
    match self.filter_picker.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(FilterFieldPickerEvent::Selected(field)) => {
        let values = self.extract_filter_values(field);
        self.filter_bar.set_field_and_values(field, values);
        return Some(ViewAction::None);
      }
      KeyResult::Event(FilterFieldPickerEvent::Cancelled) => return Some(ViewAction::None),
      KeyResult::NotHandled => {}
    }

    // Filter bar (tab navigation)
    match self.filter_bar.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(FilterBarEvent::SelectionChanged) => {
        // Reset list selection when changing filter
        self.list_state.select(Some(0));
        self.swimlane_selected = 0;
        return Some(ViewAction::None);
      }
      KeyResult::NotHandled => {}
    }

    // Status picker
    match self.status_picker.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(StatusPickerEvent::Selected(status_id)) => {
        if let Some(issue_key) = self.pending_issue_key.take() {
          self.update_issue_status(&issue_key, &status_id);
        }
        return Some(ViewAction::None);
      }
      KeyResult::Event(StatusPickerEvent::Cancelled) => {
        self.pending_issue_key = None;
        return Some(ViewAction::None);
      }
      KeyResult::NotHandled => {}
    }

    // Search
    match self.search.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(SearchEvent::Submitted(_query)) => {
        // TODO: Apply search filter
        return Some(ViewAction::None);
      }
      KeyResult::Event(SearchEvent::Cancelled) => return Some(ViewAction::None),
      KeyResult::NotHandled => {}
    }

    None
  }

  fn handle_navigation(&mut self, key: KeyEvent) -> Option<ViewAction> {
    // Mode-specific navigation
    if self.swimlane_mode {
      match key.code {
        KeyCode::Char('h') | KeyCode::Left => {
          if key.modifiers.contains(KeyModifiers::SHIFT) {
            // Shift+Left: Transition to previous column
            if self.selected_column > 0 {
              self.initiate_status_change(self.selected_column - 1);
            }
          } else {
            self.navigate_swimlane(-1, true);
          }
          Some(ViewAction::None)
        }
        KeyCode::Char('l') | KeyCode::Right => {
          if key.modifiers.contains(KeyModifiers::SHIFT) {
            // Shift+Right: Transition to next column
            let num_columns = self.columns().len();
            if self.selected_column + 1 < num_columns {
              self.initiate_status_change(self.selected_column + 1);
            }
          } else {
            self.navigate_swimlane(1, true);
          }
          Some(ViewAction::None)
        }
        KeyCode::Char('j') | KeyCode::Down => {
          self.navigate_swimlane(1, false);
          Some(ViewAction::None)
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.navigate_swimlane(-1, false);
          Some(ViewAction::None)
        }
        _ => None,
      }
    } else {
      // List mode navigation
      match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
          self.navigate_list(1);
          Some(ViewAction::None)
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.navigate_list(-1);
          Some(ViewAction::None)
        }
        _ => None,
      }
    }
  }

  fn handle_toggles(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match key.code {
      KeyCode::Char('f') => {
        self.filter_picker.show();
        Some(ViewAction::None)
      }
      KeyCode::Char('s') => {
        self.swimlane_mode = !self.swimlane_mode;
        self.list_state.select(Some(0));
        self.swimlane_selected = 0;
        self.selected_column = 0;
        Some(ViewAction::None)
      }
      _ => None,
    }
  }

  fn handle_actions(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match key.code {
      KeyCode::Char('r') => {
        self.query.refetch();
        Some(ViewAction::None)
      }
      KeyCode::Enter => self.selected_issue().map(|issue| {
        ViewAction::Push(Box::new(IssueDetailView::new(
          issue.key.clone(),
          self.jira.clone(),
        )))
      }),
      KeyCode::Char('q') | KeyCode::Esc => Some(ViewAction::Pop),
      _ => None,
    }
  }
}

impl View for BoardView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    // Clear error message on any key press
    self.error_message = None;

    self
      .handle_overlays(key)
      .or_else(|| self.handle_navigation(key))
      .or_else(|| self.handle_toggles(key))
      .or_else(|| self.handle_actions(key))
      .unwrap_or(ViewAction::None)
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    // Split area for filters (if active) and main content
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
    if self.swimlane_mode {
      self.render_swimlanes(frame, content_area);
    } else {
      self.render_list(frame, content_area);
    }

    // Let search component render its overlay
    self.search.render_overlay(frame, area);

    // Render filter field picker if active
    self.filter_picker.render_overlay(frame, area);

    // Render status picker if active
    self.status_picker.render_overlay(frame, area);

    // Render error message if present
    self.render_error(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    if self.swimlane_mode {
      format!("{} [Swimlane]", self.board_name)
    } else {
      self.board_name.clone()
    }
  }

  fn tick(&mut self) {
    let was_loading = self.query.is_loading();
    self.query.poll();

    // Update filter values when data finishes loading
    if was_loading && !self.query.is_loading() && self.query.data().is_some() {
      self.update_filter_values();
    }

    // Poll status mutation if in progress
    if let Some(ref mut query) = self.status_mutation {
      if query.poll() {
        // Mutation completed, process the result
        self.process_status_mutation();
      }
    }
  }

  fn shortcuts(&self) -> Vec<Shortcut> {
    let mut shortcuts = vec![
      Shortcut::new(":", "command"),
      Shortcut::new("/", "search"),
      Shortcut::new("r", "refresh"),
      Shortcut::new("q", "back"),
      Shortcut::new("f", "filter"),
    ];

    // Filter tab navigation shortcuts
    if self.filter_bar.is_active() {
      shortcuts.push(Shortcut::new("PgUp/Dn", "filter tab"));
    }

    // Swimlane shortcuts
    if !self.columns().is_empty() {
      shortcuts.push(Shortcut::new("s", "swimlane"));
      if self.swimlane_mode {
        shortcuts.push(Shortcut::new("S-h/l", "transition"));
      }
    }

    shortcuts
  }
}
