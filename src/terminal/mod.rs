#[cfg(unix)]
mod pty_terminal;
#[cfg(not(unix))]
mod pty_terminal_stub;

pub(crate) mod grid_element;
#[cfg(unix)]
mod log_stream;
mod terminal_view;

#[cfg(unix)]
pub use log_stream::LogStream;

#[cfg(unix)]
pub use pty_terminal::*;
#[cfg(not(unix))]
pub use pty_terminal_stub::*;

pub use terminal_view::*;

use std::sync::Arc;

/// Common interface every terminal-grid backend implements. Lets
/// `TerminalView` drive either a real interactive PTY (`PtyTerminal`)
/// or a one-way log feed (`LogStream`) through the same code path —
/// selection, scrolling, drag-to-extend etc. all share one widget.
pub trait TerminalSource: Send + Sync + 'static {
  fn is_connected(&self) -> bool;
  fn error(&self) -> Option<String>;
  /// Latest viewport snapshot from libghostty.
  fn get_content_with_offset(&self, scroll_offset: usize) -> TerminalContent;
  fn max_scroll(&self) -> usize;
  /// Build a `(cols, rows)` resize callback the grid element invokes
  /// from `prepaint` once it knows the real container bounds.
  fn resize_callback(&self) -> Arc<dyn Fn(u16, u16) + Send + Sync + 'static>;
  fn scroll_by(&self, delta: isize);
  fn scroll_to_bottom(&self);
  /// Resize the terminal to the given cell dimensions.
  fn resize(&self, cols: u16, rows: u16);
  /// Send a key with modifiers. No-op for one-way sources (logs).
  fn send_key(&self, _key: &str, _ctrl: bool, _alt: bool, _shift: bool) {}
  /// Send a single character. No-op for one-way sources (logs).
  fn send_char(&self, _c: char) {}
}

#[cfg(unix)]
impl TerminalSource for PtyTerminal {
  fn is_connected(&self) -> bool {
    PtyTerminal::is_connected(self)
  }
  fn error(&self) -> Option<String> {
    PtyTerminal::error(self)
  }
  fn get_content_with_offset(&self, scroll_offset: usize) -> TerminalContent {
    PtyTerminal::get_content_with_offset(self, scroll_offset)
  }
  fn max_scroll(&self) -> usize {
    PtyTerminal::max_scroll(self)
  }
  fn resize_callback(&self) -> Arc<dyn Fn(u16, u16) + Send + Sync + 'static> {
    PtyTerminal::resize_callback(self)
  }
  fn scroll_by(&self, delta: isize) {
    PtyTerminal::scroll_by(self, delta);
  }
  fn scroll_to_bottom(&self) {
    PtyTerminal::scroll_to_bottom(self);
  }
  fn resize(&self, cols: u16, rows: u16) {
    PtyTerminal::resize(self, cols, rows);
  }
  fn send_key(&self, key: &str, ctrl: bool, alt: bool, shift: bool) {
    PtyTerminal::send_key(self, key, ctrl, alt, shift);
  }
  fn send_char(&self, c: char) {
    PtyTerminal::send_char(self, c);
  }
}

#[cfg(unix)]
impl TerminalSource for LogStream {
  fn is_connected(&self) -> bool {
    LogStream::is_alive(self)
  }
  fn error(&self) -> Option<String> {
    None
  }
  fn get_content_with_offset(&self, _scroll_offset: usize) -> TerminalContent {
    LogStream::content(self)
  }
  fn max_scroll(&self) -> usize {
    LogStream::max_scroll(self)
  }
  fn resize_callback(&self) -> Arc<dyn Fn(u16, u16) + Send + Sync + 'static> {
    LogStream::resize_callback(self)
  }
  fn scroll_by(&self, delta: isize) {
    LogStream::scroll_by(self, delta);
  }
  fn scroll_to_bottom(&self) {
    LogStream::scroll_to_bottom(self);
  }
  fn resize(&self, cols: u16, rows: u16) {
    LogStream::resize(self, cols, rows);
  }
  // send_key / send_char default to no-op.
}
