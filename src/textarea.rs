//! A text of several lines to edit: notes, a small file. The keys of [`crate::input::Input`]
//! (readline's Ctrl keys, the arrows) on the line under the cursor, plus `<CR>` to break a line,
//! `<BS>` at a line's start to join it to the one above, `<Up>`/`<Down>` between lines; `<Esc>` is
//! the app's (done). It scrolls to its cursor, and [`TextArea::take`] hands the text over and wipes
//! its own copy, for text that can hold secrets.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::input::Edit;
use crate::keys::Key;
use crate::theme::Colors;
use crate::widgets::cols;

#[derive(Debug, Clone)]
pub struct TextArea {
    lines: Vec<Vec<char>>,
    row: usize,
    /// In characters, 0..=the line's length.
    col: usize,
    /// The first line and column drawn, kept between frames.
    top: usize,
    left: usize,
}

impl Default for TextArea {
    fn default() -> Self {
        TextArea::new("")
    }
}

impl TextArea {
    pub fn new(text: &str) -> TextArea {
        let lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
        TextArea {
            lines,
            row: 0,
            col: 0,
            top: 0,
            left: 0,
        }
    }

    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The text, leaving it empty and its old characters overwritten.
    pub fn take(&mut self) -> String {
        let text = self.text();
        self.wipe();
        text
    }

    fn wipe(&mut self) {
        for line in self.lines.iter_mut() {
            for c in line.iter_mut() {
                // volatile, so the writes aren't dropped as dead before the clear
                unsafe { std::ptr::write_volatile(c, '\0') };
            }
        }
        self.lines = vec![Vec::new()];
        self.row = 0;
        self.col = 0;
        self.top = 0;
        self.left = 0;
    }

    /// The cursor's line and column (in characters).
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn handle(&mut self, key: Key) -> Edit {
        let len = self.lines[self.row].len();
        match key {
            Key::Esc => return Edit::Cancel,
            Key::Enter => {
                let rest = self.lines[self.row].split_off(self.col);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                return Edit::Changed;
            }
            Key::Char(c) => {
                self.lines[self.row].insert(self.col, c);
                self.col += 1;
                return Edit::Changed;
            }
            Key::Backspace | Key::Ctrl('h') if self.col > 0 => {
                self.col -= 1;
                self.lines[self.row].remove(self.col);
                return Edit::Changed;
            }
            // at a line's start: join it to the one above
            Key::Backspace | Key::Ctrl('h') if self.row > 0 => {
                let line = self.lines.remove(self.row);
                self.row -= 1;
                self.col = self.lines[self.row].len();
                self.lines[self.row].extend(line);
                return Edit::Changed;
            }
            Key::Ctrl('d') if self.col < len => {
                self.lines[self.row].remove(self.col);
                return Edit::Changed;
            }
            Key::Ctrl('u') if self.col > 0 => {
                self.lines[self.row].drain(..self.col);
                self.col = 0;
                return Edit::Changed;
            }
            Key::Ctrl('k') if self.col < len => {
                self.lines[self.row].truncate(self.col);
                return Edit::Changed;
            }
            Key::Tab => {
                for _ in 0..2 {
                    self.lines[self.row].insert(self.col, ' ');
                    self.col += 1;
                }
                return Edit::Changed;
            }
            Key::Left | Key::Ctrl('b') => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                }
            }
            Key::Right | Key::Ctrl('f') => {
                if self.col < len {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            }
            Key::Up | Key::Ctrl('p') if self.row > 0 => {
                self.row -= 1;
                self.col = self.col.min(self.lines[self.row].len());
            }
            Key::Down | Key::Ctrl('n') if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.col = self.col.min(self.lines[self.row].len());
            }
            Key::Ctrl('a') => self.col = 0,
            Key::Ctrl('e') => self.col = len,
            Key::BackTab => return Edit::Ignored,
            _ => {}
        }
        Edit::Moved
    }

    /// Draw it in `area`, scrolled so the cursor shows; the terminal's cursor goes there.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, c: &Colors) {
        let (h, w) = (area.height as usize, area.width as usize);
        if self.row < self.top {
            self.top = self.row;
        } else if h > 0 && self.row >= self.top + h {
            self.top = self.row + 1 - h;
        }
        let at: usize = self.lines[self.row][..self.col]
            .iter()
            .map(|ch| cols(ch.encode_utf8(&mut [0; 4])))
            .sum();
        if at < self.left {
            self.left = at;
        } else if w > 0 && at >= self.left + w {
            self.left = at + 1 - w;
        }
        let shown: Vec<Line> = self
            .lines
            .iter()
            .skip(self.top)
            .take(h)
            .map(|l| {
                let mut x = 0;
                let mut s = String::new();
                for ch in l {
                    let cw = cols(ch.encode_utf8(&mut [0; 4]));
                    if x >= self.left && x + cw <= self.left + w {
                        s.push(*ch);
                    }
                    x += cw;
                }
                Line::raw(s)
            })
            .collect();
        frame.render_widget(Paragraph::new(shown).style(c.base()), area);
        let x = area.x + (at - self.left).min(w.saturating_sub(1)) as u16;
        let y = area.y + (self.row - self.top) as u16;
        frame.set_cursor_position((x, y));
    }
}

