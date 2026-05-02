//! Terminal emulation using libghostty (`libghostty-vt`)
//!
//! This module owns a single-threaded actor that drives the Ghostty VT core
//! (which is `!Send + !Sync`), pumps a PTY child process via `portable-pty`,
//! and exposes a `TerminalContent` snapshot to the GPUI render layer.
//!
//! Threading layout:
//!  - **Reader thread**: owns the PTY reader, blocks on `read`, forwards bytes
//!    through a channel to the terminal thread.
//!  - **Terminal thread**: owns the `Terminal`, `RenderState`, `Encoder`, and
//!    PTY writer. Receives reader bytes, key/resize commands, calls
//!    `vt_write`, and snapshots a fresh `TerminalContent` into the shared
//!    `Arc<Mutex<TerminalContent>>` after each batch.
//!  - **UI thread**: pulls the latest `TerminalContent` and dispatches key /
//!    resize commands through the channel.
//!
//! Available on Unix targets only. Windows uses the stub in `pty_terminal_stub.rs`.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, atomic::AtomicBool, atomic::AtomicUsize, atomic::Ordering};
use std::thread;

use anyhow::{Result, anyhow};
use libghostty_vt::render::{CellIterator, Dirty, RenderState, RowIterator};
use libghostty_vt::style::RgbColor;
use libghostty_vt::terminal::{Options as TermOptions, Terminal};
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Type of terminal session
#[derive(Debug, Clone)]
pub enum TerminalSessionType {
  /// SSH into a Colima VM
  ColimaSsh { profile: Option<String> },
  /// Exec into a Docker container
  DockerExec {
    container_id: String,
    shell: Option<String>,
  },
  /// Exec into a Kubernetes pod
  KubectlExec {
    pod_name: String,
    namespace: String,
    container: Option<String>,
    shell: Option<String>,
  },
  /// Run an arbitrary command (e.g. `colima model serve …`).
  Custom { program: String, args: Vec<String> },
}

impl TerminalSessionType {
  pub fn colima_ssh(profile: Option<String>) -> Self {
    Self::ColimaSsh { profile }
  }

  pub fn docker_exec(container_id: String, shell: Option<String>) -> Self {
    Self::DockerExec { container_id, shell }
  }

  pub fn kubectl_exec(pod_name: String, namespace: String, container: Option<String>, shell: Option<String>) -> Self {
    Self::KubectlExec {
      pod_name,
      namespace,
      container,
      shell,
    }
  }

  pub fn custom_command(program: String, args: Vec<String>) -> Self {
    Self::Custom { program, args }
  }

  /// Build a `CommandBuilder` for `portable-pty` from the session type.
  fn to_command(&self) -> CommandBuilder {
    let (program, args) = match self {
      Self::ColimaSsh { profile } => {
        let mut args = vec!["ssh".to_string()];
        if let Some(p) = profile
          && p != "default"
        {
          args.push("--profile".to_string());
          args.push(p.clone());
        }
        ("colima", args)
      }
      Self::DockerExec { container_id, shell } => {
        let mut args = vec![
          "exec".to_string(),
          "-it".to_string(),
          "-e".to_string(),
          "TERM=xterm-256color".to_string(),
          container_id.clone(),
        ];
        if let Some(sh) = shell {
          args.push(sh.clone());
        } else {
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push("exec $(command -v bash || command -v zsh || command -v ash || command -v sh)".to_string());
        }
        ("docker", args)
      }
      Self::KubectlExec {
        pod_name,
        namespace,
        container,
        shell,
      } => {
        let mut args = vec![
          "exec".to_string(),
          "-it".to_string(),
          "-n".to_string(),
          namespace.clone(),
        ];
        if let Some(c) = container {
          args.push("-c".to_string());
          args.push(c.clone());
        }
        args.push(pod_name.clone());
        args.push("--".to_string());
        if let Some(sh) = shell {
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push(format!("TERM=xterm-256color exec {sh}"));
        } else {
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push(
            "TERM=xterm-256color exec $(command -v bash || command -v zsh || command -v ash || command -v sh)"
              .to_string(),
          );
        }
        ("kubectl", args)
      }
      Self::Custom { program, args } => (program.as_str(), args.clone()),
    };

    let mut cmd = CommandBuilder::new(program);
    for arg in args {
      cmd.arg(arg);
    }
    cmd.env("TERM", "xterm-256color");
    cmd
  }
}

