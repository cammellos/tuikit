//! The look and the inputs as [`ratatui_kit`] components, for apps built on it:
//!
//! - [`Theme`] goes at the root: it paints the background and gives every component under it the
//!   Nord colours, and [`Colors`] as a context;
//! - [`Chrome`] is the four rows every app has, its children the body;
//! - [`InputField`] is the [`Input`] as a component, a secret one included.
//!
//! A component inside a closed `Modal` stays alive, so a secret [`InputField`] in a modal goes under
//! an `if`: only taking it out of the tree drops it, and dropping it is what wipes it.

use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyEventKind};
use ratatui::layout::{Constraint, Direction};
use ratatui::style::Color;
use ratatui::text::Line;
use ratatui::widgets::{Block, Widget};
use ratatui_kit::layout_style::LayoutStyle;
use ratatui_kit::prelude::*;
use ratatui_kit::Element;
use zeroize::Zeroizing;

use crate::chrome::{self, Cmdline, Hint};
use crate::input::{Edit, Input};
use crate::keys::Key;
use crate::theme::Colors;

impl Colors {
    /// These colours under the names `ratatui_kit` draws with.
    pub fn palette(&self) -> Palette {
        let mut p = Palette::default();
        p.fg = self.text;
        p.fg_dim = self.text_dim;
        p.bg = self.bg;
        p.surface = self.surface;
        p.overlay = self.panel;
        p.accent = self.primary;
        p.on_accent = self.bg;
        p.selection = self.panel;
        p.border = self.faint;
        p.border_active = self.primary;
        p.success = self.success;
        p.warning = self.warning;
        p.error = self.error;
        p.info = self.primary;
        p.placeholder = self.text_dim;
        p
    }
}

/// `children` under an override of the theme `T`.
fn overriding<'a, T: ComponentTheme>(
    theme: T,
    children: Vec<AnyElement<'a>>,
) -> Vec<AnyElement<'a>> {
    vec![Element::<ThemeOverride<T>> {
        key: ElementKey::user("tuikit-theme"),
        props: ThemeOverrideProps { children, theme },
    }
    .into()]
}

#[derive(Default, Props)]
pub struct ThemeProps<'a> {
    pub children: Vec<AnyElement<'a>>,
    pub colors: Colors,
}

/// The root of an app's tree.
pub struct Theme {
    colors: Colors,
}

impl Component for Theme {
    type Props<'a> = ThemeProps<'a>;

    fn new(props: &Self::Props<'_>) -> Self {
        Self {
            colors: props.colors,
        }
    }

    fn update(
        &mut self,
        props: &mut Self::Props<'_>,
        _hooks: Hooks,
        updater: &mut ComponentUpdater,
    ) {
        let c = props.colors;
        self.colors = c;
        let palette = c.palette();

        // ratatui_kit puts `on_accent` on `selection` too; here that's dark text on a dark row, and
        // a selected row is quieter than an accent anyway: it takes the same style as everywhere
        let mut select = SelectTheme::from_palette(&palette);
        select.highlight_style = c.selected();
        let mut multi = MultiSelectTheme::from_palette(&palette);
        multi.highlight_style = c.selected();
        let mut tree = TreeSelectTheme::from_palette(&palette);
        tree.highlight_style = c.selected();
        let mut table = TableTheme::from_palette(&palette);
        table.highlight_style = c.selected();

        let children = std::mem::take(&mut props.children);
        let children = overriding(select, children);
        let children = overriding(multi, children);
        let children = overriding(tree, children);
        let children = overriding(table, children);
        let mut root = Element::<PaletteProvider> {
            key: ElementKey::user("tuikit-palette"),
            props: PaletteProviderProps { children, palette },
        };
        updater.set_transparent_layout(true);
        updater.update_children([&mut root], Some(Context::owned(c)));
    }

    fn draw(&mut self, drawer: &mut ComponentDrawer<'_, '_>) {
        let area = drawer.frame.area();
        Block::default()
            .style(self.colors.base())
            .render(area, drawer.buffer_mut());
    }
}

#[derive(Default, Props)]
pub struct InputFieldProps {
    /// Drawn as dots, wiped when taken, cancelled or dropped.
    pub secret: bool,
    /// Takes the keys and shows the cursor.
    pub focused: bool,
    /// What it starts with, when it isn't secret.
    pub text: String,
    /// `<CR>`: the text, leaving the field empty.
    pub on_submit: Handler<'static, Zeroizing<String>>,
    /// `<Esc>`.
    pub on_cancel: Handler<'static, ()>,
}

