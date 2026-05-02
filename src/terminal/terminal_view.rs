//! Terminal view component with full keyboard support
//!
//! Provides a terminal UI that supports:
//! - Full keyboard input (all Ctrl, Alt, function keys)
//! - Per-cell colors and styling
//! - Mouse wheel scrolling
//! - Dynamic terminal resize based on container bounds
//! - vim, nano, htop, and other full-screen applications

use std::sync::{Arc, Mutex};

use gpui::{
  App, Bounds, ClipboardItem, Context, FocusHandle, Focusable, Hsla, InteractiveElement, KeyDownEvent, MouseButton,
  MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
  Window, div, hsla, prelude::*, px,
};
use gpui_component::{
  Icon, IconName,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};

use super::{PtyTerminal, TerminalSessionType, grid_element::GridMetrics};
use crate::state::{TerminalCursorStyle, settings_state};

/// Cell in scroll-aware coords. `col` is the viewport column (cols don't
/// move with scrolling). `abs_row` is `viewport_row - scroll_offset` at
/// record time, so it stays attached to the same line of content as the
/// user scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsCell {
  col: u16,
  abs_row: i32,
}

/// Normalized absolute selection (start precedes end in reading order).
#[derive(Debug, Clone, Copy)]
struct AbsSelection {
  start: AbsCell,
  end: AbsCell,
}

/// Active drag-selection state in scroll-aware cell coords.
#[derive(Debug, Clone)]
struct SelectionState {
  anchor: AbsCell,
  focus: AbsCell,
  /// Mouse button still held — keep extending on move.
  dragging: bool,
  /// Cell content captured per `abs_row` while the row was visible in the
  /// viewport. Lets copy work even after the row has scrolled out.
  captured: std::collections::HashMap<i32, Vec<super::TerminalCell>>,
}

impl SelectionState {
  fn normalized(&self) -> AbsSelection {
    let (a, b) = (self.anchor, self.focus);
    let (start, end) = if a.abs_row < b.abs_row || (a.abs_row == b.abs_row && a.col <= b.col) {
      (a, b)
    } else {
      (b, a)
    };
    AbsSelection { start, end }
  }
  fn is_empty(&self) -> bool {
    self.anchor == self.focus
  }
  fn new(anchor: AbsCell, focus: AbsCell, dragging: bool) -> Self {
    Self {
      anchor,
      focus,
      dragging,
      captured: std::collections::HashMap::new(),
    }
  }
}

/// Convert a viewport cell at the given view scroll into an `AbsCell`.
fn viewport_to_abs(col: u16, viewport_row: u16, view_scroll: i32) -> AbsCell {
  AbsCell {
    col,
    abs_row: i32::from(viewport_row) - view_scroll,
  }
}

/// A functional terminal view with full keyboard and color support
pub struct TerminalView {
  terminal: Option<Arc<dyn super::TerminalSource>>,
  /// Session type for PTY-backed views; `None` for log-stream views.
  session_type: Option<TerminalSessionType>,
  focus_handle: FocusHandle,
  font_size: f32,
  line_height: f32,
  char_width: f32,
  cursor_style: TerminalCursorStyle,
  is_connected: bool,
  error: Option<String>,
  /// View-level scroll offset (lines from bottom, 0 = at bottom)
  scroll_offset: usize,
  /// Last known terminal size (cols, rows) for resize detection
  last_size: (u16, u16),
  /// Cursor blink phase. `true` = visible, `false` = hidden during blink off-phase.
  cursor_visible_phase: bool,
  /// Active text selection in scroll-aware cell coords.
  selection: Option<SelectionState>,
  /// Live cell-grid metrics published by the grid element each frame.
  /// Read on mouse events to convert pixel coords back to cell coords.
  grid_metrics: Arc<Mutex<Option<GridMetrics>>>,
  /// Our own running count of how far the viewport has been scrolled up
  /// from the active area (positive = into history, 0 = at bottom).
  /// libghostty does NOT expose a scroll-offset on its snapshot — every
  /// snapshot just contains "the current viewport rows" — so we have to
  /// keep this counter ourselves and feed it into selection coord math.
  view_scroll: i32,
  /// Last mouse position observed during a drag. The auto-scroll loop
  /// polls this to decide whether to scroll the viewport.
  last_drag_pos: Option<Point<Pixels>>,
  /// True while the auto-scroll-during-drag task is running. Prevents
  /// spawning duplicates when re-entering drag.
  drag_scroll_running: bool,
  /// Find-bar state. Toggled with Cmd+F / Ctrl+F. When `visible`, the
  /// query field is rendered above the grid and viewport matches are
  /// highlighted.
  search_visible: bool,
  search_query: String,
  search_input: Option<gpui::Entity<gpui_component::input::InputState>>,
}

