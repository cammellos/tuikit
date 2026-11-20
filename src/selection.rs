//! Text to select with the mouse, as a terminal selects it but inside a region the app names (a pane), so
//! a drag never takes the pane next to it: from where the left button went down to where it is now, row by
//! row (the first from the press to the region's right edge, the last from its left edge to the pointer),
//! reversed over what was drawn, and copied to the clipboard when the button is let go (OSC 52: the
//! terminal's clipboard, through tmux with `set-clipboard on`).
//!
//! It reads what was drawn, not the app's model: the app names its selectable regions as it draws them
//! ([`Selection::region`]), and [`Selection::draw`] goes over the frame's buffer once everything is drawn,
//! reversing the selected cells and keeping their text (a picture's placeholder cells left out). The mouse
//! goes through [`Selection::mouse`] first: a press in a region isn't taken (a click stays the app's), a
//! drag from it is, and so is the release that ends one. [`components::SelectionLayer`] is the draw step as
//! a `ratatui_kit` component, to put last in the tree.
//!
//! [`components::SelectionLayer`]: crate::components::SelectionLayer

use std::io::Write;
use std::sync::{Arc, Mutex};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// kitty's character for a picture's cells (its unicode placeholders): not text.
const PLACEHOLDER: char = '\u{10EEEE}';

/// Shared between the app's drawing and its mouse handler (cheap to clone).
#[derive(Clone, Default)]
pub struct Selection(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    /// The regions named while this frame is drawn, and the last frame's (what the mouse is matched with).
    next: Vec<Rect>,
    regions: Vec<Rect>,
    drag: Option<Drag>,
    /// The selected text as last drawn.
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    region: Rect,
    from: (u16, u16),
    to: (u16, u16),
    /// The pointer moved with the button down: a selection, not a click.
    moved: bool,
}

/// What a mouse event did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selected {
    /// Not the selection's: the app's to handle (a click, anything outside the regions).
    No,
    /// A selection started or moved: draw again.
    Moving,
    /// One ended: its text, copied.
    Copied(String),
}

impl Selection {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A region whose text can be selected, as the app draws it this frame.
    pub fn region(&self, area: Rect) {
        if !area.is_empty() {
            self.lock().next.push(area);
        }
    }

    /// A mouse event, before the app's own handling (see [`Selected`]). A press anywhere drops the selection
    /// shown before.
    pub fn mouse(&self, m: MouseEvent) -> Selected {
        let at = (m.column, m.row);
        let mut s = self.lock();
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                s.text.clear();
                s.drag = s.regions.iter().find(|r| inside(**r, at)).map(|&region| Drag { region, from: at, to: at, moved: false });
                Selected::No
            }
            MouseEventKind::Down(_) => {
                s.drag = None;
                s.text.clear();
                Selected::No
            }
            MouseEventKind::Drag(MouseButton::Left) => match s.drag.as_mut() {
                Some(d) => {
                    d.to = clamp(d.region, at);
                    d.moved |= d.to != d.from;
                    Selected::Moving
                }
                None => Selected::No,
            },
            MouseEventKind::Up(MouseButton::Left) => match s.drag {
                Some(d) if d.moved => {
                    let text = s.text.clone();
                    if !text.is_empty() {
                        copy(&text);
                    }
                    Selected::Copied(text)
                }
                _ => {
                    s.drag = None;
                    Selected::No
                }
            },
            _ => Selected::No,
        }
    }

    /// Once the frame is drawn: the selection reversed over it, its text kept for the copy; the regions
    /// named this frame become the ones the mouse is matched with.
    pub fn draw(&self, buf: &mut Buffer) {
        let mut s = self.lock();
        s.regions = std::mem::take(&mut s.next);
        let Some(d) = s.drag.filter(|d| d.moved) else {
            s.text.clear();
            return;
        };
        let mut rows = Vec::new();
        for (y, x0, x1) in spans(d) {
            let mut row = String::new();
            for x in x0..=x1 {
                let Some(cell) = buf.cell_mut((x, y)) else { continue };
                if cell.symbol().starts_with(PLACEHOLDER) {
                    continue;
                }
                row.push_str(cell.symbol());
                cell.modifier.insert(Modifier::REVERSED);
            }
            rows.push(row.trim_end().to_string());
        }
        s.text = rows.join("\n").trim_end_matches('\n').to_string();
    }

    /// The selected text as last drawn ("" with none).
    pub fn text(&self) -> String {
        self.lock().text.clone()
    }
}

fn inside(r: Rect, (x, y): (u16, u16)) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn clamp(r: Rect, (x, y): (u16, u16)) -> (u16, u16) {
    (x.clamp(r.x, r.x + r.width - 1), y.clamp(r.y, r.y + r.height - 1))
}

