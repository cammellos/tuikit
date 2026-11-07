//! Pieces drawn inside the body: a list scrolled to its cursor, a hint where there's nothing to
//! show, a bar for a fraction, and text fitted to terminal columns (wide CJK characters count
//! double).

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::theme::Colors;

/// Width in terminal columns.
pub fn cols(s: &str) -> usize {
    Span::raw(s).width()
}

/// `left` and `right` on one line of `width` columns, `left` cut with "…" when it doesn't fit.
pub fn fit(left: &str, right: &str, width: usize) -> String {
    let right = format!("{right} ");
    let room = width.saturating_sub(cols(&right));
    if cols(left) < room {
        format!("{left}{}{right}", " ".repeat(room - cols(left)))
    } else {
        let mut cut = String::new();
        for ch in left.chars() {
            if cols(&cut) + cols(ch.encode_utf8(&mut [0; 4])) > room.saturating_sub(2) {
                break;
            }
            cut.push(ch);
        }
        let pad = room.saturating_sub(cols(&cut) + 1);
        format!("{cut}…{}{right}", " ".repeat(pad))
    }
}

/// Words onto lines of at most `width` columns; a word longer than that gets a line of its own.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && cols(&line) + 1 + cols(word) > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// One dim line where the body has nothing to show: "loading…", "nothing here".
pub fn hint(frame: &mut Frame, area: Rect, c: &Colors, text: &str) {
    frame.render_widget(Paragraph::new(format!(" {text}")).style(c.dim()), area);
}

/// A list from last frame's first row, scrolled only as far as `selected` needs, the cursor's row
/// `selected()`; returns the first row drawn, to pass back as `offset` next frame. `selected`
/// None draws it without a cursor.
pub fn list(
    frame: &mut Frame,
    area: Rect,
    c: &Colors,
    items: Vec<ListItem>,
    selected: Option<usize>,
    offset: usize,
) -> usize {
    let list = List::new(items).highlight_style(c.selected());
    let mut state = ListState::default()
        .with_selected(selected)
        .with_offset(offset);
    frame.render_stateful_widget(list, area, &mut state);
    state.offset()
}

/// The item of a [`list`] drawn in `area` from `offset` that the screen cell (`column`, `row`) is
/// on, if any: what a click picked. `len` is how many items it has.
pub fn list_hit(area: Rect, offset: usize, len: usize, column: u16, row: u16) -> Option<usize> {
    if !area.contains((column, row).into()) {
        return None;
    }
    let i = offset + (row - area.y) as usize;
    (i < len).then_some(i)
}

/// A section header row for [`list`].
pub fn header_item<'a>(c: &Colors, name: &str) -> ListItem<'a> {
    ListItem::new(format!(" {name}")).style(c.header())
}

/// A row of a folder tree for [`list`], indented two columns a level: a folder is `▾ name/` open
/// or `▸ name/` closed (`folder` Some(open)), a leaf (None) lines up with the folder names above it.
/// The app keeps which folders are open and flattens the tree into these rows.
pub fn tree_item<'a>(c: &Colors, depth: usize, name: &str, folder: Option<bool>) -> ListItem<'a> {
    let indent = "  ".repeat(depth);
    match folder {
        Some(open) => ListItem::new(format!(" {indent}{} {name}/", if open { "▾" } else { "▸" }))
            .style(c.header()),
        None => ListItem::new(format!(" {indent}  {name}")),
    }
}

/// `fraction` (0 to 1) as a bar `width` cells long, filled `primary` on `panel`.
pub fn bar(c: &Colors, fraction: f64, width: usize) -> Vec<Span<'static>> {
    let filled = ((fraction.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    vec![
        Span::styled(
            "█".repeat(filled),
            ratatui::style::Style::default().fg(c.primary),
        ),
        Span::styled(
            "█".repeat(width - filled),
            ratatui::style::Style::default().fg(c.panel),
        ),
    ]
}

/// A `label  value` row whose label column is `width` columns, as a settings list has them.
pub fn field<'a>(label: &str, value: &str, width: usize) -> Line<'a> {
    let pad = width.saturating_sub(cols(label)).max(1);
    Line::raw(format!(" {label}{}{value}", " ".repeat(pad)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_fills_the_width_and_keeps_the_right_side() {
        for left in [" Legacy combo", " 被丟失的他", &" x".repeat(40)] {
            let line = fit(left, "r/mtg  2h", 40);
            assert_eq!(cols(&line), 40, "{line:?}");
            assert!(line.ends_with("2h "), "{line:?}");
        }
    }

    #[test]
    fn fit_cuts_wide_text_with_an_ellipsis() {
        let line = fit(&"被".repeat(30), "1:00", 20);
        assert!(line.contains('…'), "{line:?}");
        assert!(cols(&line) <= 20, "{line:?}");
    }

    #[test]
    fn wrap_breaks_at_words() {
        assert_eq!(
            wrap("one two three four", 9),
            vec!["one two", "three", "four"]
        );
        assert_eq!(
            wrap("averyveryverylongword x", 5),
            vec!["averyveryverylongword", "x"]
        );
    }

    #[test]
    fn bar_is_always_its_width() {
        let c = Colors::default();
        for f in [-1.0, 0.0, 0.33, 1.0, 2.0] {
            let width: usize = bar(&c, f, 20)
                .iter()
                .map(|s| s.content.chars().count())
                .sum();
            assert_eq!(width, 20, "{f}");
        }
    }

    #[test]
    fn list_hit_maps_a_click_to_an_item() {
        let area = Rect::new(2, 1, 10, 4);
        assert_eq!(list_hit(area, 0, 10, 3, 1), Some(0));
        assert_eq!(list_hit(area, 5, 10, 3, 3), Some(7), "scrolled");
        assert_eq!(list_hit(area, 8, 10, 3, 4), None, "past the end");
        assert_eq!(list_hit(area, 0, 10, 0, 1), None, "outside");
    }

    #[test]
    fn tree_rows_indent_and_mark_folders() {
        let c = Colors::default();
        let text = |i: ListItem| {
            let mut term =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(20, 1)).unwrap();
            term.draw(|f| {
                f.render_widget(List::new(vec![i]), f.area());
            })
            .unwrap();
            let buf = term.backend().buffer().clone();
            (0..20)
                .map(|x| buf[(x, 0)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        };
        assert_eq!(
            text(tree_item(&c, 0, "Personal", Some(true))),
            " ▾ Personal/"
        );
        assert_eq!(
            text(tree_item(&c, 1, "Travel", Some(false))),
            "   ▸ Travel/"
        );
        assert_eq!(text(tree_item(&c, 1, "wifi", None)), "     wifi");
    }

    #[test]
    fn fields_line_up() {
        assert_eq!(field("Tune", "Solar", 8).to_string(), " Tune    Solar");
        assert_eq!(field("Progressions", "3", 8).to_string(), " Progressions 3");
    }
}
