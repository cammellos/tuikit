//! Key bindings: actions, key sequences in nvim notation, and the matcher that turns keystrokes
//! into actions. An app declares its actions with [`actions!`] (one enum per keymap it needs: a
//! menu's, a drill's) and gets a [`Keymap`] for each, its defaults overridable from the config.
//!
//! Notation: plain characters (`n`, `G`, `gg`, `,o`) and `<...>` names: `<C-x>` (Ctrl),
//! `<CR>`/`<Enter>`, `<Esc>`, `<Space>`, `<Tab>`, `<S-Tab>`, `<BS>`, `<Up>`, `<Down>`, `<Left>`,
//! `<Right>`, `<lt>` (a literal `<`). A `<` that doesn't start one of those names is a literal `<`.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
}

impl Key {
    pub fn from_event(e: KeyEvent) -> Option<Key> {
        Some(match e.code {
            // Ctrl-Shift-x is Ctrl-x, as the terminal sends it
            KeyCode::Char(c) if e.modifiers.contains(KeyModifiers::CONTROL) => {
                Key::Ctrl(c.to_ascii_lowercase())
            }
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            _ => return None,
        })
    }
}

pub trait Action: Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;
    fn defaults(self) -> &'static [&'static str];

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// An action enum and its [`Action`] impl: `Variant = "config_name" ["default", "<C-x>"]`.
///
/// ```
/// tuikit::actions!(Nav {
///     Down = "down" ["n", "<Down>"],
///     Quit = "quit" ["q"],
/// });
/// ```
#[macro_export]
macro_rules! actions {
    ($ty:ident { $($variant:ident = $name:literal [$($default:literal),*]),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $ty { $($variant),* }

        impl $crate::keys::Action for $ty {
            const ALL: &'static [$ty] = &[$($ty::$variant),*];

            fn name(self) -> &'static str {
                match self { $($ty::$variant => $name),* }
            }

            fn defaults(self) -> &'static [&'static str] {
                match self { $($ty::$variant => &[$($default),*]),* }
            }
        }
    };
}

pub fn parse_seq(s: &str) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some(k) = rest.find('>').and_then(|end| Some((end, parse_special(&rest[1..end])?))) {
                keys.push(k.1);
                rest = &rest[k.0 + 1..];
                continue;
            }
        }
        keys.push(Key::Char(c));
        rest = &rest[c.len_utf8()..];
    }
    if keys.is_empty() {
        return Err("empty key sequence".into());
    }
    Ok(keys)
}

fn parse_special(name: &str) -> Option<Key> {
    let lower = name.to_ascii_lowercase();
    Some(match lower.as_str() {
        "cr" | "enter" | "return" => Key::Enter,
        "esc" => Key::Esc,
        "space" => Key::Char(' '),
        "tab" => Key::Tab,
        "s-tab" => Key::BackTab,
        "bs" | "backspace" => Key::Backspace,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "lt" => Key::Char('<'),
        _ => {
            let c = lower.strip_prefix("c-")?;
            let mut chars = c.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Ctrl(ch)
        }
    })
}

pub enum Match<A> {
    Action(A),
    /// The keys so far start a longer binding: wait for more.
    Pending,
    None,
}

#[derive(Debug, Clone)]
pub struct Keymap<A> {
    bindings: Vec<(Vec<Key>, A)>,
}

impl<A: Action> Default for Keymap<A> {
    fn default() -> Self {
        Self::with_overrides(&HashMap::new(), "keys").0
    }
}

impl<A: Action> Keymap<A> {
    /// The default bindings, with each action named in `overrides` bound to exactly those
    /// sequences instead. Returns the problems found (unknown actions, bad notation), reported
    /// under `[section]`; those entries are skipped.
    pub fn with_overrides(
        overrides: &HashMap<String, Vec<String>>,
        section: &str,
    ) -> (Self, Vec<String>) {
        let mut errors = Vec::new();
        let mut unknown: Vec<&String> = overrides
            .keys()
            .filter(|name| A::from_name(name).is_none())
            .collect();
        unknown.sort();
        for name in unknown {
            errors.push(format!("unknown action in [{section}]: {name}"));
        }
        let mut bindings = Vec::new();
        for &action in A::ALL {
            let seqs: Vec<&str> = match overrides.get(action.name()) {
                Some(v) => v.iter().map(String::as_str).collect(),
                None => action.defaults().to_vec(),
            };
            for s in seqs {
                match parse_seq(s) {
                    Ok(keys) => bindings.push((keys, action)),
                    Err(e) => errors.push(format!("[{section}] {}: {s:?}: {e}", action.name())),
                }
            }
        }
        (Self { bindings }, errors)
    }

    /// Feed one keystroke. `pending` holds the keys typed so far.
    pub fn feed(&self, pending: &mut Vec<Key>, key: Key) -> Match<A> {
        if key == Key::Esc && !pending.is_empty() {
            pending.clear();
            return Match::None;
        }
        pending.push(key);
        match self.lookup(pending) {
            Match::None if pending.len() > 1 => {
                // Like nvim: an unfinished sequence is dropped and the last key is tried on its own.
                pending.clear();
                self.feed(pending, key)
            }
            Match::None => {
                pending.clear();
                Match::None
            }
            Match::Action(a) => {
                pending.clear();
                Match::Action(a)
            }
            Match::Pending => Match::Pending,
        }
    }

