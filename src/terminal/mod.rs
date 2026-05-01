#[cfg(unix)]
mod pty_terminal;
#[cfg(not(unix))]
mod pty_terminal_stub;

mod terminal_view;

#[cfg(unix)]
pub use pty_terminal::*;
#[cfg(not(unix))]
pub use pty_terminal_stub::*;

pub use terminal_view::*;
