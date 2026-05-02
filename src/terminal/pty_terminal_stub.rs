//! Windows / non-Unix stub for the terminal module.
//!
//! libghostty currently has no Windows build path (the vendored Zig build
//! doesn't emit a Windows static lib). The UI surfaces this by rendering an
//! "unsupported" placeholder via `TerminalView`, which only constructs a
//! `PtyTerminal` after this stub returns `Err(...)`.

use anyhow::{Result, anyhow};

/// Type of terminal session
#[derive(Debug, Clone)]
pub enum TerminalSessionType {
  ColimaSsh {
    profile: Option<String>,
  },
  DockerExec {
    container_id: String,
    shell: Option<String>,
  },
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
}

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

#[derive(Debug, Clone, Default)]
pub struct TerminalLine {
  pub cells: Vec<TerminalCell>,
}

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
      cursor_visible: false,
      rows: 24,
      total_lines: 0,
      scroll_offset: 0,
    }
  }
}

#[derive(Default)]
pub struct TerminalState {
  pub connected: bool,
  pub error: Option<String>,
}

pub struct PtyTerminal;

impl PtyTerminal {
  pub fn new(_session_type: &TerminalSessionType) -> Result<Self> {
    Err(anyhow!("Terminal is not supported on this platform yet"))
  }

  pub fn with_size(_session_type: &TerminalSessionType, _cols: u16, _rows: u16) -> Result<Self> {
    Err(anyhow!("Terminal is not supported on this platform yet"))
  }

  pub fn send_bytes(&self, _bytes: &[u8]) {}
  pub fn send_char(&self, _c: char) {}
  pub fn send_key(&self, _key: &str, _ctrl: bool, _alt: bool, _shift: bool) {}

  pub fn get_content_with_offset(&self, _scroll_offset: usize) -> TerminalContent {
    TerminalContent::default()
  }

  pub fn max_scroll(&self) -> usize {
    0
  }

  pub fn is_connected(&self) -> bool {
    false
  }

  pub fn error(&self) -> Option<String> {
    Some("Terminal is not supported on this platform yet".to_string())
  }

  pub fn resize(&self, _cols: u16, _rows: u16) {}
  pub fn scroll_by(&self, _delta: isize) {}
  pub fn scroll_to_bottom(&self) {}

  pub fn resize_callback(&self) -> std::sync::Arc<dyn Fn(u16, u16) + Send + Sync + 'static> {
    use std::sync::Arc;
    Arc::new(|_, _| {})
  }

  #[allow(dead_code)]
  pub fn size(&self) -> (usize, usize) {
    (0, 0)
  }

  pub fn close(&mut self) {}
}
