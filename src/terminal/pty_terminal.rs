use anyhow::Result;
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::Arc;
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

  fn build_command(&self) -> CommandBuilder {
    let mut cmd = match self {
      Self::ColimaSsh { profile } => {
        let mut cmd = CommandBuilder::new("colima");
        cmd.arg("ssh");
        if let Some(p) = profile
          && p != "default"
        {
          cmd.arg("--profile");
          cmd.arg(p);
        }
        cmd
      }
      Self::DockerExec { container_id, shell } => {
        let mut cmd = CommandBuilder::new("docker");
        cmd.arg("exec");
        cmd.arg("-it");
        cmd.arg("-e");
        cmd.arg("TERM=xterm-256color");
        cmd.arg(container_id);
        cmd.arg(shell.as_deref().unwrap_or("/bin/sh"));
        cmd
      }
      Self::KubectlExec {
        pod_name,
        namespace,
        container,
        shell,
      } => {
        let mut cmd = CommandBuilder::new("kubectl");
        cmd.arg("exec");
        cmd.arg("-it");
        cmd.arg("-n");
        cmd.arg(namespace);
        if let Some(c) = container {
          cmd.arg("-c");
          cmd.arg(c);
        }
        cmd.arg(pod_name);
        cmd.arg("--");
        cmd.arg(shell.as_deref().unwrap_or("/bin/sh"));
        cmd
      }
    };
    cmd.env("TERM", "xterm-256color");
    cmd
  }
}

/// Terminal key input
#[derive(Debug, Clone, Copy)]
pub enum TerminalKey {
  Char(char),
  Enter,
  Backspace,
  Tab,
  Escape,
  Up,
  Down,
  Right,
  Left,
  Home,
  End,
  PageUp,
  PageDown,
  Delete,
  CtrlC,
  CtrlD,
  CtrlL,
  CtrlZ,
}

impl TerminalKey {
  pub fn to_bytes(&self) -> Vec<u8> {
    match self {
      Self::Char(c) => {
        let mut buf = [0u8; 4];
        c.encode_utf8(&mut buf).as_bytes().to_vec()
      }
      Self::Enter => vec![0x0d],
      Self::Backspace => vec![0x7f],
      Self::Tab => vec![0x09],
      Self::Escape => vec![0x1b],
      Self::Up => vec![0x1b, 0x5b, 0x41],
      Self::Down => vec![0x1b, 0x5b, 0x42],
      Self::Right => vec![0x1b, 0x5b, 0x43],
      Self::Left => vec![0x1b, 0x5b, 0x44],
      Self::Home => vec![0x1b, 0x5b, 0x48],
      Self::End => vec![0x1b, 0x5b, 0x46],
      Self::PageUp => vec![0x1b, 0x5b, 0x35, 0x7e],
      Self::PageDown => vec![0x1b, 0x5b, 0x36, 0x7e],
      Self::Delete => vec![0x1b, 0x5b, 0x33, 0x7e],
      Self::CtrlC => vec![0x03],
      Self::CtrlD => vec![0x04],
      Self::CtrlL => vec![0x0c],
      Self::CtrlZ => vec![0x1a],
    }
  }
}

/// Rendered line from terminal
#[derive(Debug, Clone)]
pub struct TerminalLine {
  pub cells: Vec<TerminalCell>,
}

impl TerminalLine {
  pub fn new() -> Self {
    Self { cells: Vec::new() }
  }
}

impl Default for TerminalLine {
  fn default() -> Self {
    Self::new()
  }
}

/// A terminal cell with text
#[derive(Debug, Clone)]
pub struct TerminalCell {
  pub char: char,
}

impl Default for TerminalCell {
  fn default() -> Self {
    Self { char: ' ' }
  }
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

/// Terminal state managed with simple buffer approach
pub struct TerminalBuffer {
  lines: Vec<TerminalLine>,
  cursor_row: usize,
  cursor_col: usize,
  cols: usize,
  rows: usize,
  scroll_offset: i32,
  current_fg: (u8, u8, u8),
  current_bg: (u8, u8, u8),
  current_bold: bool,
  pub connected: bool,
  pub error: Option<String>,
}

impl Default for TerminalBuffer {
  fn default() -> Self {
    Self::new(80, 24)
  }
}

impl TerminalBuffer {
  pub fn new(cols: usize, rows: usize) -> Self {
    Self {
      lines: vec![TerminalLine::new()],
      cursor_row: 0,
      cursor_col: 0,
      cols,
      rows,
      scroll_offset: 0,
      current_fg: (169, 177, 214),
      current_bg: (26, 27, 38),
      current_bold: false,
      connected: false,
      error: None,
    }
  }

