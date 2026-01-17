use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

/// Application events
#[derive(Debug)]
pub enum Event {
  /// Terminal key press
  Key(KeyEvent),
  /// Periodic tick for UI refresh
  Tick,
  /// Result from Jira API call
  Jira(JiraEvent),
  /// Error message to display
  Error(String),
}

/// Events from Jira API operations
#[derive(Debug)]
pub enum JiraEvent {
  /// Issues loaded
  IssuesLoaded(Vec<crate::jira::types::IssueSummary>),
  /// Single issue loaded with details
  IssueLoaded(Box<crate::jira::types::Issue>),
  /// Boards loaded
  BoardsLoaded(Vec<crate::jira::types::Board>),
  /// Board data loaded (issues, configuration, quick filters)
  BoardDataLoaded {
    board_id: u64,
    board_name: String,
    issues: Vec<crate::jira::types::IssueSummary>,
    config: crate::jira::types::BoardConfiguration,
    filters: Vec<crate::jira::types::QuickFilter>,
  },
  /// Loading started (for spinner)
  Loading,
}

/// Event handler that produces events from terminal input and a tick timer
pub struct EventHandler {
  rx: mpsc::UnboundedReceiver<Event>,
  _tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
  /// Create a new event handler with the given tick rate
  pub fn new(tick_rate: Duration) -> Self {
    let (tx, rx) = mpsc::unbounded_channel();
    let event_tx = tx.clone();

    // Spawn terminal event reader
    tokio::spawn(async move {
      loop {
        if event::poll(tick_rate).unwrap_or(false) {
          if let Ok(evt) = event::read() {
            match evt {
              CrosstermEvent::Key(key) => {
                if event_tx.send(Event::Key(key)).is_err() {
                  break;
                }
              }
              _ => {}
            }
          }
        } else {
          // Tick
          if event_tx.send(Event::Tick).is_err() {
            break;
          }
        }
      }
    });

    Self { rx, _tx: tx }
  }

  /// Get a clone of the sender for sending events from async tasks
  pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
    self._tx.clone()
  }

  /// Receive the next event
  pub async fn next(&mut self) -> Option<Event> {
    self.rx.recv().await
  }
}
