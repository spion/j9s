use crate::commands::{self, Command};
use crate::config::Config;
use crate::event::{Event, EventHandler, JiraEvent};
use crate::jira::client::JiraClient;
use crate::jira::types::{Board, Issue, IssueSummary};
use crate::ui;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

/// Input mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
  Normal,
  Command,
  Search,
}

/// View state - each variant owns its data
#[derive(Debug)]
pub enum ViewState {
  // Root views (set via : commands)
  IssueList {
    issues: Vec<IssueSummary>,
    selected: usize,
    project: String,
    loading: bool,
  },
  BoardList {
    boards: Vec<Board>,
    selected: usize,
    loading: bool,
  },

  // Detail views (pushed via Enter)
  IssueDetail {
    issue: Box<Issue>,
    loading: bool,
  },
}

impl Default for ViewState {
  fn default() -> Self {
    ViewState::IssueList {
      issues: Vec::new(),
      selected: 0,
      project: String::new(),
      loading: true,
    }
  }
}

/// Main application state
pub struct App {
  /// Navigation stack - root is always at index 0
  view_stack: Vec<ViewState>,

  /// Current input mode
  mode: Mode,

  /// Command input buffer (after pressing :)
  command_input: String,

  /// Search filter input (after pressing /)
  search_filter: String,

  /// Selected autocomplete suggestion index
  selected_suggestion: usize,

  /// Application configuration
  config: Config,

  /// Jira client
  jira: JiraClient,

  /// Event sender for async tasks
  event_tx: mpsc::UnboundedSender<Event>,

  /// Whether to quit
  should_quit: bool,
}

impl App {
  pub async fn new(config: Config) -> Result<Self> {
    let jira = JiraClient::new(&config)?;
    let (tx, _rx) = mpsc::unbounded_channel();

    let default_project = config.default_project.clone().unwrap_or_default();

    Ok(Self {
      view_stack: vec![ViewState::IssueList {
        issues: Vec::new(),
        selected: 0,
        project: default_project,
        loading: true,
      }],
      mode: Mode::Normal,
      command_input: String::new(),
      search_filter: String::new(),
      selected_suggestion: 0,
      config,
      jira,
      event_tx: tx,
      should_quit: false,
    })
  }

  pub async fn run(&mut self) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Create event handler
    let mut events = EventHandler::new(Duration::from_millis(250));
    self.event_tx = events.sender();

    // Initial data load
    self.load_initial_data();

    // Main loop
    while !self.should_quit {
      // Draw UI
      terminal.draw(|frame| ui::draw(frame, self))?;

      // Handle events
      if let Some(event) = events.next().await {
        self.handle_event(event)?;
      }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
  }