  fn ensure_line(&mut self, row: usize) {
    while self.lines.len() <= row {
      self.lines.push(TerminalLine::new());
    }
  }

  fn ensure_cell(&mut self, row: usize, col: usize) {
    self.ensure_line(row);
    while self.lines[row].cells.len() <= col {
      self.lines[row].cells.push(TerminalCell::default());
    }
  }

  pub fn put_char(&mut self, c: char) {
    self.ensure_cell(self.cursor_row, self.cursor_col);
    self.lines[self.cursor_row].cells[self.cursor_col] = TerminalCell { char: c };
    self.cursor_col += 1;

    // Wrap at column limit
    if self.cursor_col >= self.cols {
      self.cursor_col = 0;
      self.cursor_row += 1;
      self.ensure_line(self.cursor_row);
    }
  }

  pub fn newline(&mut self) {
    self.cursor_row += 1;
    self.cursor_col = 0;
    self.ensure_line(self.cursor_row);

    // Limit scrollback to 10000 lines
    const MAX_LINES: usize = 10000;
    if self.lines.len() > MAX_LINES {
      let excess = self.lines.len() - MAX_LINES;
      self.lines.drain(0..excess);
      self.cursor_row = self.cursor_row.saturating_sub(excess);
    }
  }

  pub fn carriage_return(&mut self) {
    self.cursor_col = 0;
  }

  pub fn backspace(&mut self) {
    if self.cursor_col > 0 {
      self.cursor_col -= 1;
    }
  }

  pub fn clear_line_from_cursor(&mut self) {
    self.ensure_line(self.cursor_row);
    if self.cursor_col < self.lines[self.cursor_row].cells.len() {
      self.lines[self.cursor_row].cells.truncate(self.cursor_col);
    }
  }

  pub fn clear_screen(&mut self) {
    self.lines.clear();
    self.lines.push(TerminalLine::new());
    self.cursor_row = 0;
    self.cursor_col = 0;
  }

  pub fn move_cursor(&mut self, row: usize, col: usize) {
    self.cursor_row = row;
    self.cursor_col = col;
    self.ensure_line(self.cursor_row);
  }

  pub fn set_fg(&mut self, r: u8, g: u8, b: u8) {
    self.current_fg = (r, g, b);
  }

  pub fn set_bg(&mut self, r: u8, g: u8, b: u8) {
    self.current_bg = (r, g, b);
  }

  pub fn set_bold(&mut self, bold: bool) {
    self.current_bold = bold;
  }

  pub fn reset_style(&mut self) {
    self.current_fg = (169, 177, 214);
    self.current_bg = (26, 27, 38);
    self.current_bold = false;
  }

  pub fn scroll(&mut self, delta: i32) {
    let max_scroll = self.lines.len().saturating_sub(self.rows) as i32;
    self.scroll_offset = (self.scroll_offset + delta).clamp(0, max_scroll.max(0));
  }

  pub fn scroll_to_bottom(&mut self) {
    self.scroll_offset = 0;
  }

  pub fn set_display_rows(&mut self, rows: usize) {
    self.rows = rows.max(10); // Minimum 10 rows
  }

  pub fn get_content(&self) -> TerminalContent {
    let total_lines = self.lines.len();
    let visible_start = if self.scroll_offset > 0 {
      total_lines.saturating_sub(self.rows + self.scroll_offset as usize)
    } else {
      total_lines.saturating_sub(self.rows)
    };

    let visible_end = (visible_start + self.rows).min(total_lines);
    let visible_lines: Vec<TerminalLine> = self.lines[visible_start..visible_end].to_vec();

    TerminalContent {
      lines: visible_lines,
      cursor_row: self.cursor_row.saturating_sub(visible_start),
      cursor_col: self.cursor_col,
      cursor_visible: self.scroll_offset == 0,
      rows: self.rows,
      total_lines,
      scroll_offset: self.scroll_offset as usize,
    }
  }
}

/// VTE performer for parsing terminal escape sequences
struct TerminalPerformer {
  buffer: Arc<Mutex<TerminalBuffer>>,
}

impl TerminalPerformer {
  fn new(buffer: Arc<Mutex<TerminalBuffer>>) -> Self {
    Self { buffer }
  }

