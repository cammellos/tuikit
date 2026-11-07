//! The look and the keys the terminal apps share (eardrill, rdtui, sonetui), so they behave and
//! look the same:
//!
//! - [`theme`]: the Nord-based palette and the styles named after what they mark;
//! - [`keys`]: key sequences in nvim notation and the keymaps that match them to actions;
//! - [`config`]: the `[keys]` and `[colors]` sections every app's config file has;
//! - [`chrome`]: the four rows around the app's own screen: tabline, body, statusline, command line;
//! - [`widgets`]: lists (and which item a click is on), tree rows, hints, bars, and fitting text
//!   to terminal columns;
//! - [`input`]: a one-line text input (a secret one draws dots);
//! - [`form`]: `label  value` rows of inputs, notes and actions, with an error under a row;
//! - [`popup`]: a box over the body for a confirmation or a pick;
//! - [`menu`]: a few choices where they were asked for (a right click), keys or clicks to pick;
//! - [`term`]: the mouse, on while a guard lives.

pub mod chrome;
pub mod config;
pub mod form;
pub mod input;
pub mod keys;
pub mod menu;
pub mod popup;
pub mod term;
pub mod theme;
pub mod widgets;

pub use theme::Colors;
