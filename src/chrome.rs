//! The four rows around an app's own screen, the same in every app:
//!
//! ```text
//!  Queue  Playlists   Mix › Track          tabline: the tabs (or the app's name), where you are
//!  …                                       body: the app's
//!  PLAYING  Artist – Title    1:02/3:16    statusline: the mode, what's on, counts on the right
//! <Space> pause  > next  q quit         g  command line: a prompt, a message, or the keys to press
//! ```
//!
//! [`frame`] paints the background and hands out the four areas; the app draws its body and calls
//! [`tabline`], [`statusline`] and [`cmdline`] for the rest.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::keys::{pending_label, Key};
use crate::theme::Colors;
use crate::widgets::cols;

pub struct Areas {
    pub tabline: Rect,
    pub body: Rect,
    pub statusline: Rect,
    pub cmdline: Rect,
}

/// Paint the background and split the screen.
pub fn frame(frame: &mut Frame, c: &Colors) -> Areas {
    frame.render_widget(Block::default().style(c.base()), frame.area());
    let [tabline, body, statusline, cmdline] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    Areas {
        tabline,
        body,
        statusline,
        cmdline,
    }
}

/// The tabs, `active` drawn `active()`, then the breadcrumb of where you are in it ("r/horror ›
/// A post"). An app without tabs passes its own name as the one tab. Returns each tab's area, for
/// clicks.
pub fn tabline(
    frame: &mut Frame,
    area: Rect,
    c: &Colors,
    tabs: &[&str],
    active: usize,
    crumbs: &[String],
) -> Vec<Rect> {
    let mut x = area.x;
    let mut rects = Vec::with_capacity(tabs.len());
    let mut spans: Vec<Span> = Vec::new();
    for (i, name) in tabs.iter().enumerate() {
        let label = format!(" {name} ");
        let width = cols(&label) as u16;
        rects.push(Rect {
            x,
            y: area.y,
            width,
            height: 1,
        });
        x += width;
        spans.push(if i == active {
            Span::styled(label, c.active())
        } else {
            Span::styled(label, c.dim().bg(c.surface))
        });
    }
    let crumbs: Vec<&str> = crumbs
        .iter()
        .map(String::as_str)
        .filter(|t| !t.is_empty())
        .collect();
    if !crumbs.is_empty() {
        spans.push(Span::styled(
            format!("  {}", crumbs.join(" › ")),
            ratatui::style::Style::default().fg(c.text_header),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(c.bar()), area);
    rects
}

/// The statusline: `mode` as a badge in `color` (PLAYING, PAUSED, OFFLINE), `left` after it, `right`
/// at the end; spans without a colour of their own are bright on the left, dim on the right. `left`
/// is cut first when they don't fit. Returns the badge's area, for a click on it.
pub fn statusline<'a>(
    frame: &mut Frame,
    area: Rect,
    c: &Colors,
    mode: &str,
    color: Color,
    left: impl Into<Line<'a>>,
    right: impl Into<Line<'a>>,
) -> Rect {
    statusline_with(
        frame,
        area,
        c,
        Span::styled(format!(" {mode} "), c.badge(color)),
        left,
        right,
    )
}

/// [`statusline`] with the badge as the app draws it: a button with its own look.
pub fn statusline_with<'a>(
    frame: &mut Frame,
    area: Rect,
    c: &Colors,
    badge: Span<'a>,
    left: impl Into<Line<'a>>,
    right: impl Into<Line<'a>>,
) -> Rect {
    let badge_width = badge.width() as u16;
    let mut left: Line = left.into();
    let right: Line = right.into();
    let right_width = right.width();
    // the badge, a space, left, at least a space, right, a space
    let room = (area.width as usize).saturating_sub(badge.width() + right_width + 3);
    if left.width() > room {
        left = cut(left, room);
    }
    let pad = (area.width as usize).saturating_sub(badge.width() + left.width() + right_width + 2);
    let mut spans = vec![badge, Span::raw(" ")];
    let (left_style, right_style) = (left.style, right.style);
    spans.extend(
        left.spans
            .into_iter()
            .map(|s| unless_styled(s, left_style, c.bright())),
    );
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(
        right
            .spans
            .into_iter()
            .map(|s| unless_styled(s, right_style, c.dim())),
    );
    spans.push(Span::raw(" "));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(c.bar()), area);
    Rect {
        width: badge_width.min(area.width),
        height: 1,
        ..area
    }
}

/// `s` in its line's style, and `default` where neither gives it a colour.
fn unless_styled(
    s: Span<'_>,
    line: ratatui::style::Style,
    default: ratatui::style::Style,
) -> Span<'_> {
    let style = line.patch(s.style);
    let s = s.style(style);
    if style.fg.is_some() {
        s
    } else {
        s.patch_style(default)
    }
}

/// `line` cut to `width` columns, ending in "…".
fn cut(line: Line, width: usize) -> Line {
    let mut out = Vec::new();
    let mut used = 0;
    for span in line.spans {
        let w = span.width();
        if used + w < width {
            used += w;
            out.push(span);
            continue;
        }
        let mut text = String::new();
        for ch in span.content.chars() {
            let cw = cols(ch.encode_utf8(&mut [0; 4]));
            if used + cw + 1 > width {
                break;
            }
            used += cw;
            text.push(ch);
        }
        if width > 0 {
            text.push('…');
        }
        out.push(Span::styled(text, span.style));
        break;
    }
    Line::from(out)
}

