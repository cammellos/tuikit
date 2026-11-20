//! A picture over the whole body (feature `images`): as large as the body lets it be and never larger than it
//! is, so one that fits shows its own pixels; a bigger one is scaled down smoothly (Lanczos), keeping its
//! proportions, and centred. Its name and size in pixels on the border, the keys to close it at the bottom.
//! The app opens one on a click or a key, draws it over its screen while it's open and closes it on the
//! keys it shows (or any click): [`ImageView::new`] takes the picture the app decoded and its ratatui-image
//! [`Picker`] (the terminal's protocol and its cell's size in pixels).

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FilterType, Resize, StatefulImage};

use crate::chrome::Hint;
use crate::popup::popup;
use crate::theme::Colors;

pub struct ImageView {
    title: String,
    /// Its size in pixels.
    size: (u32, u32),
    /// A cell's size in pixels.
    cell: (u16, u16),
    protocol: StatefulProtocol,
}

impl ImageView {
    pub fn new(picker: &Picker, picture: DynamicImage, title: impl Into<String>) -> ImageView {
        let size = (picture.width(), picture.height());
        let font = picker.font_size();
        ImageView { title: title.into(), size, cell: (font.width.max(1), font.height.max(1)), protocol: picker.new_resize_protocol(picture) }
    }

    /// The keys it shows (the app closes it on them).
    pub fn hints() -> Vec<Hint> {
        vec![Hint::new("Esc q", "close")]
    }

    /// Its box over `area` (the body), the picture centred in it.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, c: &Colors) {
        let title = format!("{}  {}×{}", self.title, self.size.0, self.size.1);
        let inner = popup(frame, area, c, &title, &Self::hints());
        let rect = fitted(inner, self.size, self.cell);
        let widget = StatefulImage::default().resize(Resize::Fit(Some(FilterType::Lanczos3)));
        frame.render_stateful_widget(widget, rect, &mut self.protocol);
    }
}

/// The cells a picture of `size` pixels takes in `inner`, centred: its own size when it fits, else scaled down
/// to fit, keeping its proportions.
fn fitted(inner: Rect, size: (u32, u32), cell: (u16, u16)) -> Rect {
    let (w, h) = (f64::from(size.0.max(1)), f64::from(size.1.max(1)));
    let (cw, ch) = (f64::from(cell.0), f64::from(cell.1));
    let room = (f64::from(inner.width) * cw, f64::from(inner.height) * ch);
    let scale = (room.0 / w).min(room.1 / h).min(1.0);
    let cols = ((w * scale / cw).ceil() as u16).clamp(1, inner.width.max(1));
    let rows = ((h * scale / ch).ceil() as u16).clamp(1, inner.height.max(1));
    Rect::new(inner.x + (inner.width.saturating_sub(cols)) / 2, inner.y + (inner.height.saturating_sub(rows)) / 2, cols, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_own_size_when_it_fits_else_scaled_down_and_centred() {
        let inner = Rect::new(1, 1, 100, 40);
        // 260×135 px at 13×27 a cell: 20×5 cells, centred
        assert_eq!(fitted(inner, (260, 135), (13, 27)), Rect::new(41, 18, 20, 5));
        // twice as wide as the room: halved, its proportions kept
        let r = fitted(inner, (2600, 540), (13, 27));
        assert_eq!((r.width, r.height), (100, 10));
        assert_eq!((r.x, r.y), (1, 16));
        // taller than the room: as tall as it, narrower
        let r = fitted(inner, (130, 2160), (13, 27));
        assert_eq!((r.width, r.height), (5, 40));
    }
}