impl TerminalView {
  pub fn new(session_type: TerminalSessionType, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    // Get terminal settings
    let settings = &settings_state(cx).read(cx).settings;
    let font_size = settings.terminal_font_size;
    let line_height = font_size * settings.terminal_line_height;
    let cursor_style = settings.terminal_cursor_style;
    // Monospace character width is approximately 0.6 * font_size
    let char_width = font_size * 0.6;

    let mut view = Self {
      terminal: None,
      session_type: Some(session_type),
      focus_handle,
      font_size,
      line_height,
      char_width,
      cursor_style,
      is_connected: false,
      error: None,
      scroll_offset: 0,
      last_size: (80, 24),
      cursor_visible_phase: true,
      selection: None,
      grid_metrics: Arc::new(Mutex::new(None)),
      view_scroll: 0,
      last_drag_pos: None,
      drag_scroll_running: false,
      search_visible: false,
      search_query: String::new(),
      search_input: None,
    };

    view.connect(cx);
    Self::start_polling(cx);
    Self::start_cursor_blink(cx);

    view
  }

  pub fn for_colima(profile: Option<String>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    Self::new(TerminalSessionType::colima_ssh(profile), window, cx)
  }

  pub fn connect(&mut self, cx: &mut Context<'_, Self>) {
    let Some(session_type) = self.session_type.clone() else {
      // Log-stream views don't reconnect — the source is already set.
      return;
    };
    match PtyTerminal::new(&session_type) {
      Ok(terminal) => {
        self.is_connected = true;
        self.error = None;
        self.terminal = Some(Arc::new(terminal));
      }
      Err(e) => {
        self.error = Some(e.to_string());
        self.is_connected = false;
      }
    }
    cx.notify();
  }

  /// Build a `TerminalView` that renders an external libghostty-backed
  /// `LogStream`. No PTY, no session type, no input — just the same
  /// grid + selection / scroll / drag-extend behaviour the interactive
  /// terminal has.
  pub fn for_log_stream(stream: Arc<crate::terminal::LogStream>, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    let settings = &settings_state(cx).read(cx).settings;
    let font_size = settings.terminal_font_size;
    let line_height = font_size * settings.terminal_line_height;
    let cursor_style = settings.terminal_cursor_style;
    let char_width = font_size * 0.6;

    let view = Self {
      terminal: Some(stream),
      session_type: None,
      focus_handle,
      font_size,
      line_height,
      char_width,
      cursor_style,
      is_connected: true,
      error: None,
      scroll_offset: 0,
      last_size: (80, 24),
      cursor_visible_phase: true,
      selection: None,
      grid_metrics: Arc::new(Mutex::new(None)),
      view_scroll: 0,
      last_drag_pos: None,
      drag_scroll_running: false,
      search_visible: false,
      search_query: String::new(),
      search_input: None,
    };

    Self::start_polling(cx);
    Self::start_cursor_blink(cx);
    view
  }

  /// Toggle `cursor_visible_phase` every ~500 ms. The render path inspects
  /// `settings.terminal_cursor_blink` and only hides the cursor when both the
  /// setting is on AND the phase is off.
  fn start_cursor_blink(cx: &mut Context<'_, Self>) {
    cx.spawn(async move |this, cx| {
      loop {
        gpui::Timer::after(std::time::Duration::from_millis(530)).await;
        let alive = this
          .update(cx, |this, cx| {
            this.cursor_visible_phase = !this.cursor_visible_phase;
            cx.notify();
            true
          })
          .unwrap_or(false);
        if !alive {
          break;
        }
      }
    })
    .detach();
  }

  fn start_polling(cx: &mut Context<'_, Self>) {
    cx.spawn(async move |this, cx| {
      loop {
        gpui::Timer::after(std::time::Duration::from_millis(16)).await;

        let should_continue = this
          .update(cx, |this, cx| {
            // Check terminal connection status
            if let Some(ref terminal) = this.terminal {
              this.is_connected = terminal.is_connected();
              if let Some(err) = terminal.error() {
                this.error = Some(err);
              }
            }
            cx.notify();
            true
          })
          .unwrap_or(false);

        if !should_continue {
          break;
        }
      }
    })
    .detach();
  }

  fn send_key(&self, key: &str, ctrl: bool, alt: bool, shift: bool) {
    if let Some(terminal) = &self.terminal {
      terminal.send_key(key, ctrl, alt, shift);
    }
  }

  fn send_char(&self, c: char) {
    if let Some(terminal) = &self.terminal {
      terminal.send_char(c);
    }
  }

  /// Forward a wheel scroll into libghostty's viewport.
  /// `delta > 0` = scroll up (reveal history) — libghostty wants negative.
  /// Also updates the view-scroll counter so selection abs_row math stays
  /// in sync with what libghostty actually shows.
  fn scroll(&mut self, delta: i32, _max_scroll: usize) {
    if let Some(terminal) = &self.terminal
      && delta != 0
    {
      let clamped = i16::try_from(delta).unwrap_or(0);
      terminal.scroll_by(-isize::from(clamped));
      // Mirror in our own counter. Clamp to >= 0 — libghostty silently
      // ignores requests to scroll below the active area, so we should too.
      self.view_scroll = (self.view_scroll + i32::from(clamped)).max(0);
    }
  }