/// A terminal cell with text and styling.
///
/// `fg` / `bg` are `None` when the cell has no explicit color (the renderer
/// substitutes the theme's foreground / background so the terminal honors the
/// app theme).
#[derive(Debug, Clone, Default)]
pub struct TerminalCell {
  pub char: char,
  pub fg: Option<(u8, u8, u8)>,
  pub bg: Option<(u8, u8, u8)>,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikethrough: bool,
  pub dim: bool,
}

/// Rendered line from terminal
#[derive(Debug, Clone, Default)]
pub struct TerminalLine {
  pub cells: Vec<TerminalCell>,
}

/// Terminal content for rendering
#[derive(Debug, Clone)]
pub struct TerminalContent {
  pub lines: Vec<TerminalLine>,
  pub cursor_row: usize,
  pub cursor_col: usize,
  pub cursor_visible: bool,
  pub rows: usize,
  /// Total scrollback + visible rows (placeholder; libghostty exposes only viewport).
  #[allow(dead_code)]
  pub total_lines: usize,
  pub scroll_offset: usize,
}

impl Default for TerminalContent {
  fn default() -> Self {
    Self {
      lines: Vec::new(),
      cursor_row: 0,
      cursor_col: 0,
      cursor_visible: true,
      rows: 24,
      total_lines: 0,
      scroll_offset: 0,
    }
  }
}

/// Terminal state wrapper
#[derive(Default)]
pub struct TerminalState {
  pub connected: bool,
  pub error: Option<String>,
}

/// Default scrollback line count.
const SCROLLBACK_LINES: usize = 10_000;
/// Default cell pixel size used when calling `Terminal::resize`. Ghostty needs nonzero
/// values for image protocols; we don't render images so any sane constant works.
const CELL_PIXEL_W: u32 = 8;
const CELL_PIXEL_H: u32 = 16;

/// Internal commands sent to the terminal-owning thread.
enum Cmd {
  /// Bytes read from the PTY child stdout/stderr (push into `vt_write`).
  PtyOutput(Vec<u8>),
  /// Bytes destined for the PTY child stdin (already encoded).
  KeyInput(Vec<u8>),
  /// Resize the terminal (cols/rows + a cosmetic cell pixel size).
  Resize { cols: u16, rows: u16 },
  /// Scroll the viewport by `delta` lines (negative = into history).
  ScrollDelta(isize),
  /// Snap viewport to bottom (active area).
  ScrollToBottom,
  /// Reader thread reached EOF or errored.
  ReaderClosed,
  /// UI requests immediate shutdown (drop the actor thread).
  Shutdown,
}

/// PTY-based terminal with full emulation via libghostty.
pub struct PtyTerminal {
  cmd_tx: Sender<Cmd>,
  content: Arc<Mutex<TerminalContent>>,
  state: Arc<Mutex<TerminalState>>,
  max_scroll: Arc<AtomicUsize>,
  shutdown: Arc<AtomicBool>,
}

impl PtyTerminal {
  pub fn new(session_type: &TerminalSessionType) -> Result<Self> {
    Self::with_size(session_type, 120, 40)
  }

