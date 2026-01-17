use ratatui::prelude::Color;

/// Truncate a string to a maximum length, adding "..." if truncated
pub fn truncate(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    s.to_string()
  } else {
    format!("{}...", &s[..max_len.saturating_sub(3)])
  }
}

/// Get the display color for a Jira issue status
pub fn status_color(status: &str) -> Color {
  match status {
    "Done" | "Closed" | "Resolved" => Color::Green,
    "In Progress" | "In Review" => Color::Yellow,
    _ => Color::White,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_truncate_short_string() {
    assert_eq!(truncate("hello", 10), "hello");
  }

  #[test]
  fn test_truncate_exact_length() {
    assert_eq!(truncate("hello", 5), "hello");
  }

  #[test]
  fn test_truncate_long_string() {
    assert_eq!(truncate("hello world", 8), "hello...");
  }

  #[test]
  fn test_status_color_done() {
    assert_eq!(status_color("Done"), Color::Green);
    assert_eq!(status_color("Closed"), Color::Green);
    assert_eq!(status_color("Resolved"), Color::Green);
  }

  #[test]
  fn test_status_color_in_progress() {
    assert_eq!(status_color("In Progress"), Color::Yellow);
    assert_eq!(status_color("In Review"), Color::Yellow);
  }

  #[test]
  fn test_status_color_default() {
    assert_eq!(status_color("To Do"), Color::White);
    assert_eq!(status_color("Open"), Color::White);
  }
}
