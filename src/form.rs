//! A column of `label  value` rows to fill in, one of them focused: an entry's fields, a settings
//! page. A row is an [`Input`], a note the app writes itself (a masked password, "generated on
//! save"), or an action (a line `<CR>` does something on: "show the notes"). `<Tab>`/`<Down>` and
//! `<Up>` move between the inputs and actions, every other key goes to the focused one; a key it
//! doesn't use (`<S-Tab>`) comes back as [`FormEdit::Other`], for the app (generate a value). The
//! app checks the values as they change and puts what's wrong under the row ([`Form::set_error`]),
//! so a mistake shows while it's being made, not when the form closes.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::input::{Edit, Input};
use crate::keys::Key;
use crate::theme::Colors;
use crate::widgets::cols;

pub enum Value {
    Input(Input),
    /// Shown, not edited: the app's text, dim.
    Note(String),
    /// Focused like an input, not edited: `<CR>` on it is the app's [`FormEdit::Submit`].
    Action(String),
}

pub struct Row {
    pub label: String,
    pub value: Value,
    pub error: Option<String>,
}

/// What a key did to a [`Form`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormEdit {
    /// Row `n`'s text changed.
    Changed(usize),
    /// Focus or cursor moved, or nothing happened.
    Moved,
    /// `<CR>` on row `n`.
    Submit(usize),
    /// `<Esc>`.
    Cancel,
    /// A key the form doesn't use (`<S-Tab>`), on row `n`: the app's.
    Other(Key, usize),
}

#[derive(Default)]
pub struct Form {
    pub rows: Vec<Row>,
    focus: usize,
    /// The first row drawn, kept between frames.
    scroll: usize,
    /// Where each row was drawn last frame (screen row), for clicks.
    drawn: Vec<(usize, u16)>,
}

impl Form {
    pub fn new() -> Form {
        Form::default()
    }

    pub fn input(mut self, label: &str, input: Input) -> Form {
        self.push(label, Value::Input(input));
        self
    }

    pub fn note(mut self, label: &str, text: &str) -> Form {
        self.push(label, Value::Note(text.into()));
        self
    }

    pub fn action(mut self, label: &str, text: &str) -> Form {
        self.push(label, Value::Action(text.into()));
        self
    }

    fn push(&mut self, label: &str, value: Value) {
        self.rows.push(Row {
            label: label.into(),
            value,
            error: None,
        });
        if !self.focusable(self.focus) {
            self.focus = self.rows.len() - 1;
            if !self.focusable(self.focus) {
                self.focus = 0;
            }
        }
    }

    fn focusable(&self, n: usize) -> bool {
        matches!(
            self.rows.get(n).map(|r| &r.value),
            Some(Value::Input(_) | Value::Action(_))
        )
    }

    /// The row the keys go to (an input or an action, if the form has any).
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Focus row `n`, if it's an input or an action.
    pub fn focus_on(&mut self, n: usize) {
        if self.focusable(n) {
            self.focus = n;
        }
    }