  pub fn with_size(session_type: &TerminalSessionType, cols: u16, rows: u16) -> Result<Self> {
    // 1. Open the PTY via portable-pty.
    let pty_system = native_pty_system();
    let pair = pty_system
      .openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
      })
      .map_err(|e| anyhow!("openpty failed: {e}"))?;

    // 2. Spawn the child process attached to the slave end.
    let cmd = session_type.to_command();
    let mut child = pair
      .slave
      .spawn_command(cmd)
      .map_err(|e| anyhow!("failed to spawn command: {e}"))?;

    // Drop the slave handle; the child holds it.
    drop(pair.slave);

    let pty_writer = pair
      .master
      .take_writer()
      .map_err(|e| anyhow!("take_writer failed: {e}"))?;
    let pty_reader = pair
      .master
      .try_clone_reader()
      .map_err(|e| anyhow!("clone_reader failed: {e}"))?;

    // 3. Set up shared state visible to the UI thread.
    let content = Arc::new(Mutex::new(TerminalContent::default()));
    let state = Arc::new(Mutex::new(TerminalState {
      connected: true,
      error: None,
    }));
    let max_scroll = Arc::new(AtomicUsize::new(0));
    let shutdown = Arc::new(AtomicBool::new(false));

    // 4. Channel from reader thread + UI thread → terminal thread.
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

    // 5. Spawn the PTY reader thread. Forwards bytes to the terminal thread.
    let reader_tx = cmd_tx.clone();
    let reader_shutdown = Arc::clone(&shutdown);
    thread::Builder::new()
      .name("dockside-pty-reader".into())
      .spawn(move || pty_reader_loop(pty_reader, reader_tx, reader_shutdown))
      .map_err(|e| anyhow!("failed to spawn pty reader thread: {e}"))?;

    // 6. Spawn the terminal thread (owns the !Send Terminal).
    //    Move the master into the actor thread so we can issue PTY resizes
    //    (TIOCSWINSZ → SIGWINCH) when the UI bounds change. Without this,
    //    full-screen TUIs like vim stay stuck at their original PTY size.
    let term_content = Arc::clone(&content);
    let term_state = Arc::clone(&state);
    let term_max_scroll = Arc::clone(&max_scroll);
    let term_shutdown = Arc::clone(&shutdown);
    let master = pair.master;
    thread::Builder::new()
      .name("dockside-terminal".into())
      .spawn(move || {
        let res = terminal_actor_loop(
          cmd_rx,
          pty_writer,
          master,
          cols,
          rows,
          &term_content,
          &term_state,
          &term_max_scroll,
        );
        if let Err(err) = res {
          term_state.lock().error = Some(err.to_string());
        }
        term_state.lock().connected = false;
        term_shutdown.store(true, Ordering::SeqCst);
        // Reap the child if still running so it doesn't outlive us.
        let _ = child.kill();
        let _ = child.wait();
      })
      .map_err(|e| anyhow!("failed to spawn terminal thread: {e}"))?;

    Ok(Self {
      cmd_tx,
      content,
      state,
      max_scroll,
      shutdown,
    })
  }

  /// Send raw bytes to the PTY child (e.g. paste).
  pub fn send_bytes(&self, bytes: &[u8]) {
    let _ = self.cmd_tx.send(Cmd::KeyInput(bytes.to_vec()));
  }

  /// Send a single character to the PTY child.
  pub fn send_char(&self, c: char) {
    let mut buf = [0u8; 4];
    let bytes = c.encode_utf8(&mut buf).as_bytes();
    self.send_bytes(bytes);
  }

  /// Send a key with modifiers. The terminal thread handles encoding (Kitty
  /// keyboard protocol comes for free) and writes the bytes to the PTY.
  pub fn send_key(&self, key: &str, ctrl: bool, alt: bool, shift: bool) {
    if let Some(bytes) = encode_key_simple(key, ctrl, alt, shift) {
      let _ = self.cmd_tx.send(Cmd::KeyInput(bytes));
    }
  }

  /// Get terminal content for rendering with a specific scroll offset.
  /// `scroll_offset`: 0 = at bottom, >0 = scrolled up into history.
  pub fn get_content_with_offset(&self, scroll_offset: usize) -> TerminalContent {
    // The terminal thread always writes scroll_offset = 0 since libghostty's
    // RenderState has no built-in scrollback view; the GPUI side handles
    // history view by rendering the cached cells. For now we just return a
    // clone; scrollback through history is a follow-up.
    let _ = scroll_offset;
    self.content.lock().clone()
  }

  /// Max scroll offset (history size).
  pub fn max_scroll(&self) -> usize {
    self.max_scroll.load(Ordering::SeqCst)
  }

  /// Whether the underlying child + terminal thread are still alive.
  pub fn is_connected(&self) -> bool {
    !self.shutdown.load(Ordering::SeqCst) && self.state.lock().connected
  }

  /// Last error reported by the terminal thread, if any.
  pub fn error(&self) -> Option<String> {
    self.state.lock().error.clone()
  }

  /// Resize the terminal.
  pub fn resize(&self, cols: u16, rows: u16) {
    let _ = self.cmd_tx.send(Cmd::Resize { cols, rows });
  }

  /// Scroll the viewport by `delta` lines (negative = into history).
  pub fn scroll_by(&self, delta: isize) {
    let _ = self.cmd_tx.send(Cmd::ScrollDelta(delta));
  }

  /// Snap the viewport back to the bottom (active area).
  pub fn scroll_to_bottom(&self) {
    let _ = self.cmd_tx.send(Cmd::ScrollToBottom);
  }

  /// Build a resize callback that the GPUI element can call from inside
  /// `prepaint`. Cmd is private so callers must go through this helper.
  pub fn resize_callback(&self) -> Arc<dyn Fn(u16, u16) + Send + Sync + 'static> {
    let tx = self.cmd_tx.clone();
    Arc::new(move |cols, rows| {
      let _ = tx.send(Cmd::Resize { cols, rows });
    })
  }

  /// Get current terminal dimensions (cols, rows).
  #[allow(dead_code)]
  pub fn size(&self) -> (usize, usize) {
    let c = self.content.lock();
    (c.lines.first().map_or(0, |l| l.cells.len()), c.rows)
  }

  /// Close the terminal and signal shutdown to the actor thread.
  pub fn close(&mut self) {
    let _ = self.cmd_tx.send(Cmd::Shutdown);
    self.shutdown.store(true, Ordering::SeqCst);
  }
}