/// What a key did to the field.
#[derive(Debug, PartialEq, Eq)]
enum FieldEvent {
    Edited,
    Submit(Zeroizing<String>),
    Cancel,
    /// A key the field doesn't use (`<Tab>`): the next handler's.
    Unused,
}

fn on_key(input: &mut Input, key: Key) -> FieldEvent {
    match input.handle(key) {
        Edit::Changed | Edit::Moved => FieldEvent::Edited,
        Edit::Submit => FieldEvent::Submit(Zeroizing::new(input.take())),
        Edit::Cancel => {
            if input.secret {
                drop(Zeroizing::new(input.take()));
            }
            FieldEvent::Cancel
        }
        Edit::Ignored => FieldEvent::Unused,
    }
}

/// An [`Input`] in the tree. The text lives in the component, never in a `State`, whose copies
/// nothing would wipe.
pub struct InputField {
    input: Rc<RefCell<Input>>,
    colors: Colors,
    focused: bool,
}

impl Component for InputField {
    type Props<'a> = InputFieldProps;

    fn new(props: &Self::Props<'_>) -> Self {
        let input = if props.secret {
            Input::secret()
        } else {
            Input::new(&props.text)
        };
        Self {
            input: Rc::new(RefCell::new(input)),
            colors: Colors::default(),
            focused: props.focused,
        }
    }

    fn update(
        &mut self,
        props: &mut Self::Props<'_>,
        mut hooks: Hooks,
        updater: &mut ComponentUpdater,
    ) {
        self.focused = props.focused;
        if let Some(c) = updater.get_context::<Colors>() {
            self.colors = *c;
        }
        let focused = props.focused;
        let input = Rc::clone(&self.input);
        let mut on_submit = props.on_submit.take();
        let mut on_cancel = props.on_cancel.take();
        let mut hooks = hooks.with_context_stack(updater.component_context_stack());
        hooks.use_event_handler(EventScope::Current, EventPriority::Normal, move |event| {
            let Event::Key(k) = event else {
                return EventResult::Ignored;
            };
            if !focused || k.kind != KeyEventKind::Press {
                return EventResult::Ignored;
            }
            let Some(key) = Key::from_event(k) else {
                return EventResult::Ignored;
            };
            let happened = on_key(&mut input.borrow_mut(), key);
            match happened {
                FieldEvent::Edited => EventResult::Consumed,
                FieldEvent::Submit(text) => {
                    on_submit(text);
                    EventResult::Consumed
                }
                FieldEvent::Cancel => {
                    on_cancel(());
                    EventResult::Consumed
                }
                FieldEvent::Unused => EventResult::Ignored,
            }
        });
    }

    fn draw(&mut self, drawer: &mut ComponentDrawer<'_, '_>) {
        let area = drawer.area;
        // a closed modal still draws its children, in an empty area
        if area.is_empty() {
            return;
        }
        self.input
            .borrow_mut()
            .draw(drawer.frame, area, &self.colors, self.focused);
    }
}

/// What the command line shows: a [`Cmdline`] that owns its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandLine {
    Prompt { prompt: String, text: String },
    Message { text: String, error: bool },
    Idle { hints: Vec<Hint>, pending: Vec<Key> },
}

impl Default for CommandLine {
    fn default() -> Self {
        CommandLine::Idle {
            hints: Vec::new(),
            pending: Vec::new(),
        }
    }
}

#[derive(Default, Props)]
pub struct ChromeProps<'a> {
    /// The body.
    pub children: Vec<AnyElement<'a>>,
    /// The tabs, or the app's name as the one tab.
    pub tabs: Vec<String>,
    pub active: usize,
    /// Where you are in the tab.
    pub crumbs: Vec<String>,
    /// The statusline's badge.
    pub mode: String,
    /// The badge's colour: the primary one if not given.
    pub mode_color: Option<Color>,
    pub left: Line<'static>,
    pub right: Line<'static>,
    pub command: CommandLine,
}

#[derive(Default)]
enum Row {
    #[default]
    Empty,
    Tabs {
        tabs: Vec<String>,
        active: usize,
        crumbs: Vec<String>,
    },
    Status {
        mode: String,
        color: Option<Color>,
        left: Line<'static>,
        right: Line<'static>,
    },
    Command(CommandLine),
}

#[derive(Default, Props)]
struct BarProps {
    row: Row,
}

/// One of the rows around the body, drawn by [`crate::chrome`].
struct Bar {
    row: Row,
    colors: Colors,
}

impl Component for Bar {
    type Props<'a> = BarProps;