  fn scroll_to_bottom(&mut self) {
    if let Some(terminal) = &self.terminal {
      terminal.scroll_to_bottom();
      self.view_scroll = 0;
    }
  }

  /// Resize terminal to fit given bounds (for future bounds-based resize)
  #[allow(dead_code)]
  pub fn resize_to_fit(&mut self, bounds: Bounds<Pixels>) {
    // Account for padding (12px on each side)
    let padding = 24.0;
    let available_width: f32 = bounds.size.width.into();
    let available_height: f32 = bounds.size.height.into();

    let content_width = (available_width - padding).max(100.0);
    let content_height = (available_height - padding).max(100.0);

    // Calculate cols and rows
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cols = (content_width / self.char_width).floor() as u16;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rows = (content_height / self.line_height).floor() as u16;

    // Clamp to reasonable values
    let cols = cols.clamp(20, 500);
    let rows = rows.clamp(5, 200);

    // Only resize if size changed
    if cols != self.last_size.0 || rows != self.last_size.1 {
      self.last_size = (cols, rows);
      if let Some(ref terminal) = self.terminal {
        terminal.resize(cols, rows);
      }
    }
  }

  fn paste(&self, cx: &mut Context<'_, Self>) {
    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
      && let Some(terminal) = &self.terminal
    {
      for c in text.chars() {
        terminal.send_char(c);
      }
    }
  }

  fn copy_selection(&self, cx: &mut Context<'_, Self>) {
    let Some(sel) = self.selection.as_ref() else {
      return;
    };
    if sel.is_empty() {
      return;
    }
    let Some(terminal) = &self.terminal else { return };
    let content = terminal.get_content_with_offset(self.scroll_offset);
    let total_rows = content.lines.len();
    if total_rows == 0 {
      return;
    }
    let total_cols = content.lines.first().map_or(0, |l| l.cells.len());
    if total_cols == 0 {
      return;
    }
    let last_col = u16::try_from(total_cols - 1).unwrap_or(u16::MAX);
    let cur_scroll = self.view_scroll;
    let range = sel.normalized();
    let s_abs = range.start.abs_row;
    let e_abs = range.end.abs_row;

    let mut out = String::new();
    for abs in s_abs..=e_abs {
      let (c0, c1) = if s_abs == e_abs {
        (range.start.col.min(last_col), range.end.col.min(last_col))
      } else if abs == s_abs {
        (range.start.col.min(last_col), last_col)
      } else if abs == e_abs {
        (0u16, range.end.col.min(last_col))
      } else {
        (0u16, last_col)
      };

      // Prefer captured snapshot for this abs row (works even after the
      // row scrolled out of view). Fall back to current viewport if it's
      // visible right now and we somehow have no captured copy.
      let cells_owned: Option<Vec<super::TerminalCell>> = sel.captured.get(&abs).cloned().or_else(|| {
        let v_row = abs + cur_scroll;
        let v_row_u = usize::try_from(v_row).ok()?;
        if v_row_u < total_rows {
          Some(content.lines[v_row_u].cells.clone())
        } else {
          None
        }
      });

      let mut piece = String::new();
      if let Some(cells) = cells_owned {
        let cap = (c1 - c0 + 1) as usize;
        piece.reserve(cap);
        for col in c0..=c1 {
          let ch = cells.get(col as usize).map_or(' ', |c| c.char);
          piece.push(if ch == '\0' { ' ' } else { ch });
        }
      }
      let trimmed = piece.trim_end_matches(' ').to_string();
      out.push_str(&trimmed);
      if abs != e_abs {
        out.push('\n');
      }
    }

    if !out.is_empty() {
      cx.write_to_clipboard(ClipboardItem::new_string(out));
    }
  }

  /// Convert a window-space pixel position into a viewport cell coord using
  /// the metrics the grid published this frame. Returns `None` if metrics
  /// are not yet available. The position is clamped INTO the grid even if
  /// the actual mouse pos is outside (used by the auto-scroll loop to know
  /// "which side did the cursor leave from").
  fn pos_to_viewport_cell(&self, pos: Point<Pixels>) -> Option<(u16, u16)> {
    let m = (*self.grid_metrics.lock().ok()?)?;
    if f32::from(m.cell_width) <= 0.0 || f32::from(m.line_height) <= 0.0 || m.cols == 0 || m.rows == 0 {
      return None;
    }
    let dx = f32::from(pos.x) - f32::from(m.origin.x);
    let dy = f32::from(pos.y) - f32::from(m.origin.y);
    let col_f = (dx / f32::from(m.cell_width)).floor();
    let row_f = (dy / f32::from(m.line_height)).floor();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let col = col_f.clamp(0.0, f32::from(m.cols.saturating_sub(1))) as u16;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let row = row_f.clamp(0.0, f32::from(m.rows.saturating_sub(1))) as u16;
    Some((col, row))
  }

