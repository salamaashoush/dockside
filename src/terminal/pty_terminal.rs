//! Terminal emulation using `alacritty_terminal` backend
//!
//! This provides full terminal emulation support including:
//! - vim, nano, htop, and other full-screen applications
//! - Per-cell colors and attributes
//! - Alternate screen buffer
//! - All keyboard combinations (Ctrl, Alt, function keys)
//! - Terminal resize
//! - Mouse support (future)

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config as TermConfig, Term, TermMode};
use alacritty_terminal::tty::{self, Options as PtyOptions, Shell};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};
use anyhow::Result;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

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

  /// Build shell command for this session type
  fn to_shell(&self) -> Shell {
    match self {
      Self::ColimaSsh { profile } => {
        let mut args = vec!["ssh".to_string()];
        if let Some(p) = profile
          && p != "default"
        {
          args.push("--profile".to_string());
          args.push(p.clone());
        }
        Shell::new("colima".to_string(), args)
      }
      Self::DockerExec { container_id, shell } => {
        // Build docker exec command with proper shell
        let mut args = vec![
          "exec".to_string(),
          "-it".to_string(),
          "-e".to_string(),
          "TERM=xterm-256color".to_string(),
          container_id.clone(),
        ];

        if let Some(sh) = shell {
          // User specified a shell, use it directly
          args.push(sh.clone());
        } else {
          // Try shells in order of preference: bash, zsh, ash, sh
          // bash/zsh have full readline, ash has basic line editing, sh is last resort
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push("exec $(command -v bash || command -v zsh || command -v ash || command -v sh)".to_string());
        }

        Shell::new("docker".to_string(), args)
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
          // User specified shell - set TERM and run it
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push(format!("TERM=xterm-256color exec {sh}"));
        } else {
          // Try shells in order of preference with TERM set
          args.push("sh".to_string());
          args.push("-c".to_string());
          args.push(
            "TERM=xterm-256color exec $(command -v bash || command -v zsh || command -v ash || command -v sh)"
              .to_string(),
          );
        }

        Shell::new("kubectl".to_string(), args)
      }
    }
  }
}

/// A terminal cell with text and styling
#[derive(Debug, Clone)]
pub struct TerminalCell {
  pub char: char,
  pub fg: (u8, u8, u8),
  pub bg: (u8, u8, u8),
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikethrough: bool,
  pub dim: bool,
}