    /// The row labelled `label`.
    pub fn row(&self, label: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.label == label)
    }

    /// Row `n`'s input, if it is one.
    pub fn get(&mut self, n: usize) -> Option<&mut Input> {
        match self.rows.get_mut(n).map(|r| &mut r.value) {
            Some(Value::Input(i)) => Some(i),
            _ => None,
        }
    }

    /// The text of the input labelled `label` ("" if there's none).
    pub fn text(&self, label: &str) -> String {
        match self.row(label).map(|n| &self.rows[n].value) {
            Some(Value::Input(i)) => i.text(),
            _ => String::new(),
        }
    }

    /// Row `n`'s text, if it's a note or an action (a checkbox the app redraws).
    pub fn set_note(&mut self, n: usize, text: &str) {
        if let Some(Row {
            value: Value::Note(t) | Value::Action(t),
            ..
        }) = self.rows.get_mut(n)
        {
            *t = text.into();
        }
    }

    pub fn set_error(&mut self, n: usize, error: Option<String>) {
        if let Some(r) = self.rows.get_mut(n) {
            r.error = error;
        }
    }

    pub fn has_errors(&self) -> bool {
        self.rows.iter().any(|r| r.error.is_some())
    }

    /// Move the focus `by` rows (wrapping), skipping notes.
    fn step(&mut self, by: isize) {
        let n = self.rows.len() as isize;
        let mut at = self.focus as isize;
        for _ in 0..n {
            at = (at + by).rem_euclid(n);
            if self.focusable(at as usize) {
                self.focus = at as usize;
                return;
            }
        }
    }

    pub fn handle(&mut self, key: Key) -> FormEdit {
        match key {
            Key::Tab | Key::Down => self.step(1),
            Key::Up => self.step(-1),
            _ => {
                let focus = self.focus;
                if let Some(input) = self.get(focus) {
                    return match input.handle(key) {
                        Edit::Changed => FormEdit::Changed(focus),
                        Edit::Submit => FormEdit::Submit(focus),
                        Edit::Cancel => FormEdit::Cancel,
                        Edit::Ignored => FormEdit::Other(key, focus),
                        Edit::Moved => FormEdit::Moved,
                    };
                }
                return match key {
                    Key::Esc => FormEdit::Cancel,
                    Key::Enter if self.focusable(focus) => FormEdit::Submit(focus),
                    _ => FormEdit::Other(key, focus),
                };
            }
        }
        FormEdit::Moved
    }

    /// The row drawn at screen row `y` last frame, if any (a click on it).
    pub fn row_at(&self, y: u16) -> Option<usize> {
        self.drawn.iter().find(|(_, at)| *at == y).map(|(n, _)| *n)
    }

    /// Draw it in `area`: labels in a column as wide as the longest, each error on a line of its
    /// own under its row, scrolled so the focused row shows. The terminal's cursor goes to the
    /// focused input when `focused`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, c: &Colors, focused: bool) {
        // a space before the labels, two after the longest
        let label_w = self.rows.iter().map(|r| cols(&r.label)).max().unwrap_or(0) + 3;
        // each row's first line, counting the error lines above it
        let mut starts = Vec::with_capacity(self.rows.len());
        let mut height = 0;
        for r in &self.rows {
            starts.push(height);
            height += 1 + r.error.is_some() as usize;
        }
        let rows = area.height as usize;
        let at = starts.get(self.focus).copied().unwrap_or(0);
        let last = at
            + self
                .rows
                .get(self.focus)
                .map_or(0, |r| r.error.is_some() as usize);
        if at < self.scroll {
            self.scroll = at;
        } else if rows > 0 && last >= self.scroll + rows {
            self.scroll = last + 1 - rows;
        }
        let focus = self.focus;
        self.drawn.clear();
        for (n, r) in self.rows.iter_mut().enumerate() {
            let y = starts[n] as isize - self.scroll as isize;
            if y < 0 || y as usize >= rows {
                continue;
            }
            let line = Rect::new(area.x, area.y + y as u16, area.width, 1);
            self.drawn.push((n, line.y));
            let pad = label_w.saturating_sub(cols(&r.label) + 1);
            let label = format!(" {}{}", r.label, " ".repeat(pad));
            let label_style = if n == focus && focused {
                c.active()
            } else {
                c.dim()
            };
            frame.render_widget(Paragraph::new(Span::styled(label, label_style)), line);
            let value_area = Rect {
                x: line.x + label_w as u16,
                width: line.width.saturating_sub(label_w as u16),
                ..line
            };
            match &mut r.value {
                Value::Input(i) => i.draw(frame, value_area, c, focused && n == focus),
                Value::Note(t) => {
                    frame.render_widget(Paragraph::new(t.as_str()).style(c.dim()), value_area)
                }
                Value::Action(t) => {
                    let style = if focused && n == focus {
                        c.selected()
                    } else {
                        c.base()
                    };
                    frame.render_widget(Paragraph::new(t.as_str()).style(style), value_area)
                }
            }
            if let Some(e) = &r.error {
                if (y as usize) + 1 < rows {
                    let under = Rect {
                        y: line.y + 1,
                        ..value_area
                    };
                    frame.render_widget(
                        Paragraph::new(Line::styled(
                            e.as_str(),
                            ratatui::style::Style::default().fg(c.error),
                        )),
                        under,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn entry() -> Form {
        Form::new()
            .input("title", Input::new("Amazon"))
            .input("username", Input::new(""))
            .note("password", "•••••••• (generated on save)")
            .input("generator", Input::new("16 lower"))
    }

    fn screen(form: &mut Form, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let c = Colors::default();
        term.draw(|f| form.draw(f, f.area(), &c, true)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn tab_skips_notes_and_wraps() {
        let mut f = entry();
        assert_eq!(f.focus(), 0);
        f.handle(Key::Tab);
        assert_eq!(f.focus(), 1);
        f.handle(Key::Tab);
        assert_eq!(f.focus(), 3, "past the password note");
        f.handle(Key::Down);
        assert_eq!(f.focus(), 0, "wrapped");
        f.handle(Key::Up);
        assert_eq!(f.focus(), 3);
    }

    #[test]
    fn keys_go_to_the_focused_input() {
        let mut f = entry();
        f.handle(Key::Tab);
        assert_eq!(f.handle(Key::Char('b')), FormEdit::Changed(1));
        assert_eq!(f.text("username"), "b");
        assert_eq!(f.text("title"), "Amazon");
        assert_eq!(f.handle(Key::Enter), FormEdit::Submit(1));
        assert_eq!(f.handle(Key::Esc), FormEdit::Cancel);
    }

    #[test]
    fn actions_take_the_focus_and_s_tab_is_the_apps() {
        let mut f = entry().action("notes", "show them");
        f.focus_on(4);
        assert_eq!(f.focus(), 4, "an action can be focused");
        assert_eq!(f.handle(Key::Enter), FormEdit::Submit(4));
        assert_eq!(f.handle(Key::Char('x')), FormEdit::Other(Key::Char('x'), 4));
        f.handle(Key::Tab);
        assert_eq!(f.focus(), 0, "Tab wraps past it");
        f.handle(Key::Tab);
        assert_eq!(
            f.handle(Key::BackTab),
            FormEdit::Other(Key::BackTab, 1),
            "S-Tab: the app's"
        );
        assert_eq!(f.focus(), 1, "and it doesn't move");
    }

    #[test]
    fn row_at_finds_the_clicked_row() {
        let mut f = entry();
        screen(&mut f, 40, 6);
        assert_eq!(f.row_at(0), Some(0));
        assert_eq!(f.row_at(3), Some(3));
        assert_eq!(f.row_at(5), None);
    }

    #[test]
    fn a_form_of_notes_only_still_cancels() {
        let mut f = Form::new().note("a", "x");
        assert_eq!(f.handle(Key::Esc), FormEdit::Cancel);
        assert_eq!(
            f.handle(Key::Char('q')),
            FormEdit::Other(Key::Char('q'), 0),
            "the app's (its own q)"
        );
    }

    #[test]
    fn labels_line_up_and_errors_go_under_their_row() {
        let mut f = entry();
        let n = f.row("generator").unwrap();
        f.set_error(n, Some("needs a length".into()));
        assert!(f.has_errors());
        assert_eq!(
            screen(&mut f, 40, 6),
            vec![
                " title      Amazon",
                " username",
                " password   •••••••• (generated on save)",
                " generator  16 lower",
                "            needs a length",
                "",
            ]
        );
    }

    #[test]
    fn scrolls_to_the_focused_row() {
        let mut f = entry();
        f.focus_on(3);
        assert_eq!(
            screen(&mut f, 30, 2),
            vec![" password   •••••••• (generate", " generator  16 lower"]
        );
    }
}
