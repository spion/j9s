use crate::jira::client::JiraClient;
use crate::jira::types::{BoardColumn, BoardConfiguration, IssueSummary, QuickFilter, StatusInfo};
use crate::query::{Query, QueryState};
use crate::ui::components::{SearchInput, SearchResult, StatusPicker, StatusPickerResult};
use crate::ui::ensure_valid_selection;
use crate::ui::renderfns::{status_color, truncate};
use crate::ui::view::{Shortcut, View, ViewAction};
use crate::ui::views::IssueDetailView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use tracing::{debug, info};

/// Combined board data fetched in parallel
#[derive(Clone)]
struct BoardData {
  issues: Vec<IssueSummary>,
  columns: Vec<BoardColumn>,
  quick_filters: Vec<QuickFilter>,
}

/// View for displaying a single board with its issues
pub struct BoardView {
  board_id: u64,
  board_name: String,

  // Jira client for API calls
  jira: JiraClient,

  // Data query
  query: Query<BoardData>,

  // UI state
  list_state: ListState,    // Selection for list mode
  swimlane_selected: usize, // Selection within column for swimlane mode
  selected_column: usize,
  selected_filter: Option<usize>, // Index into quick_filters, None = "All"
  filter_bar_active: bool,        // Whether filter tabs are shown/navigable
  swimlane_mode: bool,

  // Components
  search: SearchInput,
  status_picker: StatusPicker,

  // Status update state
  pending_issue_key: Option<String>, // Issue key waiting for status picker selection
  status_mutation: Option<Query<()>>,
  error_message: Option<String>,
}

