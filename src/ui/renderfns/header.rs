use crate::ui::view::{ShortcutInfo, ShortcutVisibility};
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Row, Table};

/// Draw the header bar in k9s style (always 2 lines):
/// - Left: title (line 1), project (line 2)
/// - Middle: shortcuts in columns (2 per column, fill right)
/// - Right: j9s logo
pub fn draw_header(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  project: &str,
  shortcuts: &[ShortcutInfo],
) {
  // Split into 3 columns: left (context), middle (shortcuts), right (logo)
  let columns = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length(30), // Left: hostname/project
      Constraint::Min(30),    // Middle: shortcuts (flexible)
      Constraint::Length(6),  // Right: logo
    ])
    .split(area);

  // === Left column: title and project (always 2 lines) ===
  let left_line1 = Line::from(vec![Span::styled(
    format!(" {}", title),
    Style::default().fg(Color::Cyan),
  )]);

  let left_line2 = Line::from(vec![
    Span::styled(" Project: ", Style::default().fg(Color::DarkGray)),
    Span::styled(project, Style::default().fg(Color::Yellow).bold()),
  ]);

  let left_text = Text::from(vec![left_line1, left_line2]);
  let left_para = Paragraph::new(left_text);
  frame.render_widget(left_para, columns[0]);

  // === Middle column: shortcuts in columns (2 per column, fill right) ===
  let mut visible_shortcuts: Vec<_> = shortcuts
    .iter()
    .filter(|s| s.visibility == ShortcutVisibility::Always)
    .collect();
  visible_shortcuts.sort_by_key(|s| s.priority);

  // Build table with 2 rows, shortcuts filling columns vertically
  let num_cols = (visible_shortcuts.len() + 1) / 2;

  let format_shortcut = |s: &ShortcutInfo| -> Line {
    Line::from(vec![
      Span::styled(format!("<{}>", s.key), Style::default().fg(Color::Cyan)),
      Span::styled(
        format!(" {}", s.label),
        Style::default().fg(Color::DarkGray),
      ),
    ])
  };

  // Calculate width for each column based on longest shortcut in that column
  let mut col_widths: Vec<u16> = Vec::new();
  let mut row1_cells: Vec<Line> = Vec::new();
  let mut row2_cells: Vec<Line> = Vec::new();

  for col in 0..num_cols {
    let idx1 = col * 2;
    let idx2 = col * 2 + 1;

    let s1 = visible_shortcuts.get(idx1);
    let s2 = visible_shortcuts.get(idx2);

    // Calculate column width: max of both shortcuts' display width
    let width1 = s1.map(|s| s.key.len() + s.label.len() + 3).unwrap_or(0); // <key> label
    let width2 = s2.map(|s| s.key.len() + s.label.len() + 3).unwrap_or(0);
    col_widths.push(width1.max(width2) as u16);

    row1_cells.push(s1.map(|s| format_shortcut(s)).unwrap_or_default());
    row2_cells.push(s2.map(|s| format_shortcut(s)).unwrap_or_default());
  }

  let rows = vec![Row::new(row1_cells), Row::new(row2_cells)];
  let widths: Vec<Constraint> = col_widths.iter().map(|&w| Constraint::Length(w)).collect();

  // Add left margin by offsetting the table area
  let table_area = Rect {
    x: columns[1].x + 3,
    y: columns[1].y,
    width: columns[1].width.saturating_sub(3),
    height: columns[1].height,
  };

  let table = Table::new(rows, widths).column_spacing(2);
  frame.render_widget(table, table_area);

  // === Right column: j9s logo ===
  let logo = Paragraph::new(" j9s ")
    .style(Style::default().fg(Color::Cyan).bold())
    .alignment(Alignment::Right);
  frame.render_widget(logo, columns[2]);
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
