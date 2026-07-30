//! Output formats: table (stdout) and interactive TUI.

mod table;
pub mod tui;

pub use table::print_snapshot;
pub use tui::{run_tui, TuiConfig};