  /// Convert pos to an `AbsCell` using the most recently rendered scroll
  /// offset. This is what mouse handlers call so the recorded coords stay
  /// stable as content scrolls.
  fn pos_to_abs_cell(&self, pos: Point<Pixels>) -> Option<AbsCell> {
    let (col, row) = self.pos_to_viewport_cell(pos)?;
    Some(viewport_to_abs(col, row, self.view_scroll))
  }

  /// Returns -1 if `pos` is above the grid, +1 if below, 0 if inside.
  /// Used by the drag auto-scroll loop.
  fn drag_edge(&self, pos: Point<Pixels>) -> i32 {
    let Some(m) = self.grid_metrics.lock().ok().and_then(|g| *g) else {
      return 0;
    };
    let top = f32::from(m.origin.y);
    #[allow(clippy::cast_precision_loss)]
    let bottom = top + f32::from(m.line_height) * f32::from(m.rows);
    let y = f32::from(pos.y);
    match (y < top, y >= bottom) {
      (true, _) => -1,
      (_, true) => 1,
      _ => 0,
    }
  }

  /// Spawn the auto-scroll loop that runs while the user is drag-selecting
  /// and their cursor is outside the grid. Each tick scrolls the viewport
  /// by one row toward the cursor and re-anchors the focus, so the
  /// selection grows like in Ghostty / iTerm.
  fn start_drag_autoscroll(cx: &mut Context<'_, Self>) {
    cx.spawn(async move |this, cx| {
      loop {
        gpui::Timer::after(std::time::Duration::from_millis(33)).await;
        let still_alive = this
          .update(cx, |this, cx| {
            let active = this
              .selection
              .as_ref()
              .is_some_and(|s| s.dragging);
            if !active {
              this.drag_scroll_running = false;
              return false;
            }
            let Some(pos) = this.last_drag_pos else {
              return true;
            };
            let edge = this.drag_edge(pos);
            if edge == 0 {
              return true;
            }
            // edge < 0: cursor above grid → scroll into history (positive
            // delta in `scroll()`'s convention reveals older lines).
            this.scroll(if edge < 0 { 1 } else { -1 }, 0);
            // Recompute focus from the (now-clamped) cursor position so
            // selection extends to the new top/bottom row of the viewport.
            if let Some(new_focus) = this.pos_to_abs_cell(pos)
              && let Some(sel) = this.selection.as_mut()
            {
              sel.focus = new_focus;
            }
            cx.notify();
            true
          })
          .unwrap_or(false);
        if !still_alive {
          break;
        }
      }
    })
    .detach();
  }

  /// Snap `cell` outward to a word boundary in the given line. A "word" is
  /// a run of non-space chars.
  fn word_bounds_at(line: &super::TerminalLine, col: u16) -> (u16, u16) {
    let cells = &line.cells;
    if cells.is_empty() {
      return (col, col);
    }
    let last = u16::try_from(cells.len() - 1).unwrap_or(u16::MAX);
    let c = usize::from(col.min(last));
    let is_word = |ch: char| !ch.is_whitespace() && ch != '\0';
    if !is_word(cells[c].char) {
      return (col, col);
    }
    let mut left = c;
    while left > 0 && is_word(cells[left - 1].char) {
      left -= 1;
    }
    let mut right = c;
    while right + 1 < cells.len() && is_word(cells[right + 1].char) {
      right += 1;
    }
    (
      u16::try_from(left).unwrap_or(u16::MAX),
      u16::try_from(right).unwrap_or(u16::MAX),
    )
  }

  /// Last column with non-space content in the row, or 0 if blank.
  fn line_end_col(line: &super::TerminalLine) -> u16 {
    let mut last = 0u16;
    for (i, c) in line.cells.iter().enumerate() {
      if c.char != ' ' && c.char != '\0' {
        last = u16::try_from(i).unwrap_or(u16::MAX);
      }
    }
    last
  }
}

impl Focusable for TerminalView {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for TerminalView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Re-read terminal settings each frame so changes from the Settings view
    // (font size, line height, cursor style) take effect without rebuilding
    // the view.
    let settings = settings_state(cx).read(cx).settings.clone();
    let font_size = settings.terminal_font_size;
    let line_height = font_size * settings.terminal_line_height;
    let char_width = font_size * 0.6;
    let cursor_style = settings.terminal_cursor_style;
    self.font_size = font_size;
    self.line_height = line_height;
    self.char_width = char_width;
    self.cursor_style = cursor_style;

    // Theme colors
    let colors = cx.theme().colors;
    let bg_color = colors.sidebar;
    let cursor_color = colors.link;

    // Bounds-driven resize: the TerminalGrid element computes (cols, rows)
    // from the actual container bounds in `prepaint` and calls the
    // `on_resize` callback below. No more viewport-share guessing.
    let _ = window;