    fn new(_props: &Self::Props<'_>) -> Self {
        Self {
            row: Row::Empty,
            colors: Colors::default(),
        }
    }

    fn update(
        &mut self,
        props: &mut Self::Props<'_>,
        _hooks: Hooks,
        updater: &mut ComponentUpdater,
    ) {
        self.row = std::mem::take(&mut props.row);
        if let Some(c) = updater.get_context::<Colors>() {
            self.colors = *c;
        }
        updater.set_layout_style(LayoutStyle {
            height: Constraint::Length(1),
            ..Default::default()
        });
    }

    fn draw(&mut self, drawer: &mut ComponentDrawer<'_, '_>) {
        let area = drawer.area;
        let c = &self.colors;
        match &self.row {
            Row::Empty => {}
            Row::Tabs {
                tabs,
                active,
                crumbs,
            } => {
                let tabs: Vec<&str> = tabs.iter().map(String::as_str).collect();
                chrome::tabline(drawer.frame, area, c, &tabs, *active, crumbs);
            }
            Row::Status {
                mode,
                color,
                left,
                right,
            } => {
                let color = color.unwrap_or(c.primary);
                chrome::statusline(
                    drawer.frame,
                    area,
                    c,
                    mode,
                    color,
                    left.clone(),
                    right.clone(),
                );
            }
            Row::Command(command) => {
                let what = match command {
                    CommandLine::Prompt { prompt, text } => Cmdline::Prompt { prompt, text },
                    CommandLine::Message { text, error } => Cmdline::Message {
                        text,
                        error: *error,
                    },
                    CommandLine::Idle { hints, pending } => Cmdline::Idle { hints, pending },
                };
                chrome::cmdline(drawer.frame, area, c, what);
            }
        }
    }
}

fn bar<'a>(key: &'static str, row: Row) -> AnyElement<'a> {
    Element::<Bar> {
        key: ElementKey::user(key),
        props: BarProps { row },
    }
    .into()
}

/// The four rows every app has: tabline, body, statusline, command line, always all four.
pub struct Chrome;

impl Component for Chrome {
    type Props<'a> = ChromeProps<'a>;

    fn new(_props: &Self::Props<'_>) -> Self {
        Self
    }

