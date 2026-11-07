//! A one-line text input: the search box, a form's field, a prompt. It takes the keys a shell's line
//! editor does (readline's Ctrl keys, the arrows) and draws itself scrolled to its cursor. A secret
//! one draws a dot per character instead of the text, and [`Input::take`] hands the text over and
//! wipes its own copy.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::keys::Key;
use crate::theme::Colors;
use crate::widgets::cols;

/// What a key did to an [`Input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// The text changed.
    Changed,
    /// Only the cursor moved, or the key did nothing.
    Moved,
    /// `<CR>`: done.
    Submit,
    /// `<Esc>`: given up.
    Cancel,
    /// Not an editing key (`<Tab>`, `<Up>`, `<Down>`): the caller's, to move between inputs.
    Ignored,
}

#[derive(Debug, Clone, Default)]
pub struct Input {
    text: Vec<char>,
    /// In characters, 0..=len.
    cursor: usize,
    /// Drawn as dots, never as text.
    pub secret: bool,
    /// The first column drawn, kept between frames so the text scrolls only as the cursor needs.
    scroll: usize,
}

impl Input {
    pub fn new(text: &str) -> Input {
        let text: Vec<char> = text.chars().collect();
        let mut i = Input::default();
        i.cursor = text.len();
        i.text = text;
        i
    }

    pub fn secret() -> Input {
        let mut i = Input::default();
        i.secret = true;
        i
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Replace the text, the cursor at its end.
    pub fn set(&mut self, text: &str) {
        self.wipe();
        self.text = text.chars().collect();
        self.cursor = self.text.len();
    }

    /// The text, leaving the input empty and its old characters overwritten.
    pub fn take(&mut self) -> String {
        let text = self.text();
        self.wipe();
        text
    }

    fn wipe(&mut self) {
        // volatile, so the writes aren't dropped as dead before the clear
        for c in self.text.iter_mut() {
            unsafe { std::ptr::write_volatile(c, '\0') };
        }
        self.text.clear();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn handle(&mut self, key: Key) -> Edit {
        let len = self.text.len();
        match key {
            Key::Enter => return Edit::Submit,
            Key::Esc => return Edit::Cancel,
            Key::Char(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += 1;
                return Edit::Changed;
            }
            Key::Backspace | Key::Ctrl('h') if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.cursor);
                return Edit::Changed;
            }
            Key::Ctrl('d') if self.cursor < len => {
                self.text.remove(self.cursor);
                return Edit::Changed;
            }
            // up to the cursor / from it
            Key::Ctrl('u') if self.cursor > 0 => {
                self.text.drain(..self.cursor);
                self.cursor = 0;
                return Edit::Changed;
            }
            Key::Ctrl('k') if self.cursor < len => {
                self.text.truncate(self.cursor);
                return Edit::Changed;
            }
            // the word before the cursor, and the spaces after it
            Key::Ctrl('w') if self.cursor > 0 => {
                let mut start = self.cursor;
                while start > 0 && self.text[start - 1] == ' ' {
                    start -= 1;
                }
                while start > 0 && self.text[start - 1] != ' ' {
                    start -= 1;
                }
                self.text.drain(start..self.cursor);
                self.cursor = start;
                return Edit::Changed;
            }
            Key::Left | Key::Ctrl('b') => self.cursor = self.cursor.saturating_sub(1),
            Key::Right | Key::Ctrl('f') => self.cursor = (self.cursor + 1).min(len),
            Key::Ctrl('a') => self.cursor = 0,
            Key::Ctrl('e') => self.cursor = len,
            Key::Tab | Key::BackTab | Key::Up | Key::Down => return Edit::Ignored,
            _ => {}
        }
        Edit::Moved
    }

    /// What's drawn for each character: itself, or a dot.
    fn shown(&self) -> Vec<String> {
        self.text
            .iter()
            .map(|c| {
                if self.secret {
                    "•".into()
                } else {
                    c.to_string()
                }
            })
            .collect()
    }

