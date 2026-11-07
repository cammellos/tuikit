//! A box over the body for something that needs an answer before anything else: a confirmation,
//! a summary of what a save will do, a pick from a short list. [`popup`] clears its area, draws the
//! border with a title on top and the keys to press at the bottom, and hands back the inside for
//! the app to draw into; [`lines_height`] sizes one for a list of lines.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear};
use ratatui::Frame;

use crate::chrome::{hint_spans, Hint};
use crate::theme::Colors;

/// A `width` × `height` box (border included) centred in `area`, never bigger than it.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// The rows a popup of `n` lines needs: theirs and the border's.
pub fn lines_height(n: usize) -> u16 {
    n as u16 + 2
}

/// Draw the box at `area` (from [`centered`]): cleared, a rounded border, `title` on it and the
/// `hints` along the bottom. Returns the inside.
pub fn popup(frame: &mut Frame, area: Rect, c: &Colors, title: &str, hints: &[Hint]) -> Rect {
    frame.render_widget(Clear, area);
    let room = (area.width as usize).saturating_sub(4);
    let mut bottom = vec![Span::raw(" ")];
    bottom.extend(hint_spans(c, hints, room));
    bottom.push(Span::raw(" "));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(c.faint))
        .style(Style::default().bg(c.surface).fg(c.text))
        .title(Line::styled(format!(" {title} "), c.title()).centered())
        .title_bottom(Line::from(bottom).centered());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    #[test]
    fn centred_and_clamped() {
        let area = Rect::new(0, 0, 80, 24);
        assert_eq!(centered(area, 40, 10), Rect::new(20, 7, 40, 10));
        assert_eq!(centered(area, 100, 30), area);
        assert_eq!(lines_height(3), 5);
    }

    #[test]
    fn draws_its_title_and_hints_and_gives_the_inside() {
        let mut term = Terminal::new(TestBackend::new(30, 5)).unwrap();
        let c = Colors::default();
        term.draw(|f| {
            let inner = popup(
                f,
                f.area(),
                &c,
                "Save?",
                &[Hint::new("y", "save"), Hint::new("n", "back")],
            );
            assert_eq!(inner, Rect::new(1, 1, 28, 3));
            f.render_widget(Paragraph::new("new  Shopping/ebay"), inner);
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let row =
            |y: u16| -> String { (0..30).map(|x| buf[(x, y)].symbol().to_string()).collect() };
        assert!(row(0).contains(" Save? "), "{}", row(0));
        assert!(row(1).contains("new  Shopping/ebay"), "{}", row(1));
        assert!(row(4).contains("y save"), "{}", row(4));
    }
}