impl Default for TerminalCell {
  fn default() -> Self {
    Self {
      char: ' ',
      fg: (169, 177, 214), // Tokyo Night foreground
      bg: (26, 27, 38),    // Tokyo Night background
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
      dim: false,
    }
  }
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

/// Event proxy for alacritty terminal events
#[derive(Clone)]
struct EventProxy {
  sender: Sender<TerminalEvent>,
}

impl EventProxy {
  fn new(sender: Sender<TerminalEvent>) -> Self {
    Self { sender }
  }
}

impl EventListener for EventProxy {
  fn send_event(&self, event: Event) {
    let _ = self.sender.send(TerminalEvent::AlacrittyEvent(event));
  }
}

/// Internal terminal events
enum TerminalEvent {
  AlacrittyEvent(Event),
}

/// Terminal state wrapper
#[derive(Default)]
pub struct TerminalState {
  pub connected: bool,
  pub error: Option<String>,
}

/// Tokyo Night color palette for ANSI color mapping
fn ansi_to_rgb(color: AnsiColor) -> (u8, u8, u8) {
  match color {
    AnsiColor::Named(named) => match named {
      NamedColor::Black | NamedColor::Background => (26, 27, 38),
      NamedColor::Red | NamedColor::BrightRed => (247, 118, 142),
      NamedColor::Green | NamedColor::BrightGreen => (158, 206, 106),
      NamedColor::Yellow | NamedColor::BrightYellow => (224, 175, 104),
      NamedColor::Blue | NamedColor::BrightBlue => (122, 162, 247),
      NamedColor::Magenta | NamedColor::BrightMagenta => (187, 154, 247),
      NamedColor::Cyan | NamedColor::BrightCyan => (125, 207, 255),
      NamedColor::White => (192, 202, 245),
      NamedColor::BrightBlack => (65, 77, 104),
      NamedColor::BrightWhite => (255, 255, 255),
      _ => (169, 177, 214), // Default foreground, cursor
    },
    AnsiColor::Spec(Rgb { r, g, b }) => (r, g, b),
    AnsiColor::Indexed(idx) => {
      // 256 color palette - standard 16 ANSI colors
      match idx {
        0 => (26, 27, 38),         // Black
        1 | 9 => (247, 118, 142),  // Red / Bright Red
        2 | 10 => (158, 206, 106), // Green / Bright Green
        3 | 11 => (224, 175, 104), // Yellow / Bright Yellow
        4 | 12 => (122, 162, 247), // Blue / Bright Blue
        5 | 13 => (187, 154, 247), // Magenta / Bright Magenta
        6 | 14 => (125, 207, 255), // Cyan / Bright Cyan
        7 => (192, 202, 245),      // White
        8 => (65, 77, 104),        // Bright Black
        15 => (255, 255, 255),     // Bright White
        // 216 color cube (16-231)
        16..=231 => {
          let color_idx = idx - 16;
          let red = (color_idx / 36) % 6;
          let green = (color_idx / 6) % 6;
          let blue = color_idx % 6;
          let to_255 = |val: u8| if val == 0 { 0 } else { 55 + val * 40 };
          (to_255(red), to_255(green), to_255(blue))
        }
        // Grayscale (232-255)
        232..=255 => {
          let gray = 8 + (idx - 232) * 10;
          (gray, gray, gray)
        }
      }
    }
  }
}

/// PTY-based terminal with full emulation via `alacritty_terminal`
pub struct PtyTerminal {
  term: Arc<FairMutex<Term<EventProxy>>>,
  notifier: Notifier,
  state: Arc<Mutex<TerminalState>>,
  _event_receiver: Receiver<TerminalEvent>,
}

impl PtyTerminal {
  pub fn new(session_type: &TerminalSessionType) -> Result<Self> {
    Self::with_size(session_type, 120, 40) // Larger default for better coverage
  }

  pub fn with_size(session_type: &TerminalSessionType, cols: u16, rows: u16) -> Result<Self> {
    // Create event channel
    let (event_sender, event_receiver) = mpsc::channel();
    let event_proxy = EventProxy::new(event_sender);

    // Terminal configuration with scrollback history
    let term_config = TermConfig {
      scrolling_history: 10000, // 10k lines of scrollback
      ..TermConfig::default()
    };

    // Create terminal size
    let term_size = TermSize::new(cols as usize, rows as usize);

    // Create the terminal
    let term = Term::new(term_config, &term_size, event_proxy.clone());
    let term = Arc::new(FairMutex::new(term));

    // PTY options - set TERM for proper escape sequence handling
    let shell = session_type.to_shell();
    let mut env = HashMap::new();
    env.insert("TERM".to_string(), "xterm-256color".to_string());

    let pty_options = PtyOptions {
      shell: Some(shell),
      working_directory: Some(PathBuf::from("/")),
      env,
      drain_on_exit: false,
    };

    // Window size for PTY
    let window_size = WindowSize {
      num_cols: cols,
      num_lines: rows,
      cell_width: 1,
      cell_height: 1,
    };

    // Create PTY
    let pty = tty::new(&pty_options, window_size, 0)?;

    // Create event loop
    let event_loop = EventLoop::new(term.clone(), event_proxy, pty, false, false)?;

    // Get notifier for sending input
    let notifier = Notifier(event_loop.channel());

    // Spawn event loop thread
    let _event_loop_handle = event_loop.spawn();

    // Create state
    let state = Arc::new(Mutex::new(TerminalState {
      connected: true,
      error: None,
    }));

    // Spawn event handler thread
    let state_clone = Arc::clone(&state);
    let event_receiver_clone = event_receiver;

    thread::spawn(move || {
      while let Ok(event) = event_receiver_clone.recv() {
        if let TerminalEvent::AlacrittyEvent(Event::Exit) = event {
          let mut state = state_clone.lock();
          state.connected = false;
          break;
        }
      }
    });

    // Re-create event channel for storage (original was moved)
    let (_new_sender, new_receiver) = mpsc::channel();

    Ok(Self {
      term,
      notifier,
      state,
      _event_receiver: new_receiver,
    })
  }