    fn update(
        &mut self,
        props: &mut Self::Props<'_>,
        _hooks: Hooks,
        updater: &mut ComponentUpdater,
    ) {
        let body = Element::<View> {
            key: ElementKey::user("tuikit-body"),
            props: ViewProps {
                children: std::mem::take(&mut props.children),
                flex_direction: Direction::Vertical,
                height: Constraint::Min(1),
                ..Default::default()
            },
        }
        .into();
        let mut rows = vec![
            bar(
                "tuikit-tabline",
                Row::Tabs {
                    tabs: std::mem::take(&mut props.tabs),
                    active: props.active,
                    crumbs: std::mem::take(&mut props.crumbs),
                },
            ),
            body,
            bar(
                "tuikit-statusline",
                Row::Status {
                    mode: std::mem::take(&mut props.mode),
                    color: props.mode_color,
                    left: std::mem::take(&mut props.left),
                    right: std::mem::take(&mut props.right),
                },
            ),
            bar(
                "tuikit-command",
                Row::Command(std::mem::take(&mut props.command)),
            ),
        ];
        updater.set_layout_style(LayoutStyle {
            flex_direction: Direction::Vertical,
            ..Default::default()
        });
        updater.update_children(&mut rows, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;
    use ratatui_kit::test_util::render_frame;

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn the_component_draws_the_same_cells_as_the_functions() {
        let c = Colors::default();
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        terminal
            .draw(|f| {
                let a = chrome::frame(f, &c);
                chrome::tabline(
                    f,
                    a.tabline,
                    &c,
                    &["Inbox", "Done"],
                    1,
                    &["one".to_string()],
                );
                chrome::statusline(f, a.statusline, &c, "NORMAL", c.success, "3 open", "12:00");
                chrome::cmdline(
                    f,
                    a.cmdline,
                    &c,
                    Cmdline::Idle {
                        hints: &[Hint::new("q", "quit")],
                        pending: &[],
                    },
                );
            })
            .unwrap();

        let buf = render_frame(
            element!(Theme(colors: c) {
                Chrome(
                    tabs: vec!["Inbox".to_string(), "Done".to_string()],
                    active: 1usize,
                    crumbs: vec!["one".to_string()],
                    mode: "NORMAL".to_string(),
                    mode_color: Some(c.success),
                    left: Line::from("3 open"),
                    right: Line::from("12:00"),
                    command: CommandLine::Idle {
                        hints: vec![Hint::new("q", "quit")],
                        pending: vec![],
                    },
                )
            }),
            40,
            6,
        );
        assert_eq!(&buf, terminal.backend().buffer());
    }

    #[test]
    fn the_children_fill_the_body_and_the_rows_stay_put() {
        let buf = render_frame(
            element!(Theme(colors: Colors::default()) {
                Chrome(
                    tabs: vec!["inbox".to_string()],
                    mode: "NORMAL".to_string(),
                    command: CommandLine::Message { text: "saved".to_string(), error: false },
                ) {
                    Text(text: Line::from("first"))
                }
            }),
            30,
            8,
        );
        assert_eq!(row(&buf, 0).trim(), "inbox");
        assert_eq!(row(&buf, 1), "first");
        assert_eq!(row(&buf, 6).trim(), "NORMAL");
        assert_eq!(row(&buf, 7), "saved");
    }

    #[test]
    fn a_badge_without_a_colour_is_the_primary_one() {
        let c = Colors::default();
        let buf = render_frame(
            element!(Theme(colors: c) {
                Chrome(tabs: vec!["x".to_string()], mode: "M".to_string())
            }),
            10,
            4,
        );
        assert_eq!(buf[(0, 2)].style().bg, Some(c.primary));
    }

    fn typed(input: &mut Input, s: &str) {
        for c in s.chars() {
            assert_eq!(on_key(input, Key::Char(c)), FieldEvent::Edited);
        }
    }

    #[test]
    fn the_palette_is_the_nord_one() {
        let c = Colors::default();
        let p = c.palette();
        assert_eq!(p.bg, c.bg);
        assert_eq!(p.fg, c.text);
        assert_eq!(p.accent, c.primary);
        assert_eq!(p.selection, c.panel);
        assert_eq!(p.error, c.error);
    }

    #[test]
    fn theme_paints_the_background_and_a_readable_selected_row() {
        let c = Colors::default();
        let buf = render_frame(
            element!(Theme(colors: c) {
                Select<String>(
                    items: vec!["alpha".to_string(), "beta".to_string()],
                    default_index: Some(0),
                    active: true,
                )
            }),
            20,
            6,
        );
        assert_eq!(buf[(19, 5)].style().bg, Some(c.bg), "the last cell");
        let row = (0..6)
            .find(|&y| (0..20).any(|x| buf[(x, y)].symbol() == "a"))
            .expect("the selected row");
        let cell = (0..20)
            .map(|x| &buf[(x, row)])
            .find(|cell| cell.symbol() == "a")
            .expect("its first letter");
        assert_eq!(cell.style().bg, Some(c.panel));
        assert_eq!(cell.style().fg, Some(c.text_bright));
    }

    #[test]
    fn a_field_draws_its_text_in_the_themes_colours() {
        let mut c = Colors::default();
        c.text = Color::Rgb(1, 2, 3);
        let buf = render_frame(
            element!(Theme(colors: c) {
                InputField(text: "hello".to_string())
            }),
            10,
            1,
        );
        assert_eq!(buf[(0, 0)].symbol(), "h");
        assert_eq!(buf[(0, 0)].style().fg, Some(c.text));
    }

    #[test]
    fn a_secret_field_draws_nothing_of_a_text_it_was_given() {
        let buf = render_frame(
            element!(InputField(secret: true, text: "hunter2".to_string())),
            10,
            1,
        );
        let row: String = (0..10).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert_eq!(row.trim(), "");
    }

    #[test]
    fn submit_hands_the_text_over_and_empties_the_field() {
        let mut input = Input::secret();
        typed(&mut input, "abc");
        assert_eq!(
            on_key(&mut input, Key::Enter),
            FieldEvent::Submit(Zeroizing::new("abc".to_string()))
        );
        assert!(input.is_empty());
    }

    #[test]
    fn cancel_wipes_a_secret_and_keeps_a_plain_text() {
        let mut secret = Input::secret();
        typed(&mut secret, "abc");
        assert_eq!(on_key(&mut secret, Key::Esc), FieldEvent::Cancel);
        assert!(secret.is_empty());

        let mut plain = Input::new("abc");
        assert_eq!(on_key(&mut plain, Key::Esc), FieldEvent::Cancel);
        assert_eq!(plain.text(), "abc");
    }

    #[test]
    fn a_key_it_doesnt_use_goes_on() {
        assert_eq!(on_key(&mut Input::default(), Key::Tab), FieldEvent::Unused);
    }
}