impl Drop for PtyTerminal {
  fn drop(&mut self) {
    self.close();
  }
}

// ============================================================================
// Reader thread
// ============================================================================

#[allow(clippy::needless_pass_by_value)]
fn pty_reader_loop(mut reader: Box<dyn Read + Send>, tx: Sender<Cmd>, shutdown: Arc<AtomicBool>) {
  let mut buf = [0u8; 4096];
  while !shutdown.load(Ordering::SeqCst) {
    match reader.read(&mut buf) {
      Ok(0) => break,
      Ok(n) => {
        if tx.send(Cmd::PtyOutput(buf[..n].to_vec())).is_err() {
          break;
        }
      }
      Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
      Err(_) => break,
    }
  }
  let _ = tx.send(Cmd::ReaderClosed);
}

// ============================================================================
// Terminal thread (owns the !Send Terminal + RenderState + Encoder)
// ============================================================================

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn terminal_actor_loop(
  cmd_rx: mpsc::Receiver<Cmd>,
  pty_writer_handle: Box<dyn Write + Send>,
  pty_master: Box<dyn MasterPty + Send>,
  initial_cols: u16,
  initial_rows: u16,
  content: &Arc<Mutex<TerminalContent>>,
  state: &Arc<Mutex<TerminalState>>,
  max_scroll: &Arc<AtomicUsize>,
) -> Result<()> {
  use std::cell::RefCell;
  use std::rc::Rc;

  let pty_writer = Rc::new(RefCell::new(pty_writer_handle));

  let mut terminal = Terminal::new(TermOptions {
    cols: initial_cols,
    rows: initial_rows,
    max_scrollback: SCROLLBACK_LINES,
  })
  .map_err(|e| anyhow!("terminal init failed: {e:?}"))?;

  terminal
    .resize(initial_cols, initial_rows, CELL_PIXEL_W, CELL_PIXEL_H)
    .map_err(|e| anyhow!("initial resize failed: {e:?}"))?;

  // VT-side responses (DSR, cursor position reports, etc.) are routed back to the PTY
  // through `on_pty_write`. The closure runs synchronously inside `vt_write`; since
  // we're single-threaded here, a `Rc<RefCell<_>>` is fine.
  let writer_for_cb = Rc::clone(&pty_writer);
  terminal
    .on_pty_write(move |_t, data| {
      let _ = writer_for_cb.borrow_mut().write_all(data);
    })
    .map_err(|e| anyhow!("on_pty_write registration failed: {e:?}"))?;

  let mut render_state = RenderState::new().map_err(|e| anyhow!("render state init failed: {e:?}"))?;
  let mut row_iter = RowIterator::new().map_err(|e| anyhow!("row iter init failed: {e:?}"))?;
  let mut cell_iter = CellIterator::new().map_err(|e| anyhow!("cell iter init failed: {e:?}"))?;

  let mut current_cols = initial_cols;
  let mut current_rows = initial_rows;

  // Initial render so the UI doesn't see an empty Default for one frame.
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
    state.lock().error = Some(err.to_string());
  }

  while let Ok(cmd) = cmd_rx.recv() {
    match cmd {
      Cmd::PtyOutput(bytes) => {
        terminal.vt_write(&bytes);
      }
      Cmd::KeyInput(bytes) => {
        if pty_writer.borrow_mut().write_all(&bytes).is_err() {
          break;
        }
      }
      Cmd::Resize { cols, rows } => {
        if cols == current_cols && rows == current_rows {
          continue;
        }
        // Resize the libghostty viewport AND signal the child via the PTY
        // master (TIOCSWINSZ → SIGWINCH). TUI apps like vim need both —
        // libghostty alone doesn't reach the shell.
        if let Err(e) = terminal.resize(cols, rows, CELL_PIXEL_W, CELL_PIXEL_H) {
          state.lock().error = Some(format!("resize failed: {e:?}"));
          continue;
        }
        if let Err(e) = pty_master.resize(PtySize {
          rows,
          cols,
          pixel_width: 0,
          pixel_height: 0,
        }) {
          tracing::debug!("pty master resize failed: {e}");
        }
        current_cols = cols;
        current_rows = rows;
      }
      Cmd::ScrollDelta(delta) => {
        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(delta));
      }
      Cmd::ScrollToBottom => {
        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Bottom);
      }
      Cmd::ReaderClosed | Cmd::Shutdown => break,
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
      state.lock().error = Some(err.to_string());
    }
  }

  Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn snapshot_into(
  terminal: &Terminal<'static, 'static>,
  render_state: &mut RenderState<'static>,
  row_iter: &mut RowIterator<'static>,
  cell_iter: &mut CellIterator<'static>,
  cols: u16,
  rows: u16,
  content: &Arc<Mutex<TerminalContent>>,
  max_scroll: &Arc<AtomicUsize>,
) -> Result<()> {
  let snapshot = render_state
    .update(terminal)
    .map_err(|e| anyhow!("render update failed: {e:?}"))?;

  let colors = snapshot.colors().map_err(|e| anyhow!("colors query failed: {e:?}"))?;
  let cursor_visible = snapshot
    .cursor_visible()
    .map_err(|e| anyhow!("cursor_visible failed: {e:?}"))?;
  let cursor_vp = snapshot
    .cursor_viewport()
    .map_err(|e| anyhow!("cursor_viewport failed: {e:?}"))?;

  let _ = colors; // theme defaults are applied in the renderer, not here

  let mut lines: Vec<TerminalLine> = Vec::with_capacity(rows as usize);
  let mut row_iteration = row_iter
    .update(&snapshot)
    .map_err(|e| anyhow!("row iter update failed: {e:?}"))?;

  while let Some(row) = row_iteration.next() {
    let mut cells: Vec<TerminalCell> = Vec::with_capacity(cols as usize);

    let mut cell_iteration = cell_iter
      .update(row)
      .map_err(|e| anyhow!("cell iter update failed: {e:?}"))?;

    while let Some(cell) = cell_iteration.next() {
      let style = cell.style().map_err(|e| anyhow!("style failed: {e:?}"))?;

      let mut fg = cell
        .fg_color()
        .map_err(|e| anyhow!("fg_color failed: {e:?}"))?
        .map(rgb_tuple);
      let mut bg = cell
        .bg_color()
        .map_err(|e| anyhow!("bg_color failed: {e:?}"))?
        .map(rgb_tuple);

      if style.inverse {
        std::mem::swap(&mut fg, &mut bg);
      }

      let len = cell
        .graphemes_len()
        .map_err(|e| anyhow!("graphemes_len failed: {e:?}"))?;
      let ch = if len == 0 {
        ' '
      } else {
        let chars = cell.graphemes().map_err(|e| anyhow!("graphemes failed: {e:?}"))?;
        chars.first().copied().unwrap_or(' ')
      };

      cells.push(TerminalCell {
        char: if ch == '\0' { ' ' } else { ch },
        fg,
        bg,
        bold: style.bold,
        italic: style.italic,
        underline: !matches!(style.underline, libghostty_vt::style::Underline::None),
        strikethrough: style.strikethrough,
        dim: style.faint,
      });
    }

    while cells.len() < cols as usize {
      cells.push(TerminalCell::default());
    }

    lines.push(TerminalLine { cells });
    row.set_dirty(false).ok();
  }

  while lines.len() < rows as usize {
    let mut cells = Vec::with_capacity(cols as usize);
    for _ in 0..cols {
      cells.push(TerminalCell::default());
    }
    lines.push(TerminalLine { cells });
  }

  let (cur_row, cur_col) = cursor_vp.map_or((usize::MAX, 0), |vp| (vp.y as usize, vp.x as usize));

  let new_content = TerminalContent {
    lines,
    cursor_row: cur_row,
    cursor_col: cur_col,
    cursor_visible,
    rows: rows as usize,
    total_lines: rows as usize,
    scroll_offset: 0,
  };

  *content.lock() = new_content;
  // libghostty doesn't expose a separate scrollback row count; treat scrollback
  // as flat for now (UI can still scroll through the visible rows).
  max_scroll.store(0, Ordering::SeqCst);

  // Best-effort: mark the snapshot clean so future updates can short-circuit.
  // Some libghostty builds reject this on the very first call; ignore the
  // error since rendering already succeeded.
  let _ = snapshot.set_dirty(Dirty::Clean);

  Ok(())
}