  /// Send raw bytes to the terminal
  pub fn send_bytes(&self, bytes: &[u8]) {
    let _ = self.notifier.0.send(Msg::Input(Cow::Owned(bytes.to_vec())));
  }

  /// Send a character to the terminal
  pub fn send_char(&self, c: char) {
    let mut buf = [0u8; 4];
    let bytes = c.encode_utf8(&mut buf).as_bytes();
    self.send_bytes(bytes);
  }

  /// Send a key with modifiers
  pub fn send_key(&self, key: &str, ctrl: bool, alt: bool, shift: bool) {
    // For shells like bash/zsh, we use normal mode (ESC [ A style)
    // Application cursor mode (ESC O A style) is only used by specific apps like vim
    let app_cursor = {
      let term = self.term.lock();
      term.mode().contains(TermMode::APP_CURSOR)
    };

    let bytes = key_to_bytes(key, ctrl, alt, shift, app_cursor);
    self.send_bytes(&bytes);
  }

  /// Get terminal content for rendering with a specific scroll offset
  /// `scroll_offset`: 0 = at bottom (current screen), >0 = scrolled up into history
  pub fn get_content_with_offset(&self, scroll_offset: usize) -> TerminalContent {
    let term = self.term.lock();
    let grid = term.grid();

    let cols = grid.columns();
    let rows = grid.screen_lines();
    let history_size = grid.history_size();
    let total_lines = history_size + rows;

    // Clamp scroll offset to valid range
    let max_scroll = history_size;
    let actual_offset = scroll_offset.min(max_scroll);

    let mut lines = Vec::with_capacity(rows);

    // Calculate the line range to display based on our scroll offset
    // Line indices: negative = history, 0 to rows-1 = current screen
    // actual_offset=0: show lines 0 to rows-1 (current screen)
    // actual_offset=N: show lines -N to rows-1-N (scrolled into history)
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    let start_line = -(actual_offset as i32);
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    let end_line = start_line + rows as i32;

    for line_idx in start_line..end_line {
      let row_line = alacritty_terminal::index::Line(line_idx);

      // Check if this line exists in the grid
      let mut cells = Vec::with_capacity(cols);

      // Only access if the line is within valid range
      #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
      let history_min = -(history_size as i32);
      #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
      let rows_max = rows as i32;
      if line_idx >= history_min && line_idx < rows_max {
        let row = &grid[row_line];

        for col_idx in 0..cols {
          let cell = &row[alacritty_terminal::index::Column(col_idx)];
          let c = cell.c;

          // Get foreground and background colors
          let fg = ansi_to_rgb(cell.fg);
          let bg = ansi_to_rgb(cell.bg);

          // Get cell flags
          let flags = cell.flags;

          cells.push(TerminalCell {
            char: if c == '\0' { ' ' } else { c },
            fg,
            bg,
            bold: flags.contains(CellFlags::BOLD),
            italic: flags.contains(CellFlags::ITALIC),
            underline: flags.contains(CellFlags::UNDERLINE),
            strikethrough: flags.contains(CellFlags::STRIKEOUT),
            dim: flags.contains(CellFlags::DIM),
          });
        }
      } else {
        // Fill with empty cells for out-of-range lines
        for _ in 0..cols {
          cells.push(TerminalCell {
            char: ' ',
            fg: (200, 200, 200),
            bg: (0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
          });
        }
      }

      lines.push(TerminalLine { cells });
    }

    // Get cursor position
    let cursor = term.grid().cursor.point;
    let cursor_visible = term.mode().contains(TermMode::SHOW_CURSOR);

    // Convert cursor line to row index in our lines vector
    // If scrolled up, cursor might not be visible
    #[allow(clippy::cast_sign_loss)]
    let cursor_row = if cursor.line.0 >= start_line && cursor.line.0 < end_line {
      (cursor.line.0 - start_line) as usize
    } else {
      usize::MAX // Cursor not visible
    };

    TerminalContent {
      lines,
      cursor_row,
      cursor_col: cursor.column.0,
      cursor_visible: cursor_visible && cursor_row < rows,
      rows,
      total_lines,
      scroll_offset: actual_offset,
    }
  }

  /// Get max scroll offset (history size)
  pub fn max_scroll(&self) -> usize {
    let term = self.term.lock();
    term.grid().history_size()
  }

  /// Check if connected
  pub fn is_connected(&self) -> bool {
    self.state.lock().connected
  }

  /// Get error if any
  pub fn error(&self) -> Option<String> {
    self.state.lock().error.clone()
  }

  /// Resize the terminal
  pub fn resize(&self, cols: u16, rows: u16) {
    let window_size = WindowSize {
      num_cols: cols,
      num_lines: rows,
      cell_width: 1,
      cell_height: 1,
    };
    let _ = self.notifier.0.send(Msg::Resize(window_size));
    self.term.lock().resize(TermSize::new(cols as usize, rows as usize));
  }

  /// Get current terminal dimensions
  #[allow(dead_code)]
  pub fn size(&self) -> (usize, usize) {
    let term = self.term.lock();
    let grid = term.grid();
    (grid.columns(), grid.screen_lines())
  }

  /// Close the terminal
  pub fn close(&mut self) {
    let _ = self.notifier.0.send(Msg::Shutdown);
    self.state.lock().connected = false;
  }
}

impl Drop for PtyTerminal {
  fn drop(&mut self) {
    self.close();
  }
}

/// Convert key name to terminal escape sequence
#[allow(clippy::fn_params_excessive_bools)]
fn key_to_bytes(key: &str, ctrl: bool, alt: bool, _shift: bool, app_cursor: bool) -> Vec<u8> {
  // Handle Ctrl+key combinations
  if ctrl {
    return match key.to_lowercase().as_str() {
      "a" => vec![0x01],
      "b" => vec![0x02],
      "c" => vec![0x03],
      "d" => vec![0x04],
      "e" => vec![0x05],
      "f" => vec![0x06],
      "g" => vec![0x07],
      "h" => vec![0x08],
      "i" => vec![0x09],
      "j" => vec![0x0a],
      "k" => vec![0x0b],
      "l" => vec![0x0c],
      "m" => vec![0x0d],
      "n" => vec![0x0e],
      "o" => vec![0x0f],
      "p" => vec![0x10],
      "q" => vec![0x11],
      "r" => vec![0x12],
      "s" => vec![0x13],
      "t" => vec![0x14],
      "u" => vec![0x15],
      "v" => vec![0x16],
      "w" => vec![0x17],
      "x" => vec![0x18],
      "y" => vec![0x19],
      "z" => vec![0x1a],
      "[" | "escape" => vec![0x1b],
      "\\" => vec![0x1c],
      "]" => vec![0x1d],
      "^" | "6" => vec![0x1e],
      "_" | "-" => vec![0x1f],
      "space" | " " => vec![0x00],
      _ => vec![],
    };
  }

  // Handle Alt+key combinations
  if alt {
    let mut bytes = vec![0x1b]; // ESC prefix
    if key.len() == 1 {
      bytes.extend(key.as_bytes());
    }
    return bytes;
  }

  // Handle special keys
  match key {
    "enter" => vec![0x0d],
    "backspace" => vec![0x7f],
    "tab" => vec![0x09],
    "escape" => vec![0x1b],
    "space" => vec![0x20],
    "up" => {
      if app_cursor {
        vec![0x1b, b'O', b'A'] // ESC O A
      } else {
        vec![0x1b, b'[', b'A'] // ESC [ A
      }
    }
    "down" => {
      if app_cursor {
        vec![0x1b, b'O', b'B']
      } else {
        vec![0x1b, b'[', b'B']
      }
    }
    "right" => {
      if app_cursor {
        vec![0x1b, b'O', b'C']
      } else {
        vec![0x1b, b'[', b'C']
      }
    }
    "left" => {
      if app_cursor {
        vec![0x1b, b'O', b'D']
      } else {
        vec![0x1b, b'[', b'D']
      }
    }
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
      // Single character
      if key.len() == 1 {
        key.as_bytes().to_vec()
      } else {
        vec![]
      }
    }
  }
}
