use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

/// Application events
#[derive(Debug)]
pub enum Event {
  /// Terminal key press
  Key(KeyEvent),
  /// Periodic tick for UI refresh and query polling
  Tick,
}

/// Event handler that produces events from terminal input and a tick timer
pub struct EventHandler {
  rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
  /// Create a new event handler with the given tick rate
  pub fn new(tick_rate: Duration) -> Self {
    let (tx, rx) = mpsc::unbounded_channel();

    // Spawn terminal event reader
    tokio::spawn(async move {
      loop {
        if event::poll(tick_rate).unwrap_or(false) {
          if let Ok(evt) = event::read() {
            match evt {
              CrosstermEvent::Key(key) => {
                if tx.send(Event::Key(key)).is_err() {
                  break;
                }
              }
              _ => {}
            }
          }
        } else {
          // Tick
          if tx.send(Event::Tick).is_err() {
            break;
          }
        }
      }
    });

    Self { rx }
  }

  /// Receive the next event
  pub async fn next(&mut self) -> Option<Event> {
    self.rx.recv().await
  }
}