  fn load_initial_data(&self) {
    if let Some(ViewState::IssueList { project, .. }) = self.view_stack.first() {
      if !project.is_empty() {
        let jira = self.jira.clone();
        let project = project.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
          let _ = tx.send(Event::Jira(JiraEvent::Loading));
          match jira.search_issues(&format!("project = {}", project)).await {
            Ok(issues) => {
              let _ = tx.send(Event::Jira(JiraEvent::IssuesLoaded(issues)));
            }
            Err(e) => {
              let _ = tx.send(Event::Error(e.to_string()));
            }
          }
        });
      }
    }
  }

  fn handle_event(&mut self, event: Event) -> Result<()> {
    match event {
      Event::Key(key) => self.handle_key(key),
      Event::Tick => {} // UI refresh happens automatically
      Event::Jira(jira_event) => self.handle_jira_event(jira_event),
      Event::Error(msg) => {
        // TODO: Display error in status bar
        eprintln!("Error: {}", msg);
      }
    }
    Ok(())
  }

  fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
    match self.mode {
      Mode::Normal => self.handle_normal_mode_key(key),
      Mode::Command => self.handle_command_mode_key(key),
      Mode::Search => self.handle_search_mode_key(key),
    }
  }

  fn handle_normal_mode_key(&mut self, key: crossterm::event::KeyEvent) {
    match key.code {
      // Quit
      KeyCode::Char('q') => {
        if self.view_stack.len() > 1 {
          self.view_stack.pop();
        } else {
          self.should_quit = true;
        }
      }
      KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.should_quit = true;
      }

      // Navigation
      KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
      KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
      KeyCode::Enter => self.enter_selected(),
      KeyCode::Esc => {
        if self.view_stack.len() > 1 {
          self.view_stack.pop();
        }
      }

      // Mode switches
      KeyCode::Char(':') => {
        self.mode = Mode::Command;
        self.command_input.clear();
      }
      KeyCode::Char('/') => {
        self.mode = Mode::Search;
        self.search_filter.clear();
      }

      _ => {}
    }
  }

  fn handle_command_mode_key(&mut self, key: crossterm::event::KeyEvent) {
    match key.code {
      KeyCode::Esc => {
        self.mode = Mode::Normal;
        self.command_input.clear();
        self.selected_suggestion = 0;
      }
      KeyCode::Enter => {
        self.execute_command();
        self.mode = Mode::Normal;
        self.selected_suggestion = 0;
      }
      KeyCode::Tab | KeyCode::Down => {
        // Navigate autocomplete suggestions
        let suggestions = commands::get_suggestions(&self.command_input);
        if !suggestions.is_empty() {
          self.selected_suggestion = (self.selected_suggestion + 1) % suggestions.len();
        }
      }
      KeyCode::BackTab | KeyCode::Up => {
        // Navigate autocomplete suggestions backwards
        let suggestions = commands::get_suggestions(&self.command_input);
        if !suggestions.is_empty() {
          self.selected_suggestion = if self.selected_suggestion == 0 {
            suggestions.len() - 1
          } else {
            self.selected_suggestion - 1
          };
        }
      }
      KeyCode::Backspace => {
        self.command_input.pop();
        self.selected_suggestion = 0; // Reset selection on input change
      }
      KeyCode::Char(c) => {
        self.command_input.push(c);
        self.selected_suggestion = 0; // Reset selection on input change
      }
      _ => {}
    }
  }

  fn handle_search_mode_key(&mut self, key: crossterm::event::KeyEvent) {
    match key.code {
      KeyCode::Esc => {
        self.mode = Mode::Normal;
        self.search_filter.clear();
      }
      KeyCode::Enter => {
        // Apply filter and return to normal mode
        self.mode = Mode::Normal;
      }
      KeyCode::Backspace => {
        self.search_filter.pop();
      }
      KeyCode::Char(c) => {
        self.search_filter.push(c);
      }
      _ => {}
    }
  }

  fn execute_command(&mut self) {
    // Get the command to execute - either from selected suggestion or direct input
    let suggestions = commands::get_suggestions(&self.command_input);
    let cmd = if !suggestions.is_empty() && self.selected_suggestion < suggestions.len() {
      suggestions[self.selected_suggestion].name.to_string()
    } else {
      self.command_input.trim().to_lowercase()
    };

    match cmd.as_str() {
      "issues" => {
        let project = self.config.default_project.clone().unwrap_or_default();
        self.view_stack[0] = ViewState::IssueList {
          issues: Vec::new(),
          selected: 0,
          project: project.clone(),
          loading: true,
        };
        self.view_stack.truncate(1);
        self.load_initial_data();
      }
      "boards" => {
        self.view_stack[0] = ViewState::BoardList {
          boards: Vec::new(),
          selected: 0,
          loading: true,
        };
        self.view_stack.truncate(1);
        self.load_boards();
      }
      "epics" => {
        // TODO: Implement epics view
      }
      "searches" => {
        // TODO: Implement saved searches view
      }
      "quit" => {
        self.should_quit = true;
      }
      _ => {
        // Unknown command
      }
    }
    self.command_input.clear();
  }

  fn load_boards(&self) {
    let jira = self.jira.clone();
    let tx = self.event_tx.clone();

    tokio::spawn(async move {
      let _ = tx.send(Event::Jira(JiraEvent::Loading));
      match jira.get_boards().await {
        Ok(boards) => {
          let _ = tx.send(Event::Jira(JiraEvent::BoardsLoaded(boards)));
        }
        Err(e) => {
          let _ = tx.send(Event::Error(e.to_string()));
        }
      }
    });
  }

  fn handle_jira_event(&mut self, event: JiraEvent) {
    match event {
      JiraEvent::IssuesLoaded(issues) => {
        if let Some(ViewState::IssueList {
          issues: ref mut list,
          loading,
          ..
        }) = self.view_stack.first_mut()
        {
          *list = issues;
          *loading = false;
        }
      }
      JiraEvent::IssueLoaded(issue) => {
        // Push detail view
        self.view_stack.push(ViewState::IssueDetail {
          issue,
          loading: false,
        });
      }
      JiraEvent::BoardsLoaded(boards) => {
        if let Some(ViewState::BoardList {
          boards: ref mut list,
          loading,
          ..
        }) = self.view_stack.first_mut()
        {
          *list = boards;
          *loading = false;
        }
      }
      JiraEvent::Loading => {
        // Update loading state
        if let Some(view) = self.view_stack.last_mut() {
          match view {
            ViewState::IssueList { loading, .. } => *loading = true,
            ViewState::BoardList { loading, .. } => *loading = true,
            ViewState::IssueDetail { loading, .. } => *loading = true,
          }
        }
      }
    }
  }

  fn move_selection(&mut self, delta: i32) {
    if let Some(view) = self.view_stack.last_mut() {
      match view {
        ViewState::IssueList {
          issues, selected, ..
        } => {
          let len = issues.len();
          if len > 0 {
            *selected = (*selected as i32 + delta).rem_euclid(len as i32) as usize;
          }
        }
        ViewState::BoardList {
          boards, selected, ..
        } => {
          let len = boards.len();
          if len > 0 {
            *selected = (*selected as i32 + delta).rem_euclid(len as i32) as usize;
          }
        }
        ViewState::IssueDetail { .. } => {
          // TODO: scroll within detail view
        }
      }
    }
  }

  fn enter_selected(&mut self) {
    if let Some(view) = self.view_stack.last() {
      match view {
        ViewState::IssueList {
          issues, selected, ..
        } => {
          if let Some(issue) = issues.get(*selected) {
            let jira = self.jira.clone();
            let key = issue.key.clone();
            let tx = self.event_tx.clone();

            tokio::spawn(async move {
              let _ = tx.send(Event::Jira(JiraEvent::Loading));
              match jira.get_issue(&key).await {
                Ok(issue) => {
                  let _ = tx.send(Event::Jira(JiraEvent::IssueLoaded(Box::new(issue))));
                }
                Err(e) => {
                  let _ = tx.send(Event::Error(e.to_string()));
                }
              }
            });
          }
        }
        ViewState::BoardList { .. } => {
          // TODO: Open board detail view
        }
        ViewState::IssueDetail { .. } => {
          // TODO: Could open comments, linked issues, etc.
        }
      }
    }
  }

  // Accessors for UI rendering
  pub fn current_view(&self) -> Option<&ViewState> {
    self.view_stack.last()
  }

  pub fn mode(&self) -> &Mode {
    &self.mode
  }

  pub fn command_input(&self) -> &str {
    &self.command_input
  }

  pub fn search_filter(&self) -> &str {
    &self.search_filter
  }

  pub fn jira_url(&self) -> &str {
    &self.config.jira.url
  }

  pub fn current_project(&self) -> &str {
    // Get project from current view or config default
    if let Some(view) = self.view_stack.first() {
      if let ViewState::IssueList { project, .. } = view {
        return project;
      }
    }
    self.config.default_project.as_deref().unwrap_or("")
  }

  pub fn view_breadcrumb(&self) -> Vec<String> {
    self
      .view_stack
      .iter()
      .map(|v| v.breadcrumb_label())
      .collect()
  }

  pub fn autocomplete_suggestions(&self) -> Vec<&'static Command> {
    commands::get_suggestions(&self.command_input)
  }

  pub fn selected_suggestion(&self) -> usize {
    self.selected_suggestion
  }
}

impl ViewState {
  /// Get the label for this view in the breadcrumb
  fn breadcrumb_label(&self) -> String {
    match self {
      ViewState::IssueList { project, .. } => {
        if project.is_empty() {
          "Issues".to_string()
        } else {
          format!("Issues [{}]", project)
        }
      }
      ViewState::BoardList { .. } => "Boards".to_string(),
      ViewState::IssueDetail { issue, .. } => issue.key.clone(),
    }
  }
}
