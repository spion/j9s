use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Draw the header bar with logo, context, and shortcuts
pub fn draw_header(frame: &mut Frame, area: Rect, jira_url: &str, project: &str) {
  // Extract domain from URL
  let domain = extract_domain(jira_url);

  // Build header content
  let header = Line::from(vec![
    Span::styled(" j9s ", Style::default().fg(Color::Cyan).bold()),
    Span::styled("│", Style::default().fg(Color::DarkGray)),
    Span::styled(format!(" {} ", domain), Style::default().fg(Color::White)),
    Span::styled("│", Style::default().fg(Color::DarkGray)),
    Span::styled(
      format!(" {} ", project),
      Style::default().fg(Color::Yellow).bold(),
    ),
    Span::raw("  "),
    // Shortcuts - keys and brackets highlighted, descriptions dimmed
    Span::styled("<:>", Style::default().fg(Color::Cyan)),
    Span::styled(" command", Style::default().fg(Color::DarkGray)),
    Span::raw("   "),
    Span::styled("</>", Style::default().fg(Color::Cyan)),
    Span::styled(" filter", Style::default().fg(Color::DarkGray)),
    Span::raw("   "),
    Span::styled("<q>", Style::default().fg(Color::Cyan)),
    Span::styled(" back", Style::default().fg(Color::DarkGray)),
  ]);

  let paragraph = Paragraph::new(header).style(Style::default().bg(Color::Black));

  frame.render_widget(paragraph, area);
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