    /// Draw it in `area` (one row), scrolled so the cursor shows; the terminal's cursor goes there
    /// when `focused`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, c: &Colors, focused: bool) {
        let width = area.width as usize;
        let shown = self.shown();
        let col = |i: usize| -> usize { shown[..i].iter().map(|s| cols(s)).sum() };
        let at = col(self.cursor);
        // keep the cursor inside, one column spare at the right for it to sit on
        if at < self.scroll {
            self.scroll = at;
        } else if width > 0 && at >= self.scroll + width {
            self.scroll = at + 1 - width;
        }
        let mut spans = Vec::new();
        let mut x = 0;
        for s in &shown {
            let w = cols(s);
            if x >= self.scroll && x + w <= self.scroll + width {
                spans.push(Span::raw(s.clone()));
            }
            x += w;
        }
        let style = if focused { c.bright() } else { c.base() };
        frame.render_widget(Paragraph::new(Line::from(spans)).style(style), area);
        if focused {
            let x = area.x + (at - self.scroll).min(width.saturating_sub(1)) as u16;
            frame.set_cursor_position((x, area.y));
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if self.secret {
            self.wipe();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[Key]) -> Input {
        let mut i = Input::default();
        for k in keys {
            i.handle(*k);
        }
        i
    }

    fn chars(s: &str) -> Vec<Key> {
        s.chars().map(Key::Char).collect()
    }

    #[test]
    fn types_and_moves_like_a_line_editor() {
        let mut keys = chars("hello world");
        keys.extend([Key::Ctrl('a'), Key::Right, Key::Char('E')]);
        assert_eq!(typed(&keys).text(), "hEello world");

        let mut keys = chars("one two  ");
        keys.push(Key::Ctrl('w'));
        assert_eq!(typed(&keys).text(), "one ");

        let mut keys = chars("abc");
        keys.extend([Key::Left, Key::Ctrl('k')]);
        assert_eq!(typed(&keys).text(), "ab");

        let mut keys = chars("abc");
        keys.extend([Key::Left, Key::Ctrl('u')]);
        assert_eq!(typed(&keys).text(), "c");

        let mut keys = chars("abc");
        keys.extend([Key::Ctrl('a'), Key::Backspace, Key::Ctrl('d')]);
        assert_eq!(
            typed(&keys).text(),
            "bc",
            "backspace at the start does nothing"
        );
    }

    #[test]
    fn says_what_a_key_did() {
        let mut i = Input::new("x");
        assert_eq!(i.handle(Key::Char('y')), Edit::Changed);
        assert_eq!(i.handle(Key::Left), Edit::Moved);
        assert_eq!(i.handle(Key::Tab), Edit::Ignored);
        assert_eq!(i.handle(Key::Enter), Edit::Submit);
        assert_eq!(i.handle(Key::Esc), Edit::Cancel);
    }

    #[test]
    fn take_empties_it() {
        let mut i = Input::secret();
        for k in chars("s3cret") {
            i.handle(k);
        }
        assert_eq!(i.take(), "s3cret");
        assert!(i.is_empty());
        assert_eq!(i.handle(Key::Char('a')), Edit::Changed);
        assert_eq!(i.text(), "a");
    }

    #[test]
    fn a_secret_draws_dots_and_scrolls_to_the_cursor() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut term = Terminal::new(TestBackend::new(5, 1)).unwrap();
        let c = Colors::default();

        let mut i = Input::secret();
        for k in chars("abcdefgh") {
            i.handle(k);
        }
        term.draw(|f| i.draw(f, f.area(), &c, true)).unwrap();
        let row: String = (0..5)
            .map(|x| term.backend().buffer()[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(row, "•••• ", "the last four, and the cursor after them");

        let mut i = Input::new("abcdefgh");
        i.handle(Key::Ctrl('a'));
        term.draw(|f| i.draw(f, f.area(), &c, true)).unwrap();
        let row: String = (0..5)
            .map(|x| term.backend().buffer()[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(row, "abcde", "back at the start");
    }
}
