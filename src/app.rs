use crate::config::Config;
use crate::event::{Event, EventHandler, JiraEvent};
use crate::jira::client::JiraClient;
use crate::ui;
use crate::ui::components::{CommandInput, CommandResult};
use crate::ui::view::{View, ViewAction};
use crate::ui::views::{BoardListView, IssueDetailView, IssueListView};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

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
      view_stack: vec![Box::new(IssueListView::new(default_project))],
      command: CommandInput::new(),
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
    // Get project from current view if it's an issue list
    let project = self
      .view_stack
      .first()
      .and_then(|v| v.project())
      .unwrap_or("")
      .to_string();

    if !project.is_empty() {
      let jira = self.jira.clone();
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
        ViewAction::LoadIssue { key } => self.load_issue(&key),
        ViewAction::LoadBoard { id } => self.load_board(id),
        ViewAction::Pop => {
          if self.view_stack.len() > 1 {
            self.view_stack.pop();
          }
        }
        ViewAction::Quit => {
          if self.view_stack.len() > 1 {
            self.view_stack.pop();
          } else {
            self.should_quit = true;
          }
        }
        ViewAction::None => {}
      }
    }
  }

  fn load_issue(&self, key: &str) {
    let jira = self.jira.clone();
    let key = key.to_string();
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

  fn load_board(&self, _id: u64) {
    // TODO: Load board details
  }

  fn execute_command(&mut self, cmd: &str) {
    match cmd {
      "issues" => {
        let project = self.config.default_project.clone().unwrap_or_default();
        self.view_stack = vec![Box::new(IssueListView::new(project))];
        self.load_initial_data();
      }
      "boards" => {
        self.view_stack = vec![Box::new(BoardListView::new())];
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
    match &event {
      JiraEvent::IssueLoaded(issue) => {
        // Push detail view - this is a special case handled by App
        self
          .view_stack
          .push(Box::new(IssueDetailView::new(issue.as_ref().clone())));
        return;
      }
      JiraEvent::Loading => {
        // Set loading on current view
        if let Some(view) = self.view_stack.last_mut() {
          view.set_loading(true);
        }
        return;
      }
      _ => {}
    }

    // Let views handle other events
    // Try the top view first, then root view
    if let Some(view) = self.view_stack.last_mut() {
      if view.receive_data(&event) {
        return;
      }
    }
    if self.view_stack.len() > 1 {
      if let Some(view) = self.view_stack.first_mut() {
        view.receive_data(&event);
      }
    }
  }

  // Accessors for UI rendering
  pub fn current_view(&self) -> Option<&dyn View> {
    self.view_stack.last().map(|v| v.as_ref())
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
}