  fn ansi_color_to_rgb(code: u8) -> (u8, u8, u8) {
    // Tokyo Night color palette
    match code {
      0 => (26, 27, 38),     // Black
      1 => (247, 118, 142),  // Red
      2 => (158, 206, 106),  // Green
      3 => (224, 175, 104),  // Yellow
      4 => (122, 162, 247),  // Blue
      5 => (187, 154, 247),  // Magenta
      6 => (125, 207, 255),  // Cyan
      7 => (192, 202, 245),  // White
      8 => (65, 77, 104),    // Bright Black
      9 => (247, 118, 142),  // Bright Red
      10 => (158, 206, 106), // Bright Green
      11 => (224, 175, 104), // Bright Yellow
      12 => (122, 162, 247), // Bright Blue
      13 => (187, 154, 247), // Bright Magenta
      14 => (125, 207, 255), // Bright Cyan
      15 => (255, 255, 255), // Bright White
      // 216 color cube (16-231)
      16..=231 => {
        let n = code - 16;
        let r = (n / 36) % 6;
        let g = (n / 6) % 6;
        let b = n % 6;
        let to_255 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        (to_255(r), to_255(g), to_255(b))
      }
      // Grayscale (232-255)
      232..=255 => {
        let gray = 8 + (code - 232) * 10;
        (gray, gray, gray)
      }
    }
  }
}

impl vte::Perform for TerminalPerformer {
  fn print(&mut self, c: char) {
    self.buffer.lock().put_char(c);
  }

  fn execute(&mut self, byte: u8) {
    let mut buf = self.buffer.lock();
    match byte {
      0x08 => buf.backspace(),
      0x09 => {
        let spaces = 8 - (buf.cursor_col % 8);
        for _ in 0..spaces {
          buf.put_char(' ');
        }
      }
      0x0a..=0x0c => buf.newline(),
      0x0d => buf.carriage_return(),
      _ => {}
    }
  }

  fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
  fn put(&mut self, _byte: u8) {}
  fn unhook(&mut self) {}
  fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

  fn csi_dispatch(&mut self, params: &vte::Params, _intermediates: &[u8], _ignore: bool, action: char) {
    let mut buf = self.buffer.lock();
    let params: Vec<u16> = params.iter().map(|p| p.first().copied().unwrap_or(0)).collect();

    match action {
      'm' => {
        if params.is_empty() {
          buf.reset_style();
          return;
        }
        let mut i = 0;
        while i < params.len() {
          match params[i] {
            0 => buf.reset_style(),
            1 => buf.set_bold(true),
            22 => buf.set_bold(false),
            30..=37 => {
              let rgb = Self::ansi_color_to_rgb(params[i] as u8 - 30);
              buf.set_fg(rgb.0, rgb.1, rgb.2);
            }
            38 => {
              if i + 2 < params.len() && params[i + 1] == 5 {
                let rgb = Self::ansi_color_to_rgb(params[i + 2] as u8);
                buf.set_fg(rgb.0, rgb.1, rgb.2);
                i += 2;
              } else if i + 4 < params.len() && params[i + 1] == 2 {
                buf.set_fg(params[i + 2] as u8, params[i + 3] as u8, params[i + 4] as u8);
                i += 4;
              }
            }
            39 => buf.set_fg(169, 177, 214),
            40..=47 => {
              let rgb = Self::ansi_color_to_rgb(params[i] as u8 - 40);
              buf.set_bg(rgb.0, rgb.1, rgb.2);
            }
            48 => {
              if i + 2 < params.len() && params[i + 1] == 5 {
                let rgb = Self::ansi_color_to_rgb(params[i + 2] as u8);
                buf.set_bg(rgb.0, rgb.1, rgb.2);
                i += 2;
              } else if i + 4 < params.len() && params[i + 1] == 2 {
                buf.set_bg(params[i + 2] as u8, params[i + 3] as u8, params[i + 4] as u8);
                i += 4;
              }
            }
            49 => buf.set_bg(26, 27, 38),
            90..=97 => {
              let rgb = Self::ansi_color_to_rgb(params[i] as u8 - 90 + 8);
              buf.set_fg(rgb.0, rgb.1, rgb.2);
            }
            _ => {}
          }
          i += 1;
        }
      }
      'H' | 'f' => {
        let row = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
        let col = params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
        buf.move_cursor(row, col);
      }
      'J' => {
        let mode = params.first().copied().unwrap_or(0);
        if mode == 2 || mode == 3 {
          buf.clear_screen();
        }
      }
      'K' => {
        let mode = params.first().copied().unwrap_or(0);
        if mode == 0 {
          buf.clear_line_from_cursor();
        }
      }
      'A' => {
        let n = params.first().copied().unwrap_or(1).max(1) as usize;
        buf.cursor_row = buf.cursor_row.saturating_sub(n);
      }
      'B' => {
        let n = params.first().copied().unwrap_or(1).max(1) as usize;
        buf.cursor_row += n;
        let row = buf.cursor_row;
        buf.ensure_line(row);
      }
      'C' => {
        let n = params.first().copied().unwrap_or(1).max(1) as usize;
        buf.cursor_col += n;
      }
      'D' => {
        let n = params.first().copied().unwrap_or(1).max(1) as usize;
        buf.cursor_col = buf.cursor_col.saturating_sub(n);
      }
      _ => {}
    }
  }

  fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

/// PTY-based terminal with scrolling support
pub struct PtyTerminal {
  buffer: Arc<Mutex<TerminalBuffer>>,
  writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
  running: Arc<Mutex<bool>>,
}

impl PtyTerminal {
  pub fn new(session_type: TerminalSessionType) -> Result<Self> {
    let pty_system = native_pty_system();

    let pair = pty_system.openpty(PtySize {
      rows: 24,
      cols: 80,
      pixel_width: 0,
      pixel_height: 0,
    })?;

    let cmd = session_type.build_command();
    let _child = pair.slave.spawn_command(cmd)?;

    let pty_writer = pair.master.take_writer()?;
    let writer: Arc<Mutex<Option<Box<dyn Write + Send>>>> = Arc::new(Mutex::new(Some(pty_writer)));

    let buffer = Arc::new(Mutex::new(TerminalBuffer::new(80, 40)));
    let running = Arc::new(Mutex::new(true));

    buffer.lock().connected = true;

    let buffer_clone = Arc::clone(&buffer);
    let running_clone = Arc::clone(&running);
    let mut reader = pair.master.try_clone_reader()?;

    thread::spawn(move || {
      let mut parser = vte::Parser::new();
      let mut performer = TerminalPerformer::new(buffer_clone.clone());
      let mut buf = [0u8; 4096];

      loop {
        if !*running_clone.lock() {
          break;
        }

        match reader.read(&mut buf) {
          Ok(0) => {
            buffer_clone.lock().connected = false;
            break;
          }
          Ok(n) => {
            parser.advance(&mut performer, &buf[..n]);
            // Auto-scroll to bottom on new output
            buffer_clone.lock().scroll_to_bottom();
          }
          Err(e) => {
            let mut b = buffer_clone.lock();
            b.error = Some(format!("Read error: {}", e));
            b.connected = false;
            break;
          }
        }
      }
    });

    Ok(Self {
      buffer,
      writer,
      running,
    })
  }

  pub fn send_key(&self, key: TerminalKey) -> Result<()> {
    if let Some(writer) = self.writer.lock().as_mut() {
      writer.write_all(&key.to_bytes())?;
      writer.flush()?;
    }
    Ok(())
  }

  pub fn buffer(&self) -> Arc<Mutex<TerminalBuffer>> {
    Arc::clone(&self.buffer)
  }

  pub fn scroll(&self, delta: i32) {
    self.buffer.lock().scroll(delta);
  }

  pub fn close(&mut self) {
    *self.running.lock() = false;
    *self.writer.lock() = None;
  }
}

impl Drop for PtyTerminal {
  fn drop(&mut self) {
    self.close();
  }
}
