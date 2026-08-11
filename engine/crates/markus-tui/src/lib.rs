//! markus-tui — Terminal User Interface
//!
//! Full Ratatui-based TUI replacing the Bash arrow-key menu.
//! Features: animated menus, chat view, model browser, system dashboard.

pub mod app;
pub mod chat;
pub mod events;
pub mod menu;
pub mod render;
pub mod widgets;

pub use app::{App, AppMode};