impl Drop for TextArea {
    fn drop(&mut self) {
        self.wipe();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(t: &mut TextArea, s: &str) {
        for ch in s.chars() {
            t.handle(if ch == '\n' {
                Key::Enter
            } else {
                Key::Char(ch)
            });
        }
    }

    #[test]
    fn lines_break_join_and_the_cursor_moves_between_them() {
        let mut t = TextArea::default();
        typed(&mut t, "one\ntwo");
        assert_eq!(t.text(), "one\ntwo");
        assert_eq!(t.cursor(), (1, 3));
        t.handle(Key::Ctrl('a'));
        t.handle(Key::Backspace);
        assert_eq!(t.text(), "onetwo", "backspace at the start joins");
        assert_eq!(t.cursor(), (0, 3));
        t.handle(Key::Enter);
        assert_eq!(t.text(), "one\ntwo", "and Enter breaks it again");
        t.handle(Key::Up);
        t.handle(Key::Ctrl('e'));
        assert_eq!(t.cursor(), (0, 3));
        t.handle(Key::Right);
        assert_eq!(t.cursor(), (1, 0), "right off a line's end: the next one");
        assert_eq!(t.handle(Key::Esc), Edit::Cancel);
    }

    #[test]
    fn opened_on_text_the_cursor_is_at_the_top_and_take_empties_it() {
        let mut t = TextArea::new("a\nb\nc");
        assert_eq!(t.cursor(), (0, 0));
        assert_eq!(t.line_count(), 3);
        t.handle(Key::Down);
        t.handle(Key::Down);
        t.handle(Key::Down);
        assert_eq!(t.cursor(), (2, 0), "stops at the last line");
        assert_eq!(t.take(), "a\nb\nc");
        assert_eq!(t.text(), "");
    }

    #[test]
    fn scrolls_to_the_cursor() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut term = Terminal::new(TestBackend::new(6, 2)).unwrap();
        let c = Colors::default();
        let mut t = TextArea::new("l1\nl2\nl3\nlonger line");
        for _ in 0..3 {
            t.handle(Key::Down);
        }
        t.handle(Key::Ctrl('e'));
        term.draw(|f| t.draw(f, f.area(), &c)).unwrap();
        let buf = term.backend().buffer().clone();
        let row = |y: u16| -> String { (0..6).map(|x| buf[(x, y)].symbol().to_string()).collect() };
        assert_eq!(row(0), "      ", "l3, scrolled out to the left");
        assert_eq!(
            row(1),
            " line ",
            "the end of the long line, the cursor after it"
        );
    }
}
