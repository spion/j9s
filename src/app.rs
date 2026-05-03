use crate::cache::{CacheLayer, SqliteStorage};
use crate::config::Config;
use crate::db;
use crate::event::Event;
use crate::jira::JiraClient;
use crate::ui;
use crate::ui::components::{CommandEvent, CommandInput, KeyResult};
use crate::ui::view::{ShortcutInfo, View, ViewAction};
use crate::ui::views::{BoardListView, EpicListView, IssueListView};
use color_eyre::Result;
use crossterm::event::{Event as CrosstermEvent, EventStream, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
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

  /// Jira client (with caching)
  jira: JiraClient,

  /// Whether to quit
  should_quit: bool,
}

impl App {
  pub async fn new(config: Config) -> Result<Self> {
    let conn = db::open_connection()?;
    let cache_storage = SqliteStorage::new(conn)?;
    let cache = CacheLayer::new(cache_storage);
    let jira = JiraClient::new(&config, cache)?;
    jira.set_assignee_presets(config.assignees.clone()).await;

    let default_project = config.default_project.clone().unwrap_or_default();

    Ok(Self {
      view_stack: vec![Box::new(IssueListView::new(
        default_project,
        jira.clone(),
        config.default_labels.clone(),
        config.assignees.clone(),
      ))],
      command: CommandInput::new(),
      config,
      jira,
      should_quit: false,
    })
  }

  pub async fn run(&mut self) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    while !self.should_quit {
      terminal.draw(|frame| ui::draw(frame, self))?;

      let event = tokio::select! {
        _ = tick.tick() => Event::Tick,
        maybe = events.next() => match maybe {
          Some(Ok(CrosstermEvent::Key(key))) => Event::Key(key),
          Some(Ok(_)) | Some(Err(_)) => continue,
          None => break,
        },
      };

      if let Some(suspend_fn) = self.handle_event(event) {
        drop(events);
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        suspend_fn();

        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        events = EventStream::new();
        tick = tokio::time::interval(Duration::from_millis(250));
        terminal.clear()?;
        if let Some(view) = self.view_stack.last_mut() {
          view.on_resume();
        }
      }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
  }

  fn handle_event(&mut self, event: Event) -> Option<Box<dyn FnOnce()>> {
    match event {
      Event::Key(key) => self.handle_key(key),
      Event::Tick => {
        self.handle_tick();
        None
      }
    }
  }

  fn handle_tick(&mut self) {
    // Tick non-top views (ignore their actions)
    let last = self.view_stack.len().saturating_sub(1);
    for view in self.view_stack[..last].iter_mut() {
      view.tick();
    }
    // Tick top view and handle its action
    let action = match self.view_stack.last_mut() {
      Some(view) => view.tick(),
      None => ViewAction::None,
    };
    match action {
      ViewAction::Pop => {
        if self.view_stack.len() > 1 {
          self.view_stack.pop();
          if let Some(view) = self.view_stack.last_mut() {
            view.on_resume();
          }
        }
      }
      ViewAction::Push(new_view) => {
        self.view_stack.push(new_view);
      }
      _ => {}
    }
  }

  fn handle_key(&mut self, key: KeyEvent) -> Option<Box<dyn FnOnce()>> {
    // Let command component try to handle first
    match self.command.handle_key(key) {
      KeyResult::Handled => return None,
      KeyResult::Event(CommandEvent::Submitted(cmd)) => {
        self.execute_command(&cmd);
        return None;
      }
      KeyResult::Event(CommandEvent::Cancelled) => return None,
      KeyResult::NotHandled => {}
    }

    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
      self.should_quit = true;
      return None;
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
            if let Some(view) = self.view_stack.last_mut() {
              view.on_resume();
            }
          }
        }
        ViewAction::Suspend(f) => return Some(f),
        ViewAction::None => {}
      }
    }
    None
  }

  fn execute_command(&mut self, cmd: &str) {
    match cmd {
      "issues" => {
        let project = self.config.default_project.clone().unwrap_or_default();
        let labels = self.config.default_labels.clone();
        let assignees = self.config.assignees.clone();
        self.view_stack = vec![Box::new(IssueListView::new(
          project,
          self.jira.clone(),
          labels,
          assignees,
        ))];
      }
      "boards" => {
        let project = self.config.default_project.clone();
        let hide_columns = self.config.boards.hide_columns.clone();
        self.view_stack = vec![Box::new(BoardListView::new(
          project,
          self.jira.clone(),
          hide_columns,
        ))];
      }
      "epics" => {
        let project = self.config.default_project.clone().unwrap_or_default();
        let labels = self.config.default_labels.clone();
        let assignees = self.config.assignees.clone();
        self.view_stack = vec![Box::new(EpicListView::new(
          project,
          self.jira.clone(),
          labels,
          assignees,
        ))];
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

  pub fn title(&self) -> &str {
    self
      .config
      .title
      .as_deref()
      .unwrap_or_else(|| extract_domain(&self.config.jira.url))
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
  pub fn current_shortcuts(&self) -> Vec<ShortcutInfo> {
    self
      .view_stack
      .last()
      .map(|v| v.shortcuts())
      .unwrap_or_else(|| {
        vec![
          ShortcutInfo::new(":", "command").with_priority(10),
          ShortcutInfo::new("/", "search").with_priority(20),
          ShortcutInfo::new("q", "back").with_priority(30),
        ]
      })
  }
}

/// Extract domain from Jira URL
fn extract_domain(url: &str) -> &str {
  url
    .strip_prefix("https://")
    .or_else(|| url.strip_prefix("http://"))
    .unwrap_or(url)
    .split('/')
    .next()
    .unwrap_or(url)
}
