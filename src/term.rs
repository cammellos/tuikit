//! The terminal around the app: the mouse, on while a guard lives, and its events as the apps use
//! them; kitty's keyboard protocol, on while a guard lives, for the Mod (`<D-x>`) keys.

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, MouseButton, MouseEvent, MouseEventKind,
};

/// The mouse events the apps use (`mouse = true` in their config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    Click { column: u16, row: u16 },
    ScrollUp,
    ScrollDown,
}

impl Mouse {
    pub fn from_event(m: MouseEvent) -> Option<Mouse> {
        Some(match m.kind {
            MouseEventKind::Down(MouseButton::Left) => Mouse::Click {
                column: m.column,
                row: m.row,
            },
            MouseEventKind::ScrollUp => Mouse::ScrollUp,
            MouseEventKind::ScrollDown => Mouse::ScrollDown,
            _ => return None,
        })
    }
}

/// Mouse reporting on while it lives. Dropping it turns reporting off, also on an error or a
/// panic, or the shell would get the mouse's escape codes.
pub struct MouseCapture;

impl MouseCapture {
    pub fn enable() -> std::io::Result<Self> {
        crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    }
}

/// Kitty's keyboard protocol on while it lives (disambiguated escape codes): what makes the terminal
/// report Mod (Super) with a key, which [`crate::keys::Key::Super`] reads. A terminal without it
/// ignores the request, and Mod keys just don't arrive. Dropping it pops the flags again.
pub struct KeyboardProtocol;

impl KeyboardProtocol {
    pub fn enable() -> std::io::Result<Self> {
        use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
        crossterm::execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        Ok(Self)
    }
}

impl Drop for KeyboardProtocol {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::PopKeyboardEnhancementFlags
        );
    }
}
