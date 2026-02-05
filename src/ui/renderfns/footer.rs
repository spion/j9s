use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Draw the footer bar with view breadcrumb
pub fn draw_footer(frame: &mut Frame, area: Rect, breadcrumb: &[String]) {
  let mut spans = Vec::new();

  spans.push(Span::raw(" "));

  for (i, part) in breadcrumb.iter().enumerate() {
    if i > 0 {
      spans.push(" > ".dark_gray());
    }

    let style = if i == breadcrumb.len() - 1 {
      Style::new().cyan().bold()
    } else {
      Style::new().white()
    };

    spans.push(Span::styled(part.clone(), style));
  }

  let line = Line::from(spans);
  let paragraph = Paragraph::new(line).style(Style::new().on_black());

  frame.render_widget(paragraph, area);
}
