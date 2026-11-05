//! The parts of an app's TOML config every app has: where it lives, `[keys]`-style keymap
//! sections, `[colors]`, and plain flags (`mouse = true`). Everything is optional; what's missing
//! keeps its default, and each problem is collected into `errors` for the app to show, the entry
//! keeping its default.
//!
//! ```toml
//! [keys]
//! # action = sequence or [sequences], nvim notation; replaces that action's defaults.
//! down = ["n", "<Down>"]
//!
//! [colors]
//! # "#rrggbb". Names and defaults from the system palette (Nord-based):
//! # bg surface panel faint text_dim text_header text text_bright primary
//! # error success warning accent2
//! primary = "#88c0d0"
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::keys::{Action, Keymap};
use crate::theme::{parse_hex, Colors};

/// `$XDG_CONFIG_HOME/<app>`, else `~/.config/<app>`.
pub fn dir(app: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(app))
}

/// The file's table: empty when there's no file, None (and the problem in `errors`) when it
/// can't be read or isn't TOML.
pub fn read(path: &Path, errors: &mut Vec<String>) -> Option<toml::Table> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text, &name, errors),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(toml::Table::new()),
        Err(e) => {
            errors.push(format!("{}: {e}", path.display()));
            None
        }
    }
}

/// `text` as a table; `name` is the file's, for the error.
pub fn parse(text: &str, name: &str, errors: &mut Vec<String>) -> Option<toml::Table> {
    match toml::from_str(text) {
        Ok(t) => Some(t),
        Err(e) => {
            errors.push(format!("{name}: {e}"));
            None
        }
    }
}

/// The keymap from `[section]`: action = sequence or [sequences].
pub fn keymap<A: Action>(
    table: &toml::Table,
    section: &str,
    errors: &mut Vec<String>,
) -> Keymap<A> {
    let mut overrides: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(keys) = table.get(section) {
        match keys.as_table() {
            Some(keys) => {
                for (action, value) in keys {
                    let seqs = match value {
                        toml::Value::String(s) => Some(vec![s.clone()]),
                        toml::Value::Array(a) => a
                            .iter()
                            .map(|v| v.as_str().map(str::to_string))
                            .collect::<Option<Vec<_>>>(),
                        _ => None,
                    };
                    match seqs {
                        Some(seqs) => {
                            overrides.insert(action.clone(), seqs);
                        }
                        None => errors.push(format!(
                            "[{section}] {action}: expected a string or a list of strings"
                        )),
                    }
                }
            }
            None => errors.push(format!("[{section}] must be a table")),
        }
    }
    let (map, e) = Keymap::with_overrides(&overrides, section);
    errors.extend(e);
    map
}

/// The palette with `[colors]` over the defaults.
pub fn colors(table: &toml::Table, errors: &mut Vec<String>) -> Colors {
    let mut colors = Colors::default();
    if let Some(table) = table.get("colors") {
        match table.as_table() {
            Some(table) => {
                for (name, value) in table {
                    match value.as_str().and_then(parse_hex) {
                        Some(c) if colors.set(name, c) => {}
                        Some(_) => errors.push(format!("unknown colour in [colors]: {name}")),
                        None => errors.push(format!("[colors] {name}: expected \"#rrggbb\"")),
                    }
                }
            }
            None => errors.push("[colors] must be a table".into()),
        }
    }
    colors
}

/// A top-level `key = true/false`.
pub fn flag(table: &toml::Table, key: &str, default: bool, errors: &mut Vec<String>) -> bool {
    match table.get(key) {
        None => default,
        Some(toml::Value::Boolean(on)) => *on,
        Some(_) => {
            errors.push(format!("{key}: expected true or false"));
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{parse_seq, Match};
    use ratatui::style::Color;

    crate::actions!(Nav {
        Down = "down" ["n", "<Down>"],
        Quit = "quit" ["q"],
    });

    fn action_for(map: &Keymap<Nav>, s: &str) -> Option<Nav> {
        let mut pending = Vec::new();
        parse_seq(s)
            .unwrap()
            .into_iter()
            .find_map(|k| match map.feed(&mut pending, k) {
                Match::Action(a) => Some(a),
                _ => None,
            })
    }

    fn load(text: &str) -> (Keymap<Nav>, Colors, bool, Vec<String>) {
        let mut errors = Vec::new();
        let table = parse(text, "test.toml", &mut errors).unwrap_or_default();
        let map = keymap(&table, "keys", &mut errors);
        let colors = colors(&table, &mut errors);
        let mouse = flag(&table, "mouse", false, &mut errors);
        (map, colors, mouse, errors)
    }

    #[test]
    fn empty_is_defaults() {
        let (map, colors, mouse, errors) = load("");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(action_for(&map, "n"), Some(Nav::Down));
        assert_eq!(colors, Colors::default());
        assert!(!mouse);
    }

    #[test]
    fn the_doc_example_loads_cleanly() {
        let doc: String = include_str!("config.rs")
            .lines()
            .skip_while(|l| !l.starts_with("//! ```toml"))
            .skip(1)
            .take_while(|l| !l.starts_with("//! ```"))
            .map(|l| l.trim_start_matches("//!").trim_start_matches(' '))
            .collect::<Vec<_>>()
            .join("\n");
        let (map, _, _, errors) = load(&doc);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(action_for(&map, "n"), Some(Nav::Down));
    }

    #[test]
    fn overrides() {
        let (map, colors, mouse, errors) = load(
            r##"
            mouse = true
            [keys]
            quit = ["q", "ZZ"]
            [colors]
            primary = "#ff0000"
            "##,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(action_for(&map, "ZZ"), Some(Nav::Quit));
        assert_eq!(colors.primary, Color::Rgb(255, 0, 0));
        assert_eq!(colors.bg, Colors::default().bg);
        assert!(mouse);
    }

    #[test]
    fn reports_bad_entries_and_keeps_defaults() {
        let (map, colors, mouse, errors) = load(
            r##"
            mouse = "yes"
            [keys]
            down = 3
            [colors]
            primary = "blue"
            nope = "#000000"
            "##,
        );
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert_eq!(action_for(&map, "n"), Some(Nav::Down));
        assert_eq!(colors.primary, Colors::default().primary);
        assert!(!mouse);
    }

    #[test]
    fn invalid_toml_is_reported() {
        let mut errors = Vec::new();
        assert!(parse("[keys", "x.toml", &mut errors).is_none());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("x.toml: "), "{errors:?}");
    }

    #[test]
    fn a_missing_file_is_an_empty_table() {
        let mut errors = Vec::new();
        let t = read(Path::new("/nonexistent/tuikit/config.toml"), &mut errors);
        assert_eq!(t, Some(toml::Table::new()));
        assert!(errors.is_empty());
    }
}