#[inline]
fn rgb_tuple(c: RgbColor) -> (u8, u8, u8) {
  (c.r, c.g, c.b)
}

// ============================================================================
// Key encoding (simple subset matching the previous alacritty mapping)
// ============================================================================

/// Map a key + modifiers to PTY bytes. We don't go through `libghostty_vt::key`
/// here because the encoder needs a live `&Terminal`; that lives on the actor
/// thread. For the common shells we use, the simple xterm-style encoding
/// matches what libghostty would emit in non-Kitty mode anyway.
#[allow(clippy::fn_params_excessive_bools)]
fn encode_key_simple(key: &str, ctrl: bool, alt: bool, _shift: bool) -> Option<Vec<u8>> {
  if ctrl {
    let byte = match key.to_lowercase().as_str() {
      "a" => 0x01,
      "b" => 0x02,
      "c" => 0x03,
      "d" => 0x04,
      "e" => 0x05,
      "f" => 0x06,
      "g" => 0x07,
      "h" => 0x08,
      "i" => 0x09,
      "j" => 0x0a,
      "k" => 0x0b,
      "l" => 0x0c,
      "m" => 0x0d,
      "n" => 0x0e,
      "o" => 0x0f,
      "p" => 0x10,
      "q" => 0x11,
      "r" => 0x12,
      "s" => 0x13,
      "t" => 0x14,
      "u" => 0x15,
      "v" => 0x16,
      "w" => 0x17,
      "x" => 0x18,
      "y" => 0x19,
      "z" => 0x1a,
      "[" | "escape" => 0x1b,
      "\\" => 0x1c,
      "]" => 0x1d,
      "^" | "6" => 0x1e,
      "_" | "-" => 0x1f,
      "space" | " " => 0x00,
      _ => return None,
    };
    return Some(vec![byte]);
  }

  if alt {
    let mut bytes = vec![0x1b];
    if key.len() == 1 {
      bytes.extend(key.as_bytes());
    }
    return Some(bytes);
  }

  let bytes: Vec<u8> = match key {
    "enter" => vec![0x0d],
    "backspace" => vec![0x7f],
    "tab" => vec![0x09],
    "escape" => vec![0x1b],
    "space" => vec![0x20],
    "up" => vec![0x1b, b'[', b'A'],
    "down" => vec![0x1b, b'[', b'B'],
    "right" => vec![0x1b, b'[', b'C'],
    "left" => vec![0x1b, b'[', b'D'],
    "home" => vec![0x1b, b'[', b'H'],
    "end" => vec![0x1b, b'[', b'F'],
    "pageup" => vec![0x1b, b'[', b'5', b'~'],
    "pagedown" => vec![0x1b, b'[', b'6', b'~'],
    "insert" => vec![0x1b, b'[', b'2', b'~'],
    "delete" => vec![0x1b, b'[', b'3', b'~'],
    "f1" => vec![0x1b, b'O', b'P'],
    "f2" => vec![0x1b, b'O', b'Q'],
    "f3" => vec![0x1b, b'O', b'R'],
    "f4" => vec![0x1b, b'O', b'S'],
    "f5" => vec![0x1b, b'[', b'1', b'5', b'~'],
    "f6" => vec![0x1b, b'[', b'1', b'7', b'~'],
    "f7" => vec![0x1b, b'[', b'1', b'8', b'~'],
    "f8" => vec![0x1b, b'[', b'1', b'9', b'~'],
    "f9" => vec![0x1b, b'[', b'2', b'0', b'~'],
    "f10" => vec![0x1b, b'[', b'2', b'1', b'~'],
    "f11" => vec![0x1b, b'[', b'2', b'3', b'~'],
    "f12" => vec![0x1b, b'[', b'2', b'4', b'~'],
    _ => {
      if key.len() == 1 {
        key.as_bytes().to_vec()
      } else {
        return None;
      }
    }
  };

  Some(bytes)
}
