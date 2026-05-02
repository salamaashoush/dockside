//! Terminal view component with full keyboard support
//!
//! Provides a terminal UI that supports:
//! - Full keyboard input (all Ctrl, Alt, function keys)
//! - Per-cell colors and styling
//! - Mouse wheel scrolling
//! - Dynamic terminal resize based on container bounds
//! - vim, nano, htop, and other full-screen applications

use gpui::{
  App, Bounds, Context, FocusHandle, Focusable, Hsla, InteractiveElement, KeyDownEvent, MouseButton, ParentElement,
  Pixels, Render, ScrollWheelEvent, Styled, Window, div, hsla, prelude::*, px,
};
use gpui_component::{
  Icon, IconName,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};

use super::{PtyTerminal, TerminalSessionType};
use crate::state::{TerminalCursorStyle, settings_state};

/// A functional terminal view with full keyboard and color support
pub struct TerminalView {
  terminal: Option<PtyTerminal>,
  session_type: TerminalSessionType,
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
      session_type,
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
    match PtyTerminal::new(&self.session_type) {
      Ok(terminal) => {
        self.is_connected = true;
        self.error = None;
        self.terminal = Some(terminal);
      }
      Err(e) => {
        self.error = Some(e.to_string());
        self.is_connected = false;
      }
    }
    cx.notify();
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

  /// Forward a wheel scroll into libghostty's viewport. Positive `delta` =
  /// scroll up (into history), negative = scroll down (toward active area).
  fn scroll(&mut self, delta: i32, _max_scroll: usize) {
    if let Some(terminal) = &self.terminal
      && delta != 0
    {
      terminal.scroll_by(-isize::from(i16::try_from(delta).unwrap_or(0)));
    }
  }

  fn scroll_to_bottom(&mut self) {
    if let Some(terminal) = &self.terminal {
      terminal.scroll_to_bottom();
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

  #[allow(clippy::unused_self)]
  fn copy_selection(&self, _cx: &mut Context<'_, Self>) {
    // TODO: Implement text selection and copy
    // For now, this is a placeholder
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

    // Custom GPUI element handles the actual cell grid.
    let on_resize = self.terminal.as_ref().map(PtyTerminal::resize_callback);
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

        // Handle clipboard shortcuts (Cmd+V for paste, Cmd+C for copy)
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

        // Apply scroll if we have any delta
        // Negate for natural scrolling: swipe down (negative y) = scroll up (increase offset)
        if lines != 0 {
          this.scroll(-lines, scroll_max);
          cx.notify();
        }
      }))
      // Click to focus
      .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, window, cx| {
        this.focus_handle.focus(window);
        cx.notify();
      }))
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