/// A key to press and what it does, for the command line: `<Space>` "replay".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub keys: String,
    pub what: String,
}

impl Hint {
    pub fn new(keys: impl Into<String>, what: impl Into<String>) -> Hint {
        Hint {
            keys: keys.into(),
            what: what.into(),
        }
    }
}

/// What the command line shows.
pub enum Cmdline<'a> {
    /// Typing: `prompt` then `text`, the cursor after it.
    Prompt { prompt: &'a str, text: &'a str },
    /// A message, red when it's an error.
    Message { text: &'a str, error: bool },
    /// Nothing going on: the keys to press, and the keys of an unfinished sequence on the right.
    Idle {
        hints: &'a [Hint],
        pending: &'a [Key],
    },
}

pub fn cmdline(frame: &mut Frame, area: Rect, c: &Colors, what: Cmdline) {
    let line = match what {
        Cmdline::Prompt { prompt, text } => {
            let x = area.x + (cols(prompt) + cols(text)) as u16;
            frame.set_cursor_position((x.min(area.right().saturating_sub(1)), area.y));
            Line::from(vec![
                Span::styled(
                    prompt.to_string(),
                    ratatui::style::Style::default().fg(c.primary),
                ),
                Span::raw(text.to_string()),
            ])
        }
        Cmdline::Message { text, error } => Line::styled(
            text.to_string(),
            ratatui::style::Style::default().fg(if error { c.error } else { c.text }),
        ),
        Cmdline::Idle { hints, pending } => {
            let pending = pending_label(pending);
            let room = (area.width as usize).saturating_sub(cols(&pending) + 2);
            let mut spans = hint_spans(c, hints, room);
            let used: usize = spans.iter().map(Span::width).sum();
            let pad = (area.width as usize).saturating_sub(used + cols(&pending) + 1);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(pending, c.dim()));
            Line::from(spans)
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// The hints that fit in `width` columns, whole: the last ones are left out, not cut.
pub fn hint_spans<'a>(c: &Colors, hints: &[Hint], width: usize) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut used = 0;
    for h in hints {
        let sep = if spans.is_empty() { 1 } else { 2 };
        let w = sep + cols(&h.keys) + 1 + cols(&h.what);
        if used + w > width {
            break;
        }
        used += w;
        spans.push(Span::raw(" ".repeat(sep)));
        spans.push(Span::styled(h.keys.clone(), c.bright()));
        spans.push(Span::styled(format!(" {}", h.what), c.dim()));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn screen(width: u16, height: u16, draw: impl FnOnce(&mut Frame)) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(draw).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_four_rows() {
        let c = Colors::default();
        let rows = screen(40, 5, |f| {
            let a = frame(f, &c);
            assert_eq!(
                (a.tabline.y, a.body.height, a.statusline.y, a.cmdline.y),
                (0, 2, 3, 4)
            );
            tabline(
                f,
                a.tabline,
                &c,
                &["eardrill"],
                0,
                &["Menu".into(), "Notes".into()],
            );
            statusline(f, a.statusline, &c, "MENU", c.primary, "", "streak 3");
            cmdline(
                f,
                a.cmdline,
                &c,
                Cmdline::Idle {
                    hints: &[Hint::new("q", "quit")],
                    pending: &[Key::Char('g')],
                },
            );
        });
        assert_eq!(rows[0].trim_end(), " eardrill   Menu › Notes");
        assert_eq!(rows[3], format!(" MENU {}streak 3 ", " ".repeat(25)));
        assert_eq!(rows[4], format!(" q quit{}g ", " ".repeat(31)));
    }

    #[test]
    fn hints_that_dont_fit_are_left_out_whole() {
        let c = Colors::default();
        let hints = [
            Hint::new("<Space>", "replay"),
            Hint::new("<Tab>", "new key"),
            Hint::new("<Esc>", "menu"),
        ];
        let text = |w| {
            hint_spans(&c, &hints, w)
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(text(80), " <Space> replay  <Tab> new key  <Esc> menu");
        assert_eq!(text(30), " <Space> replay  <Tab> new key");
        assert_eq!(text(5), "");
    }

    #[test]
    fn the_left_keeps_its_own_colour_and_the_badge_is_clickable() {
        let c = Colors::default();
        let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
        let mut badge = Rect::default();
        terminal
            .draw(|f| {
                let title = Line::styled(
                    "Zu – Ostia",
                    ratatui::style::Style::default().fg(c.text_dim),
                );
                badge = statusline(f, f.area(), &c, "▶", c.text_dim, title, "1:00");
            })
            .unwrap();
        assert_eq!(badge, Rect::new(0, 0, 3, 1));
        assert_eq!(terminal.backend().buffer()[(4, 0)].fg, c.text_dim);
    }

    #[test]
    fn a_long_status_is_cut_before_the_right_side() {
        let c = Colors::default();
        let rows = screen(30, 1, |f| {
            statusline(
                f,
                f.area(),
                &c,
                "TUNE",
                c.primary,
                "Autumn Leaves › Guide tones",
                "3/40",
            );
        });
        assert_eq!(rows[0], " TUNE  Autumn Leaves › … 3/40 ");
    }
}