    // Handle error state
    if let Some(ref err) = self.error {
      let (headline, hint) = classify_terminal_error(err);

      return div()
        .id("terminal-error")
        .size_full()
        .flex_1()
        .min_h_0()
        .bg(bg_color)
        .flex()
        .items_center()
        .justify_center()
        .child(
          v_flex()
            .items_center()
            .gap(px(12.))
            .max_w(px(520.))
            .px(px(24.))
            .child(Icon::new(IconName::CircleX).size(px(48.)).text_color(colors.danger))
            .child(
              div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.danger)
                .text_center()
                .child(headline.to_string()),
            )
            .child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .text_center()
                .child(hint.to_string()),
            )
            .child(
              div()
                .text_xs()
                .font_family("monospace")
                .text_color(colors.muted_foreground.opacity(0.7))
                .text_center()
                .child(err.clone()),
            )
            .child(
              h_flex()
                .gap(px(8.))
                .mt(px(8.))
                .child(
                  Button::new("reconnect")
                    .label("Reconnect")
                    .primary()
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                      this.error = None;
                      this.connect(cx);
                    })),
                ),
            ),
        )
        .into_any_element();
    }

    // Handle connecting state
    if !self.is_connected {
      return div()
        .id("terminal-connecting")
        .size_full()
        .flex_1()
        .min_h_0()
        .bg(bg_color)
        .flex()
        .items_center()
        .justify_center()
        .child(
          v_flex()
            .items_center()
            .gap(px(16.))
            .child(
              Icon::new(IconName::Loader)
                .size(px(48.))
                .text_color(colors.muted_foreground),
            )
            .child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Connecting to container..."),
            ),
        )
        .into_any_element();
    }

    // Get terminal content with our scroll offset
    let (content, max_scroll) = if let Some(ref terminal) = self.terminal {
      let max = terminal.max_scroll();
      // Clamp scroll offset to valid range
      if self.scroll_offset > max {
        self.scroll_offset = max;
      }
      (terminal.get_content_with_offset(self.scroll_offset), max)
    } else {
      (super::TerminalContent::default(), 0)
    };

    // Apply blink: when the setting is enabled and we're in the off-phase,
    // hide the cursor for this frame.
    let mut content = content;
    if settings.terminal_cursor_blink && !self.cursor_visible_phase {
      content.cursor_visible = false;
    }
    let total_lines_for_scrollbar = content.lines.len();
    let visible_rows = content.rows;
    let scroll_offset_for_bar = content.scroll_offset;

    // Capture cells for any selection rows currently in viewport. This
    // is what makes copy work after the user scrolls past the selection
    // — every visible row in range gets its content snapshotted while
    // visible.
    let cur_scroll = self.view_scroll;
    if let Some(sel) = self.selection.as_mut() {
      let range = sel.normalized();
      for (row_idx, line) in content.lines.iter().enumerate() {
        let row_i32 = i32::try_from(row_idx).unwrap_or(i32::MAX);
        let abs = row_i32 - cur_scroll;
        if abs >= range.start.abs_row && abs <= range.end.abs_row {
          sel.captured.insert(abs, line.cells.clone());
        }
      }
    }

    // Convert the abs-row selection into clipped viewport coords for the
    // grid renderer. Rows that are entirely off-screen drop out; the
    // first/last visible row gets clipped to col 0 / last_col so the
    // overlay rect stays a continuous shape.
    let selection_for_grid = self.selection.as_ref().and_then(|sel| {
      if sel.is_empty() {
        return None;
      }
      let total_rows = content.lines.len();
      let total_cols = content.lines.first().map_or(0, |l| l.cells.len());
      if total_rows == 0 || total_cols == 0 {
        return None;
      }
      let last_col = u16::try_from(total_cols - 1).unwrap_or(u16::MAX);
      let rows_i32 = i32::try_from(total_rows).unwrap_or(i32::MAX);
      let range = sel.normalized();
      let s_v = range.start.abs_row + cur_scroll;
      let e_v = range.end.abs_row + cur_scroll;
      // Selection entirely above or below the viewport — nothing to draw.
      if e_v < 0 || s_v >= rows_i32 {
        return None;
      }
      let s_row_clamped = s_v.max(0);
      let e_row_clamped = e_v.min(rows_i32 - 1);
      let start_col = if s_row_clamped == s_v {
        range.start.col.min(last_col)
      } else {
        0
      };
      let end_col = if e_row_clamped == e_v {
        range.end.col.min(last_col)
      } else {
        last_col
      };
      let s_row = u16::try_from(s_row_clamped).unwrap_or(u16::MAX);
      let e_row = u16::try_from(e_row_clamped).unwrap_or(u16::MAX);
      Some(super::grid_element::GridSelection {
        start: (start_col, s_row),
        end: (end_col, e_row),
      })
    });

    // Custom GPUI element handles the actual cell grid.
    let on_resize = self.terminal.as_ref().map(|t| t.resize_callback());
    let selection_color = colors.selection;
    let selection_color = if selection_color.a >= 0.95 {
      // If theme reports an opaque selection swatch, drop alpha so glyphs
      // stay legible under the overlay.
      Hsla {
        a: 0.35,
        ..selection_color
      }
    } else {
      selection_color
    };
    // Compute search match cells (viewport rows only) for the current
    // query. Skipped when the find bar is hidden or empty.
    let search_matches: Vec<(u16, u16, u16)> = if self.search_visible && !self.search_query.is_empty() {
      let q: Vec<char> = self.search_query.chars().collect();
      let qlen = q.len();
      let mut hits = Vec::new();
      for (row_idx, line) in content.lines.iter().enumerate() {
        let line_cells = &line.cells;
        if line_cells.len() < qlen {
          continue;
        }
        for start in 0..=line_cells.len().saturating_sub(qlen) {
          if (0..qlen).all(|k| {
            let cell = &line_cells[start + k];
            let ch = if cell.char == '\0' { ' ' } else { cell.char };
            ch.eq_ignore_ascii_case(&q[k])
          }) {
            let r = u16::try_from(row_idx).unwrap_or(u16::MAX);
            let c0 = u16::try_from(start).unwrap_or(u16::MAX);
            let c1 = u16::try_from(start + qlen - 1).unwrap_or(u16::MAX);
            hits.push((r, c0, c1));
          }
        }
      }
      hits
    } else {
      Vec::new()
    };

    let search_match_color = Hsla {
      h: 50.0 / 360.0,
      s: 0.95,
      l: 0.55,
      a: 0.5,
    };

    let search_match_count = search_matches.len();
    let _ = search_match_count; // displayed via the find bar via `match_count` below
    let grid = super::grid_element::TerminalGrid {
      content,
      font_size: px(font_size),
      line_height_factor: settings.terminal_line_height,
      default_fg: colors.foreground,
      default_bg: bg_color,
      cursor_color,
      cursor_style,
      on_resize,
      last_reported_size: None,
      selection: selection_for_grid,
      selection_color,
      metrics_sink: Some(self.grid_metrics.clone()),
      search_matches: search_matches.clone(),
      search_match_color,
    };

    // Scrollbar - only show if there's content to scroll
    #[allow(clippy::cast_precision_loss)]
    let scrollbar = if total_lines_for_scrollbar > visible_rows {
      let content_height = total_lines_for_scrollbar as f32 * line_height;
      let track_height = content_height.max(100.0);

      let visible_ratio = visible_rows as f32 / total_lines_for_scrollbar as f32;
      let thumb_height = (visible_ratio * track_height).max(20.0).min(track_height);

      let max_lines = (total_lines_for_scrollbar - visible_rows) as f32;
      let scroll_ratio = if max_lines > 0.0 {
        scroll_offset_for_bar as f32 / max_lines
      } else {
        0.0
      };
      let thumb_top = (1.0 - scroll_ratio) * (track_height - thumb_height);

      Some(
        div()
          .absolute()
          .right(px(4.))
          .top(px(4.))
          .h(px(track_height))
          .w(px(8.))
          .rounded(px(4.))
          .bg(colors.border.opacity(0.3))
          .child(
            div()
              .absolute()
              .w_full()
              .top(px(thumb_top))
              .h(px(thumb_height))
              .bg(colors.muted_foreground.opacity(0.7))
              .rounded(px(4.)),
          ),
      )
    } else {
      None
    };

    // Store values for the scroll handler closure
    let scroll_line_height = line_height;
    let scroll_max = max_scroll;

    div()
      .id("terminal-container")
      .track_focus(&self.focus_handle)
      .flex()
      .flex_col()
      .size_full()
      .flex_1()
      .min_h_0()
      .bg(bg_color)
      .rounded(px(8.))
      .p(px(12.))
      .relative()
      // Keyboard input - full support for all keys
      .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
        let key = event.keystroke.key.as_str();
        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let shift = event.keystroke.modifiers.shift;
        let cmd = event.keystroke.modifiers.platform; // Cmd on macOS, Ctrl on other platforms

        // Find-bar shortcuts: Cmd+F (mac) or Ctrl+F (Linux/Windows). Esc
        // closes the bar.
        if (cmd || (ctrl && !shift)) && !alt && key.to_lowercase() == "f" {
          this.search_visible = !this.search_visible;
          if !this.search_visible {
            this.search_query.clear();
          }
          cx.notify();
          return;
        }
        if this.search_visible && key == "escape" {
          this.search_visible = false;
          this.search_query.clear();
          cx.notify();
          return;
        }

        // Clipboard shortcuts:
        //  - Cmd+C / Cmd+V on macOS (platform = Cmd)
        //  - Ctrl+Shift+C / Ctrl+Shift+V on Linux/Windows so plain Ctrl+C
        //    still sends SIGINT to the foreground process.
        if cmd && !ctrl && !alt {
          match key.to_lowercase().as_str() {
            "v" => {
              this.paste(cx);
              this.scroll_to_bottom();
              return;
            }
            "c" => {
              this.copy_selection(cx);
              return;
            }
            _ => {}
          }
        }
        if ctrl && shift && !alt {
          match key.to_lowercase().as_str() {
            "c" => {
              this.copy_selection(cx);
              return;
            }
            "v" => {
              this.paste(cx);
              this.scroll_to_bottom();
              return;
            }
            _ => {}
          }
        }

        // Typing past the modifier guards above invalidates any selection.
        this.selection = None;

        // Auto-scroll to bottom when typing
        this.scroll_to_bottom();

        // Handle special keys and character input
        match key {
          // Navigation and function keys
          "enter" | "backspace" | "tab" | "escape" | "space" | "up" | "down" | "left" | "right" | "home" | "end"
          | "pageup" | "pagedown" | "insert" | "delete" | "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8"
          | "f9" | "f10" | "f11" | "f12" => {
            this.send_key(key, ctrl, alt, shift);
          }
          // Single character keys
          _ if key.len() == 1 => {
            if ctrl || alt {
              this.send_key(key, ctrl, alt, shift);
            } else {
              // Regular character - send directly
              if let Some(c) = key.chars().next() {
                this.send_char(c);
              }
            }
          }
          _ => {}
        }
      }))
      // Mouse wheel scrolling - view-level scroll handling
      .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _window, cx| {
        // Get scroll delta - try both line and pixel delta
        let raw_lines = match event.delta {
          gpui::ScrollDelta::Lines(delta) => {
            // Lines-based scroll (discrete scroll wheel)
            delta.y
          }
          gpui::ScrollDelta::Pixels(delta) => {
            // Pixel-based scroll (trackpad) - convert to lines
            let y: f32 = delta.y.into();
            y / scroll_line_height
          }
        };

        // Convert to integer lines, ensuring at least 1 line for small movements
        #[allow(clippy::cast_possible_truncation)]
        let lines = if raw_lines.abs() >= 0.5 {
          raw_lines.round() as i32
        } else if raw_lines.abs() > 0.0 {
          if raw_lines > 0.0 { 1 } else { -1 }
        } else {
          0
        };

        // `lines > 0` from GPUI = wheel scrolled up. Pass through directly
        // — `scroll()` translates "+ = into history" into libghostty's
        // negative-delta convention.
        if lines != 0 {
          // Selection is anchored in scroll-aware (abs_row) coords, so the
          // highlight stays attached to the same content as the viewport
          // moves — no clear here.
          this.scroll(lines, scroll_max);
          cx.notify();
        }
      }))
      // Mouse-down: focus + start (or extend) selection.
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, ev: &MouseDownEvent, window, cx| {
          this.focus_handle.focus(window);
          let Some(viewport_cell) = this.pos_to_viewport_cell(ev.position) else {
            cx.notify();
            return;
          };
          let scroll = this.view_scroll;
          let abs = viewport_to_abs(viewport_cell.0, viewport_cell.1, scroll);
          let row_v = viewport_cell.1;
          match ev.click_count {
            1 if ev.modifiers.shift => {
              if let Some(sel) = this.selection.as_mut() {
                sel.focus = abs;
                sel.dragging = true;
              } else {
                this.selection = Some(SelectionState::new(abs, abs, true));
              }
            }
            2 => {
              if let Some(terminal) = &this.terminal {
                let content = terminal.get_content_with_offset(this.scroll_offset);
                if let Some(line) = content.lines.get(row_v as usize) {
                  let (l, r) = Self::word_bounds_at(line, viewport_cell.0);
                  let a = viewport_to_abs(l, row_v, scroll);
                  let f = viewport_to_abs(r, row_v, scroll);
                  this.selection = Some(SelectionState::new(a, f, false));
                }
              }
            }
            3 => {
              if let Some(terminal) = &this.terminal {
                let content = terminal.get_content_with_offset(this.scroll_offset);
                if let Some(line) = content.lines.get(row_v as usize) {
                  let end = Self::line_end_col(line);
                  let a = viewport_to_abs(0, row_v, scroll);
                  let f = viewport_to_abs(end, row_v, scroll);
                  this.selection = Some(SelectionState::new(a, f, false));
                }
              }
            }
            _ => {
              this.selection = Some(SelectionState::new(abs, abs, true));
            }
          }
          // If we just entered a drag, kick off the auto-scroll loop.
          if this
            .selection
            .as_ref()
            .is_some_and(|s| s.dragging)
            && !this.drag_scroll_running
          {
            this.drag_scroll_running = true;
            this.last_drag_pos = Some(ev.position);
            Self::start_drag_autoscroll(cx);
          }
          cx.notify();
        }),
      )
      // Mouse-move: extend selection while the left button is held.
      .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
        if !ev.dragging() {
          return;
        }
        let still_dragging = this.selection.as_ref().is_some_and(|s| s.dragging);
        if !still_dragging {
          return;
        }
        // Always remember the latest position so the auto-scroll loop can
        // act on it even when the cursor stops moving outside the grid.
        this.last_drag_pos = Some(ev.position);
        if let Some(abs) = this.pos_to_abs_cell(ev.position)
          && let Some(s) = this.selection.as_mut()
        {
          s.focus = abs;
        }
        cx.notify();
      }))
      // Mouse-up: finalize / clear empty selection.
      .on_mouse_up(
        MouseButton::Left,
        cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
          if let Some(sel) = this.selection.as_mut() {
            sel.dragging = false;
            if sel.is_empty() {
              this.selection = None;
            }
          }
          cx.notify();
        }),
      )
      // Find bar — only present when active. Lazily creates an
      // InputState on first show, mirrors the field's text into our
      // own `search_query`, and shows the live match count.
      .when(self.search_visible, |el| {
        if self.search_input.is_none() {
          self.search_input = Some(cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
              .placeholder("Search visible logs (Esc to close)")
          }));
        }
        if let Some(input) = self.search_input.clone() {
          let current = input.read(cx).text().to_string();
          if current != self.search_query {
            self.search_query = current;
          }
        }
        let match_count = search_matches.len();
        el.child(
          h_flex()
            .id("terminal-find-bar")
            .w_full()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .mb(px(8.))
            .rounded(px(6.))
            .bg(colors.muted)
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("Find:"),
            )
            .when_some(self.search_input.clone(), |el, input| {
              el.child(
                div()
                  .flex_1()
                  .child(gpui_component::input::Input::new(&input).appearance(false)),
              )
            })
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(format!("{match_count} matches")),
            ),
        )
      })
      // Terminal content area — custom Element renders the cell grid.
      .child(
        div()
          .id("terminal-content")
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .relative()
          .child(grid)
          .children(scrollbar),
      )
      .into_any_element()
  }
}

