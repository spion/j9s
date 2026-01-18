use crate::config::Config;
use crate::event::{Event, EventHandler};
use crate::jira::client::JiraClient;
use crate::ui;
use crate::ui::components::{CommandInput, CommandResult};
use crate::ui::view::{Shortcut, View, ViewAction};
use crate::ui::views::{BoardListView, IssueListView};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use std::io::stdout;
use std::time::Duration;

/// Main application state
pub struct App {
  /// Navigation stack - root is always at index 0
  view_stack: Vec<Box<dyn View>>,

  /// Command input component
  command: CommandInput,

  /// Application configuration
  config: Config,

  /// Jira client
  jira: JiraClient,

  /// Whether to quit
  should_quit: bool,
}

impl App {
  pub async fn new(config: Config) -> Result<Self> {
    let jira = JiraClient::new(&config)?;

    let default_project = config.default_project.clone().unwrap_or_default();

    Ok(Self {
      view_stack: vec![Box::new(IssueListView::new(default_project, jira.clone()))],
      command: CommandInput::new(),
      config,
      jira,
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

  fn handle_event(&mut self, event: Event) -> Result<()> {
    match event {
      Event::Key(key) => self.handle_key(key),
      Event::Tick => self.handle_tick(),
    }
    Ok(())
  }

  fn handle_tick(&mut self) {
    // Let all views poll their queries
    for view in &mut self.view_stack {
      view.tick();
    }
  }

  fn handle_key(&mut self, key: KeyEvent) {
    // Let command component try to handle first
    match self.command.handle_key(key) {
      CommandResult::Active => return,
      CommandResult::Submitted(cmd) => {
        self.execute_command(&cmd);
        return;
      }
      CommandResult::Cancelled => return,
      CommandResult::NotHandled => {}
    }

    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
      self.should_quit = true;
      return;
    }

    // Delegate to current view
    if let Some(view) = self.view_stack.last_mut() {
      match view.handle_key(key) {
        ViewAction::Push(new_view) => {
          self.view_stack.push(new_view);
        }
        ViewAction::Pop => {
          if self.view_stack.len() > 1 {
            self.view_stack.pop();
          }
        }
        ViewAction::None => {}
      }
    }
  }

  fn execute_command(&mut self, cmd: &str) {
    match cmd {
      "issues" => {
        let project = self.config.default_project.clone().unwrap_or_default();
        self.view_stack = vec![Box::new(IssueListView::new(project, self.jira.clone()))];
      }
      "boards" => {
        self.view_stack = vec![Box::new(BoardListView::new(self.jira.clone()))];
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
  }

  // Accessors for UI rendering
  pub fn current_view_mut(&mut self) -> Option<&mut dyn View> {
    match self.view_stack.last_mut() {
      Some(v) => Some(&mut **v),
      None => None,
    }
  }

  pub fn jira_url(&self) -> &str {
    &self.config.jira.url
  }

  pub fn current_project(&self) -> &str {
    // Get project from current view or config default
    self
      .view_stack
      .first()
      .and_then(|v| v.project())
      .unwrap_or_else(|| self.config.default_project.as_deref().unwrap_or(""))
  }

  pub fn view_breadcrumb(&self) -> Vec<String> {
    self
      .view_stack
      .iter()
      .map(|v| v.breadcrumb_label())
      .collect()
  }

  /// Render command overlay if active
  pub fn render_command_overlay(&self, frame: &mut Frame, area: Rect) {
    self.command.render_overlay(frame, area);
  }

  /// Get current view's shortcuts
  pub fn current_shortcuts(&self) -> Vec<Shortcut> {
    self
      .view_stack
      .last()
      .map(|v| v.shortcuts())
      .unwrap_or_else(|| {
        vec![
          Shortcut::new(":", "command"),
          Shortcut::new("/", "filter"),
          Shortcut::new("q", "back"),
        ]
      })
  }
}
