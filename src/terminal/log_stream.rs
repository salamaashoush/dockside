//! Log-only "terminal" — feeds raw bytes into a libghostty Terminal so we can
//! render container/pod logs through the same grid renderer the interactive
//! terminal uses (ANSI colors, cursor styles, scrollback) without spawning a
//! PTY or child process.

use std::sync::mpsc::{self, Sender};
use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread;

use anyhow::{Result, anyhow};
use libghostty_vt::render::{CellIterator, RenderState, RowIterator};
use libghostty_vt::terminal::{Options as TermOptions, ScrollViewport, Terminal};
use parking_lot::Mutex;

use super::pty_terminal::{TerminalContent, snapshot_into};

const SCROLLBACK_LINES: usize = 10_000;
const CELL_PIXEL_W: u32 = 8;
const CELL_PIXEL_H: u32 = 16;

enum Cmd {
  Bytes(Vec<u8>),
  Resize { cols: u16, rows: u16 },
  ScrollDelta(isize),
  ScrollToBottom,
  Shutdown,
}

/// PTY-less log viewer. Same render contract as `PtyTerminal` (publishes
/// `TerminalContent` snapshots into a shared mutex) but with no child
/// process / no PTY / no input channel — the producer just calls
/// [`feed_bytes`] with whatever the upstream stream yielded.
pub struct LogStream {
  cmd_tx: Sender<Cmd>,
  content: Arc<Mutex<TerminalContent>>,
  max_scroll: Arc<AtomicUsize>,
  shutdown: Arc<AtomicBool>,
}

impl LogStream {
  pub fn new(cols: u16, rows: u16) -> Result<Self> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    let content = Arc::new(Mutex::new(TerminalContent::default()));
    let max_scroll = Arc::new(AtomicUsize::new(0));
    let shutdown = Arc::new(AtomicBool::new(false));

    let term_content = Arc::clone(&content);
    let term_max_scroll = Arc::clone(&max_scroll);
    let term_shutdown = Arc::clone(&shutdown);

    thread::Builder::new()
      .name("dockside-log-stream".into())
      .spawn(move || {
        let _ = run_actor(cmd_rx, cols, rows, &term_content, &term_max_scroll);
        term_shutdown.store(true, Ordering::SeqCst);
      })
      .map_err(|e| anyhow!("failed to spawn log stream thread: {e}"))?;

    Ok(Self {
      cmd_tx,
      content,
      max_scroll,
      shutdown,
    })
  }

  pub fn feed_bytes(&self, bytes: Vec<u8>) {
    let _ = self.cmd_tx.send(Cmd::Bytes(bytes));
  }

  pub fn resize(&self, cols: u16, rows: u16) {
    let _ = self.cmd_tx.send(Cmd::Resize { cols, rows });
  }

  pub fn scroll_by(&self, delta: isize) {
    let _ = self.cmd_tx.send(Cmd::ScrollDelta(delta));
  }

  pub fn scroll_to_bottom(&self) {
    let _ = self.cmd_tx.send(Cmd::ScrollToBottom);
  }

  pub fn content(&self) -> TerminalContent {
    self.content.lock().clone()
  }

  pub fn max_scroll(&self) -> usize {
    self.max_scroll.load(Ordering::SeqCst)
  }

  pub fn is_alive(&self) -> bool {
    !self.shutdown.load(Ordering::SeqCst)
  }

  /// Build a resize callback the renderer can call from `prepaint` so the
  /// log viewport tracks the actual container bounds.
  pub fn resize_callback(&self) -> Arc<dyn Fn(u16, u16) + Send + Sync + 'static> {
    let tx = self.cmd_tx.clone();
    Arc::new(move |cols, rows| {
      let _ = tx.send(Cmd::Resize { cols, rows });
    })
  }
}

impl Drop for LogStream {
  fn drop(&mut self) {
    let _ = self.cmd_tx.send(Cmd::Shutdown);
  }
}

fn run_actor(
  cmd_rx: mpsc::Receiver<Cmd>,
  initial_cols: u16,
  initial_rows: u16,
  content: &Arc<Mutex<TerminalContent>>,
  max_scroll: &Arc<AtomicUsize>,
) -> Result<()> {
  let mut terminal = Terminal::new(TermOptions {
    cols: initial_cols,
    rows: initial_rows,
    max_scrollback: SCROLLBACK_LINES,
  })
  .map_err(|e| anyhow!("terminal init failed: {e:?}"))?;

  terminal
    .resize(initial_cols, initial_rows, CELL_PIXEL_W, CELL_PIXEL_H)
    .map_err(|e| anyhow!("initial resize failed: {e:?}"))?;

  // Logs never write back to a tty, so the on_pty_write callback is a sink.
  terminal
    .on_pty_write(|_t, _data| {})
    .map_err(|e| anyhow!("on_pty_write failed: {e:?}"))?;

  let mut render_state = RenderState::new().map_err(|e| anyhow!("render state init failed: {e:?}"))?;
  let mut row_iter = RowIterator::new().map_err(|e| anyhow!("row iter init failed: {e:?}"))?;
  let mut cell_iter = CellIterator::new().map_err(|e| anyhow!("cell iter init failed: {e:?}"))?;

  let mut current_cols = initial_cols;
  let mut current_rows = initial_rows;

  let _ = snapshot_into(
    &terminal,
    &mut render_state,
    &mut row_iter,
    &mut cell_iter,
    current_cols,
    current_rows,
    content,
    max_scroll,
  );

  while let Ok(cmd) = cmd_rx.recv() {
    match cmd {
      Cmd::Bytes(bytes) => terminal.vt_write(&bytes),
      Cmd::Resize { cols, rows } => {
        if cols == current_cols && rows == current_rows {
          continue;
        }
        if let Err(e) = terminal.resize(cols, rows, CELL_PIXEL_W, CELL_PIXEL_H) {
          tracing::warn!(target: "dockside.logstream", err = ?e, "resize failed");
          continue;
        }
        current_cols = cols;
        current_rows = rows;
      }
      Cmd::ScrollDelta(delta) => {
        terminal.scroll_viewport(ScrollViewport::Delta(delta));
      }
      Cmd::ScrollToBottom => {
        terminal.scroll_viewport(ScrollViewport::Bottom);
      }
      Cmd::Shutdown => break,
    }

    if let Err(err) = snapshot_into(
      &terminal,
      &mut render_state,
      &mut row_iter,
      &mut cell_iter,
      current_cols,
      current_rows,
      content,
      max_scroll,
    ) {
      tracing::warn!(target: "dockside.logstream", err = %err, "snapshot failed");
    }
  }
  Ok(())
}