/// Classify a raw terminal error into (headline, hint) for the user.
fn classify_terminal_error(err: &str) -> (&'static str, &'static str) {
  let lower = err.to_lowercase();
  if lower.contains("no such container") || lower.contains("container not found") || lower.contains("no longer exists")
  {
    return (
      "Container not found",
      "It may have been removed. Refresh the container list and try again.",
    );
  }
  if lower.contains("is not running") || lower.contains("has exited") || lower.contains("exited") {
    return ("Container is not running", "Start the container first, then reconnect.");
  }
  if lower.contains("is paused") {
    return (
      "Container is paused",
      "Unpause the container to open a terminal session.",
    );
  }
  if lower.contains("oci runtime exec failed")
    || lower.contains("executable file not found")
    || lower.contains("no such file or directory")
  {
    return (
      "No shell available in this container",
      "Minimal images (distroless, scratch) ship without /bin/sh. Use `docker run --rm -it your-image sh` on a debug variant instead.",
    );
  }
  if lower.contains("permission denied") {
    return (
      "Permission denied",
      "The container or Docker daemon refused the exec. You may need elevated privileges.",
    );
  }
  if lower.contains("not supported on this platform") {
    return (
      "Terminal not supported on this platform",
      "Terminal sessions currently work on macOS and Linux only.",
    );
  }
  (
    "Failed to attach terminal",
    "Inspect the error below for details, then try Reconnect.",
  )
}

