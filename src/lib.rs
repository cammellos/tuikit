//! The look and the keys the terminal apps share (eardrill, rdtui, sonetui), so they behave and
//! look the same:
//!
//! - [`theme`]: the Nord-based palette and the styles named after what they mark;
//! - [`keys`]: key sequences in nvim notation and the keymaps that match them to actions;
//! - [`config`]: the `[keys]` and `[colors]` sections every app's config file has;
//! - [`chrome`]: the four rows around the app's own screen: tabline, body, statusline, command line;
//! - [`widgets`]: lists, hints, bars, and fitting text to terminal columns;
//! - [`term`]: the mouse, on while a guard lives.

pub mod chrome;
pub mod config;
pub mod keys;
pub mod term;
pub mod theme;
pub mod widgets;

pub use theme::Colors;
