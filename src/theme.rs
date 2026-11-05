//! The palette (Nord-based, the system's) and the styles drawn with it. Apps style by what a thing
//! is (`selected`, `active`, `header`), not by colour, so the same thing looks the same in each:
//!
//! - `active`: the thing in play now: the current tab, a mode badge, the note to play;
//! - `selected`: the cursor's row in a list, quieter than `active`;
//! - `header`: section headers in a list or a page;
//! - `title`: a screen's own title, over its text;
//! - `dim`, `bright`, `faint`: text less or more prominent than the body's.

use ratatui::style::{Color, Modifier, Style};

macro_rules! colors {
    ($($field:ident = $hex:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Colors { $(pub $field: Color),* }

        impl Default for Colors {
            fn default() -> Self {
                Self { $($field: parse_hex($hex).expect("default colour")),* }
            }
        }

        impl Colors {
            /// The names `[colors]` takes, in the order of the docs.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($field)),*];

            /// Set the colour called `name`; false if there's none by that name.
            pub fn set(&mut self, name: &str, color: Color) -> bool {
                match name {
                    $(stringify!($field) => self.$field = color,)*
                    _ => return false,
                }
                true
            }
        }
    };
}

colors! {
    bg = "#2e3440",
    surface = "#3b4252",
    panel = "#434c5e",
    faint = "#4c566a",
    text_dim = "#888e99",
    text_header = "#9aa0aa",
    text = "#aeb4bf",
    text_bright = "#d8dee9",
    primary = "#88c0d0",
    error = "#c07c82",
    success = "#9caa92",
    warning = "#b1a58b",
    accent2 = "#aa92a6",
}

impl Colors {
    /// The whole screen's background and text.
    pub fn base(&self) -> Style {
        Style::default().bg(self.bg).fg(self.text)
    }

    /// The tabline's and statusline's background.
    pub fn bar(&self) -> Style {
        Style::default().bg(self.surface)
    }

    pub fn active(&self) -> Style {
        self.badge(self.primary)
    }

    /// Text on a `color` background, as the statusline's mode and the current tab.
    pub fn badge(&self, color: Color) -> Style {
        Style::default()
            .fg(self.bg)
            .bg(color)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected(&self) -> Style {
        Style::default().bg(self.panel).fg(self.text_bright)
    }

    pub fn header(&self) -> Style {
        Style::default()
            .fg(self.text_header)
            .add_modifier(Modifier::BOLD)
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    pub fn bright(&self) -> Style {
        Style::default().fg(self.text_bright)
    }

    pub fn faint(&self) -> Style {
        Style::default().fg(self.faint)
    }

    /// `color` in bold: a verdict ("Right", "No"), a count that matters.
    pub fn strong(&self, color: Color) -> Style {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }
}

/// "#rrggbb" as a colour.
pub fn parse_hex(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_nord_palette() {
        let c = Colors::default();
        assert_eq!(c.bg, Color::Rgb(0x2e, 0x34, 0x40));
        assert_eq!(c.primary, Color::Rgb(0x88, 0xc0, 0xd0));
        assert_eq!(Colors::NAMES.len(), 13);
    }

    #[test]
    fn set_by_name() {
        let mut c = Colors::default();
        assert!(c.set("primary", Color::Rgb(1, 2, 3)));
        assert_eq!(c.primary, Color::Rgb(1, 2, 3));
        assert!(!c.set("nope", Color::Rgb(1, 2, 3)));
    }

    #[test]
    fn hex() {
        assert_eq!(parse_hex("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_hex("ff0000"), None);
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gggggg"), None);
    }
}