/// Convert RGB values (0-255) to HSLA color (kept for potential future use).
#[allow(dead_code, clippy::many_single_char_names)]
fn rgb_to_hsla(red: u8, green: u8, blue: u8) -> Hsla {
  let red_f = f32::from(red) / 255.0;
  let green_f = f32::from(green) / 255.0;
  let blue_f = f32::from(blue) / 255.0;

  let max_val = red_f.max(green_f).max(blue_f);
  let min_val = red_f.min(green_f).min(blue_f);
  let lightness = f32::midpoint(max_val, min_val);

  if (max_val - min_val).abs() < f32::EPSILON {
    return hsla(0.0, 0.0, lightness, 1.0);
  }

  let delta = max_val - min_val;
  let saturation = if lightness > 0.5 {
    delta / (2.0 - max_val - min_val)
  } else {
    delta / (max_val + min_val)
  };

  let hue = if (max_val - red_f).abs() < f32::EPSILON {
    let mut hue_val = (green_f - blue_f) / delta;
    if green_f < blue_f {
      hue_val += 6.0;
    }
    hue_val / 6.0
  } else if (max_val - green_f).abs() < f32::EPSILON {
    ((blue_f - red_f) / delta + 2.0) / 6.0
  } else {
    ((red_f - green_f) / delta + 4.0) / 6.0
  };

  hsla(hue, saturation, lightness, 1.0)
}