    fn lookup(&self, keys: &[Key]) -> Match<A> {
        let mut exact = None;
        let mut longer = false;
        for (seq, action) in &self.bindings {
            if seq.as_slice() == keys {
                exact = Some(*action);
            } else if seq.starts_with(keys) {
                longer = true;
            }
        }
        match (exact, longer) {
            (_, true) => Match::Pending,
            (Some(a), false) => Match::Action(a),
            (None, false) => Match::None,
        }
    }

    /// The first sequence bound to `action`, for help text; None when it's unbound.
    pub fn describe(&self, action: A) -> Option<String> {
        self.bindings
            .iter()
            .find(|(_, a)| *a == action)
            .map(|(seq, _)| seq.iter().map(display_key).collect())
    }

    /// [`Keymap::describe`] for running text: "(unbound)" when it's unbound.
    pub fn label(&self, action: A) -> String {
        self.describe(action)
            .unwrap_or_else(|| "(unbound)".into())
    }

    /// The single keys that start a binding, for spotting clashes with an app's other keys.
    pub fn first_keys(&self) -> impl Iterator<Item = (Key, A)> + '_ {
        self.bindings.iter().map(|(seq, a)| (seq[0], *a))
    }
}

pub fn display_key(k: &Key) -> String {
    match k {
        Key::Char(' ') => "<Space>".into(),
        Key::Char(c) => c.to_string(),
        Key::Ctrl(c) => format!("<C-{c}>"),
        Key::Enter => "<CR>".into(),
        Key::Esc => "<Esc>".into(),
        Key::Tab => "<Tab>".into(),
        Key::BackTab => "<S-Tab>".into(),
        Key::Backspace => "<BS>".into(),
        Key::Up => "<Up>".into(),
        Key::Down => "<Down>".into(),
        Key::Left => "<Left>".into(),
        Key::Right => "<Right>".into(),
    }
}

/// The keys typed so far of an unfinished sequence, as nvim's showcmd: "g", ",", "^w".
pub fn pending_label(keys: &[Key]) -> String {
    keys.iter()
        .map(|k| match k {
            Key::Char(c) => c.to_string(),
            Key::Ctrl(c) => format!("^{c}"),
            _ => String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::actions!(Nav {
        Down = "down" ["n", "<Down>"],
        Up = "up" ["e", "<Up>"],
        Top = "top" ["gg"],
        Remove = "remove" ["dd"],
        Select = "select" ["<CR>"],
        Quit = "quit" ["q"],
    });

    fn feed_all<A: Action>(map: &Keymap<A>, s: &str) -> Vec<A> {
        let mut pending = Vec::new();
        parse_seq(s)
            .unwrap()
            .into_iter()
            .filter_map(|k| match map.feed(&mut pending, k) {
                Match::Action(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn parses_nvim_notation() {
        assert_eq!(parse_seq("<C-d>").unwrap(), vec![Key::Ctrl('d')]);
        assert_eq!(parse_seq("gg").unwrap(), vec![Key::Char('g'), Key::Char('g')]);
        assert_eq!(parse_seq("<CR>").unwrap(), vec![Key::Enter]);
        assert_eq!(parse_seq("<Space>").unwrap(), vec![Key::Char(' ')]);
        assert_eq!(parse_seq("<lt>").unwrap(), vec![Key::Char('<')]);
        assert_eq!(parse_seq("<").unwrap(), vec![Key::Char('<')]);
        assert_eq!(
            parse_seq("<x>").unwrap(),
            vec![Key::Char('<'), Key::Char('x'), Key::Char('>')]
        );
        assert!(parse_seq("").is_err());
    }

    #[test]
    fn sequences_wait_and_fall_back() {
        let map = Keymap::<Nav>::default();
        assert_eq!(feed_all(&map, "n"), vec![Nav::Down]);
        assert_eq!(feed_all(&map, "gg"), vec![Nav::Top]);
        assert_eq!(feed_all(&map, "dd"), vec![Nav::Remove]);
        // `g` then `n`: no `gn`, so `n` acts on its own
        assert_eq!(feed_all(&map, "gn"), vec![Nav::Down]);
        // Esc drops an unfinished sequence
        assert_eq!(feed_all(&map, "d<Esc>d"), vec![]);
    }

    #[test]
    fn override_replaces_only_that_action() {
        let overrides = HashMap::from([
            ("down".to_string(), vec!["j".to_string()]),
            ("bogus".to_string(), vec!["x".to_string()]),
        ]);
        let (map, errors) = Keymap::<Nav>::with_overrides(&overrides, "keys");
        assert_eq!(feed_all(&map, "j"), vec![Nav::Down]);
        assert_eq!(feed_all(&map, "n"), vec![]);
        assert_eq!(feed_all(&map, "e"), vec![Nav::Up]);
        assert_eq!(errors, vec!["unknown action in [keys]: bogus"]);
    }

    #[test]
    fn describes_the_first_binding() {
        let map = Keymap::<Nav>::default();
        assert_eq!(map.describe(Nav::Select).as_deref(), Some("<CR>"));
        let overrides = HashMap::from([("quit".to_string(), vec![])]);
        let (map, _) = Keymap::<Nav>::with_overrides(&overrides, "keys");
        assert_eq!(map.describe(Nav::Quit), None);
        assert_eq!(map.label(Nav::Quit), "(unbound)");
        assert_eq!(Nav::from_name("top"), Some(Nav::Top));
    }

    #[test]
    fn pending_keys_read_like_showcmd() {
        assert_eq!(pending_label(&[Key::Char(','), Key::Ctrl('w')]), ",^w");
    }
}
