//! The look and the keys the terminal apps share (eardrill, rdtui, sonetui), so they behave and
//! look the same:
//!
//! - [`theme`]: the Nord-based palette and the styles named after what they mark;
//! - [`keys`]: key sequences in nvim notation and the keymaps that match them to actions;
//! - [`config`]: the `[keys]` and `[colors]` sections every app's config file has;
//! - [`chrome`]: the four rows around the app's own screen: tabline, body, statusline, command line;
//! - [`widgets`]: lists, tree rows, hints, bars, and fitting text to terminal columns;
//! - [`input`]: a one-line text input (a secret one draws dots);
//! - [`form`]: `label  value` rows of inputs and notes, with an error under a row;
//! - [`popup`]: a box over the body for a confirmation or a pick;
//! - [`term`]: the mouse, on while a guard lives.

pub mod chrome;
pub mod config;
pub mod form;
pub mod input;
pub mod keys;
pub mod popup;
pub mod term;
pub mod theme;
pub mod widgets;

pub use theme::Colors;
