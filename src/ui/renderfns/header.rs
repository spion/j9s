use crate::ui::view::Shortcut;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Draw the header bar in k9s style (always 2 lines):
/// - Left: hostname (line 1), project (line 2)
/// - Middle: shortcuts (2 lines)
/// - Right: j9s logo
pub fn draw_header(
  frame: &mut Frame,
  area: Rect,
  jira_url: &str,
  project: &str,
  shortcuts: &[Shortcut],
) {
  // Extract domain from URL
  let domain = extract_domain(jira_url);

  // Split into 3 columns: left (context), spacer, middle (shortcuts), right (logo)
  let columns = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length(30), // Left: hostname/project
      Constraint::Length(10), // Spacer
      Constraint::Min(30),    // Middle: shortcuts (flexible)
      Constraint::Length(6),  // Right: logo
    ])
    .split(area);

  // === Left column: hostname and project (always 2 lines) ===
  let left_line1 = Line::from(vec![Span::styled(
    format!(" {}", domain),
    Style::default().fg(Color::Cyan),
  )]);

  let left_line2 = Line::from(vec![
    Span::styled(" Project: ", Style::default().fg(Color::DarkGray)),
    Span::styled(project, Style::default().fg(Color::Yellow).bold()),
  ]);

  let left_text = Text::from(vec![left_line1, left_line2]);
  let left_para = Paragraph::new(left_text);
  frame.render_widget(left_para, columns[0]);

  // === Middle column: shortcuts (2 lines) ===
  // Core shortcuts on line 1, view-specific on line 2
  let core_keys = [":", "/", "q"];
  let (core_shortcuts, view_shortcuts): (Vec<_>, Vec<_>) =
    shortcuts.iter().partition(|s| core_keys.contains(&s.key));

  let mut mid_line1_spans = Vec::new();
  for shortcut in &core_shortcuts {
    mid_line1_spans.push(Span::styled(
      format!("<{}>", shortcut.key),
      Style::default().fg(Color::Cyan),
    ));
    mid_line1_spans.push(Span::styled(
      format!(" {}  ", shortcut.label),
      Style::default().fg(Color::DarkGray),
    ));
  }
  let mid_line1 = Line::from(mid_line1_spans);

  let mut mid_line2_spans = Vec::new();
  for shortcut in &view_shortcuts {
    mid_line2_spans.push(Span::styled(
      format!("<{}>", shortcut.key),
      Style::default().fg(Color::Cyan),
    ));
    mid_line2_spans.push(Span::styled(
      format!(" {}  ", shortcut.label),
      Style::default().fg(Color::DarkGray),
    ));
  }
  let mid_line2 = Line::from(mid_line2_spans);

  let mid_text = Text::from(vec![mid_line1, mid_line2]);
  let mid_para = Paragraph::new(mid_text);
  frame.render_widget(mid_para, columns[2]);

  // === Right column: j9s logo ===
  let logo = Paragraph::new(" j9s ")
    .style(Style::default().fg(Color::Cyan).bold())
    .alignment(Alignment::Right);
  frame.render_widget(logo, columns[3]);
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_domain() {
    assert_eq!(
      extract_domain("https://foo.atlassian.net"),
      "foo.atlassian.net"
    );
    assert_eq!(
      extract_domain("https://jira.company.com/rest"),
      "jira.company.com"
    );
    assert_eq!(extract_domain("http://localhost:8080"), "localhost:8080");
  }
}
