use crossterm::event::KeyEvent;

#[derive(Debug)]
pub enum Event {
  Key(KeyEvent),
  Tick,
}