impl BoardView {
  pub fn new(board_id: u64, board_name: String, jira: JiraClient) -> Self {
    let jira_for_query = jira.clone();
    let mut query = Query::new(move || {
      let jira = jira_for_query.clone();
      async move {
        // Fetch all board data in parallel
        let (issues_result, config_result, filters_result) = tokio::join!(
          jira.get_board_issues(board_id),
          jira.get_board_configuration(board_id),
          jira.get_board_quick_filters(board_id),
        );

        let issues = issues_result.map_err(|e| e.to_string())?;
        let config = config_result.unwrap_or_else(|_| BoardConfiguration {
          columns: Vec::new(),
        });
        let quick_filters = filters_result.unwrap_or_default();

        Ok(BoardData {
          issues,
          columns: config.columns,
          quick_filters,
        })
      }
    });

    // Start fetching immediately
    query.fetch();

    Self {
      board_id,
      board_name,
      jira,
      query,
      list_state: ListState::default(),
      swimlane_selected: 0,
      selected_column: 0,
      selected_filter: None,
      filter_bar_active: false,
      swimlane_mode: false,
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

  fn columns(&self) -> &[BoardColumn] {
    self.data().map(|d| d.columns.as_slice()).unwrap_or(&[])
  }

  fn quick_filters(&self) -> &[QuickFilter] {
    self
      .data()
      .map(|d| d.quick_filters.as_slice())
      .unwrap_or(&[])
  }

  fn is_loading(&self) -> bool {
    self.query.is_loading()
  }

  /// Get issues filtered by active quick filters and search
  fn filtered_issues(&self) -> Vec<&IssueSummary> {
    self.issues().iter().collect()
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
            format!("{:<12}", issue.key),
            Style::default().fg(Color::Cyan),
          ),
          Span::raw(" "),
          Span::styled(
            format!("{:<12}", truncate(&issue.status, 12)),
            Style::default().fg(color),
          ),
          Span::raw(" "),
          Span::raw(truncate(&issue.summary, 60)),
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

      let title = format!(" {} ({}) ", truncate(&column.name, 10), issues.len());
      let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

      let items: Vec<ListItem> = issues
        .iter()
        .map(|issue| {
          let issue_id = Line::from(vec![Span::styled(
            // truncate(&issue.key, col_area.width.saturating_sub(4) as usize),
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

  /// Render quick filter tabs
  fn render_filters(&self, frame: &mut Frame, area: Rect) {
    let quick_filters = self.quick_filters();
    if quick_filters.is_empty() {
      return;
    }

    let mut spans = Vec::new();

    // "All" tab (when no filter is selected)
    let all_style = if self.selected_filter.is_none() {
      Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
      Style::default().fg(Color::Gray)
    };
    spans.push(Span::styled(" All ", all_style));
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // Individual filter tabs
    for (idx, filter) in quick_filters.iter().enumerate() {
      let is_selected = self.selected_filter == Some(idx);
      let style = if is_selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
      } else {
        Style::default().fg(Color::Gray)
      };
      spans.push(Span::styled(format!(" {} ", filter.name), style));
      if idx < quick_filters.len() - 1 {
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
      }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
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

  /// Navigate filter tabs (left/right)
  fn navigate_filter(&mut self, direction: i32) {
    let quick_filters = self.quick_filters();
    if quick_filters.is_empty() {
      return;
    }

    // Total tabs = "All" + quick_filters
    let total_tabs = quick_filters.len() + 1;

    // Current position: None = 0 (All), Some(idx) = idx + 1
    let current_pos = self.selected_filter.map(|i| i + 1).unwrap_or(0);

    // Calculate new position with wrapping
    let new_pos = if direction > 0 {
      (current_pos + 1) % total_tabs
    } else {
      current_pos.checked_sub(1).unwrap_or(total_tabs - 1)
    };

    // Convert back: 0 = None (All), > 0 = Some(idx - 1)
    self.selected_filter = if new_pos == 0 {
      None
    } else {
      Some(new_pos - 1)
    };

    // Reset list selection when changing filter
    self.list_state.select(Some(0));
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
}

impl View for BoardView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    // Clear error message on any key press
    self.error_message = None;

    // Handle status picker first if active
    if self.status_picker.is_active() {
      match self.status_picker.handle_key(key) {
        StatusPickerResult::Active => return ViewAction::None,
        StatusPickerResult::Selected(status_id) => {
          // Execute status update with selected status
          if let Some(issue_key) = self.pending_issue_key.take() {
            self.update_issue_status(&issue_key, &status_id);
          }
          return ViewAction::None;
        }
        StatusPickerResult::Cancelled => {
          self.pending_issue_key = None;
          return ViewAction::None;
        }
        StatusPickerResult::NotHandled => {}
      }
    }

    // Let search component try to handle first
    match self.search.handle_key(key) {
      SearchResult::Active => return ViewAction::None,
      SearchResult::Submitted(_query) => {
        // TODO: Apply search filter
        return ViewAction::None;
      }
      SearchResult::Cancelled => return ViewAction::None,
      SearchResult::NotHandled => {}
    }

    match key.code {
      // Filter tab navigation (when filter bar active)
      KeyCode::PageUp => {
        if self.filter_bar_active && !self.quick_filters().is_empty() {
          self.navigate_filter(-1);
        }
      }
      KeyCode::PageDown => {
        if self.filter_bar_active && !self.quick_filters().is_empty() {
          self.navigate_filter(1);
        }
      }
      KeyCode::Char('f') => {
        if !self.quick_filters().is_empty() {
          self.filter_bar_active = !self.filter_bar_active;
        }
      }
      KeyCode::Char('s') => {
        self.swimlane_mode = !self.swimlane_mode;
        self.list_state.select(Some(0));
        self.swimlane_selected = 0;
        self.selected_column = 0;
      }
      // Refresh
      KeyCode::Char('r') => {
        self.query.refetch();
      }
      _ => {}
    }

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
        }
        KeyCode::Char('j') | KeyCode::Down => {
          self.navigate_swimlane(1, false);
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.navigate_swimlane(-1, false);
        }

        _ => {}
      }
      ViewAction::None
    } else {
      // Normal mode key handling
      match key.code {
        // Vertical navigation
        KeyCode::Char('j') | KeyCode::Down => {
          self.navigate_list(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
          self.navigate_list(-1);
        }

        // Open issue detail
        KeyCode::Enter => {
          if let Some(issue) = self.selected_issue() {
            return ViewAction::Push(Box::new(IssueDetailView::new(
              issue.key.clone(),
              self.jira.clone(),
            )));
          }
        }

        // Back
        KeyCode::Char('q') | KeyCode::Esc => return ViewAction::Pop,

        _ => {}
      }
      ViewAction::None
    }
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    // Split area for filters (if active) and main content
    let show_filters = self.filter_bar_active && !self.quick_filters().is_empty();
    let (filter_area, content_area) = if show_filters {
      let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
      (Some(chunks[0]), chunks[1])
    } else {
      (None, area)
    };

    // Render quick filters when active
    if let Some(filter_area) = filter_area {
      self.render_filters(frame, filter_area);
    }

    // Render main content
    if self.swimlane_mode {
      self.render_swimlanes(frame, content_area);
    } else {
      self.render_list(frame, content_area);
    }

    // Let search component render its overlay
    self.search.render_overlay(frame, area);

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
    self.query.poll();

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
    ];

    // Filter shortcuts
    if !self.quick_filters().is_empty() {
      shortcuts.push(Shortcut::new("f", "filters"));
      if self.filter_bar_active {
        shortcuts.push(Shortcut::new("h/l", "filter tab"));
      }
    }

    // Swimlane shortcuts
    if !self.columns().is_empty() {
      shortcuts.push(Shortcut::new("s", "swimlane"));
      if self.swimlane_mode {
        shortcuts.push(Shortcut::new("PgUp/Dn", "column"));
        shortcuts.push(Shortcut::new("S-PgUp/Dn", "transition"));
      }
    }

    shortcuts
  }
}