/// Each selected row's cells, (row, first column, last column), top to bottom: a terminal's stream selection
/// inside the region.
fn spans(d: Drag) -> Vec<(u16, u16, u16)> {
    let (a, b) = if (d.from.1, d.from.0) <= (d.to.1, d.to.0) { (d.from, d.to) } else { (d.to, d.from) };
    let (left, right) = (d.region.x, d.region.x + d.region.width - 1);
    (a.1..=b.1)
        .map(|y| {
            let x0 = if y == a.1 { a.0 } else { left };
            let x1 = if y == b.1 { b.0 } else { right };
            (y, x0, x1)
        })
        .collect()
}

/// To the terminal's clipboard: OSC 52 (tmux with `set-clipboard on` passes it on, and keeps it as a buffer).
fn copy(text: &str) {
    let mut out = std::io::stdout();
    let _ = write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()));
    let _ = out.flush();
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::buffer::Buffer;

    fn ev(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }
    }

    /// Two panes side by side: "left" text in 0..10, "right" text in 10..30, 3 rows.
    fn screen() -> Buffer {
        let mut b = Buffer::empty(Rect::new(0, 0, 30, 3));
        for (y, (l, r)) in [("list one", "alpha beta"), ("list two", "gamma"), ("list 3", "delta epsilon")].iter().enumerate() {
            b.set_string(0, y as u16, l, ratatui::style::Style::default());
            b.set_string(10, y as u16, r, ratatui::style::Style::default());
        }
        b
    }

    fn drawn(sel: &Selection) -> Buffer {
        let mut b = screen();
        sel.region(Rect::new(10, 0, 20, 3));
        sel.draw(&mut b);
        b
    }

    #[test]
    fn a_drag_selects_inside_its_region_only_and_its_release_copies() {
        let sel = Selection::default();
        drawn(&sel);
        // a press in the region is still a click (the app's)
        assert_eq!(sel.mouse(ev(MouseEventKind::Down(MouseButton::Left), 16, 0)), Selected::No);
        // dragged to the left pane's side, two rows down: kept inside the region
        assert_eq!(sel.mouse(ev(MouseEventKind::Drag(MouseButton::Left), 2, 2)), Selected::Moving);
        let b = drawn(&sel);
        // the first row from the press, the middle one whole, the last up to the region's left edge (where the
        // pointer was kept)
        assert_eq!(sel.text(), "beta\ngamma\nd");
        assert!(b.cell((16, 0)).unwrap().modifier.contains(Modifier::REVERSED));
        assert!(!b.cell((2, 2)).unwrap().modifier.contains(Modifier::REVERSED), "the left pane isn't touched");
        assert_eq!(sel.mouse(ev(MouseEventKind::Up(MouseButton::Left), 2, 2)), Selected::Copied("beta\ngamma\nd".into()));
        // still shown until the next press, which drops it
        drawn(&sel);
        assert_eq!(sel.text(), "beta\ngamma\nd");
        sel.mouse(ev(MouseEventKind::Down(MouseButton::Left), 0, 0));
        drawn(&sel);
        assert_eq!(sel.text(), "");
    }

    #[test]
    fn a_click_isnt_a_selection_and_outside_the_regions_nothing_is() {
        let sel = Selection::default();
        drawn(&sel);
        sel.mouse(ev(MouseEventKind::Down(MouseButton::Left), 12, 1));
        assert_eq!(sel.mouse(ev(MouseEventKind::Up(MouseButton::Left), 12, 1)), Selected::No);
        // a drag from the left pane (not a region): the app's
        sel.mouse(ev(MouseEventKind::Down(MouseButton::Left), 1, 0));
        assert_eq!(sel.mouse(ev(MouseEventKind::Drag(MouseButton::Left), 15, 1)), Selected::No);
        assert_eq!(sel.mouse(ev(MouseEventKind::Up(MouseButton::Left), 15, 1)), Selected::No);
    }

    #[test]
    fn dragging_up_selects_the_same_way_and_pictures_are_left_out() {
        let sel = Selection::default();
        let mut b = screen();
        b.set_string(20, 1, PLACEHOLDER.to_string(), ratatui::style::Style::default());
        sel.region(Rect::new(10, 0, 20, 3));
        sel.draw(&mut b);
        sel.mouse(ev(MouseEventKind::Down(MouseButton::Left), 14, 2));
        sel.mouse(ev(MouseEventKind::Drag(MouseButton::Left), 10, 1));
        let mut b = screen();
        b.set_string(20, 1, PLACEHOLDER.to_string(), ratatui::style::Style::default());
        sel.region(Rect::new(10, 0, 20, 3));
        sel.draw(&mut b);
        assert_eq!(sel.text(), "gamma\ndelta");
    }

    #[test]
    fn base64_as_osc52_wants_it() {
        assert_eq!(base64(b"hi"), "aGk=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64("è".as_bytes()), "w6g=");
    }
}
