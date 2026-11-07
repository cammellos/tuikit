//! A small menu over the screen, where it was asked for (a right click, a key): a few choices, one
//! highlighted. Arrows (or the app's down/up keys, passed as [`Key::Down`]/[`Key::Up`]) move, `<CR>`
//! picks, `<Esc>` closes; a click on a choice picks it, a click anywhere else closes it.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::keys::Key;
use crate::term::Mouse;
use crate::theme::Colors;
use crate::widgets::cols;

/// What a key or a click did to a [`Menu`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEvent {
    /// Choice `n` was picked.
    Pick(usize),
    /// Closed without a pick.
    Close,
    /// Nothing for the app (the highlight may have moved).
    None,
}

pub struct Menu {
    pub choices: Vec<String>,
    pub selected: usize,
    /// Where it was asked for: its top-left corner, moved in to fit on the screen when drawn.
    pub at: (u16, u16),
    /// Where it was drawn last frame (border included), for clicks.
    area: Rect,
}

impl Menu {
    pub fn new(choices: Vec<String>, at: (u16, u16)) -> Menu {
        Menu {
            choices,
            selected: 0,
            at,
            area: Rect::default(),
        }
    }

    pub fn handle(&mut self, key: Key) -> MenuEvent {
        let last = self.choices.len().saturating_sub(1);
        match key {
            Key::Down | Key::Tab => self.selected = (self.selected + 1).min(last),
            Key::Up | Key::BackTab => self.selected = self.selected.saturating_sub(1),
            Key::Enter if !self.choices.is_empty() => return MenuEvent::Pick(self.selected),
            Key::Esc => return MenuEvent::Close,
            _ => {}
        }
        MenuEvent::None
    }

    pub fn click(&mut self, mouse: Mouse) -> MenuEvent {
        let (Mouse::Click { column, row } | Mouse::RightClick { column, row }) = mouse else {
            return MenuEvent::None;
        };
        let inner = Rect {
            x: self.area.x + 1,
            y: self.area.y + 1,
            width: self.area.width.saturating_sub(2),
            height: self.area.height.saturating_sub(2),
        };
        if inner.contains((column, row).into()) {
            let n = (row - inner.y) as usize;
            if n < self.choices.len() {
                self.selected = n;
                return MenuEvent::Pick(n);
            }
        }
        if self.area.contains((column, row).into()) {
            return MenuEvent::None; // on the border
        }
        MenuEvent::Close
    }

    /// Draw it at `at`, moved in to fit `screen`.
    pub fn draw(&mut self, frame: &mut Frame, screen: Rect, c: &Colors) {
        let width =
            (self.choices.iter().map(|s| cols(s)).max().unwrap_or(0) as u16 + 4).min(screen.width);
        let height = (self.choices.len() as u16 + 2).min(screen.height);
        let x = self.at.0.min(screen.right().saturating_sub(width));
        let y = self.at.1.min(screen.bottom().saturating_sub(height));
        self.area = Rect::new(x, y, width, height);
        frame.render_widget(Clear, self.area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(c.faint))
            .style(Style::default().bg(c.surface).fg(c.text));
        let inner = block.inner(self.area);
        frame.render_widget(block, self.area);
        let lines: Vec<Line> = self
            .choices
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let line = Line::raw(format!(" {s} "));
                if i == self.selected {
                    line.style(c.selected())
                } else {
                    line
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn menu() -> Menu {
        Menu::new(
            vec![
                "copy password".into(),
                "copy username".into(),
                "copy OTP".into(),
            ],
            (5, 2),
        )
    }

    #[test]
    fn keys_move_pick_and_close() {
        let mut m = menu();
        assert_eq!(m.handle(Key::Down), MenuEvent::None);
        assert_eq!(m.handle(Key::Down), MenuEvent::None);
        assert_eq!(m.handle(Key::Down), MenuEvent::None, "stops at the last");
        assert_eq!(m.handle(Key::Enter), MenuEvent::Pick(2));
        assert_eq!(m.handle(Key::Up), MenuEvent::None);
        assert_eq!(m.handle(Key::Enter), MenuEvent::Pick(1));
        assert_eq!(m.handle(Key::Esc), MenuEvent::Close);
    }

    #[test]
    fn clicks_pick_inside_close_outside_and_it_fits_the_screen() {
        let mut term = Terminal::new(TestBackend::new(20, 6)).unwrap();
        let c = Colors::default();
        let mut m = menu();
        m.at = (15, 4); // too far right and down: moved in
        term.draw(|f| m.draw(f, f.area(), &c)).unwrap();
        assert_eq!(m.area, Rect::new(3, 1, 17, 5));
        assert_eq!(
            m.click(Mouse::Click { column: 6, row: 3 }),
            MenuEvent::Pick(1),
            "the second line"
        );
        assert_eq!(
            m.click(Mouse::Click { column: 3, row: 1 }),
            MenuEvent::None,
            "the border"
        );
        assert_eq!(
            m.click(Mouse::Click { column: 0, row: 0 }),
            MenuEvent::Close
        );
        assert_eq!(m.click(Mouse::ScrollDown), MenuEvent::None);
    }
}
