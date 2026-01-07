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
      last_size: (80, 24), // Initial default, will be updated dynamically
    };

    view.connect(cx);
    Self::start_polling(cx);

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

  #[allow(clippy::cast_sign_loss)]
  fn scroll(&mut self, delta: i32, max_scroll: usize) {
    // View-level scrolling - update our scroll offset
    // Positive delta = scroll up (increase offset), negative = scroll down (decrease offset)
    let new_offset = if delta > 0 {
      self.scroll_offset.saturating_add(delta as usize)
    } else {
      self.scroll_offset.saturating_sub((-delta) as usize)
    };
    self.scroll_offset = new_offset.min(max_scroll);
  }

  fn scroll_to_bottom(&mut self) {
    self.scroll_offset = 0;
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
    let font_size = self.font_size;
    let line_height = self.line_height;
    let char_width = self.char_width;
    let cursor_style = self.cursor_style;

    // Theme colors
    let colors = cx.theme().colors;
    let bg_color = colors.sidebar;
    let cursor_color = colors.link;

    // Dynamic resize based on window viewport
    // Use viewport size to estimate terminal area
    let viewport = window.viewport_size();
    let viewport_width: f32 = viewport.width.into();
    let viewport_height: f32 = viewport.height.into();

    // Estimate content area based on typical layout:
    // - Sidebar: ~220px
    // - List panel: ~300px
    // - Remaining is detail panel (~55% of window)
    // - Toolbar/tabs: ~50px
    // - Padding: ~50px
    let estimated_width = (viewport_width * 0.55 - 50.0).max(200.0);
    let estimated_height = (viewport_height - 100.0).max(200.0);

    // Calculate terminal dimensions
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cols = (estimated_width / char_width).floor() as u16;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rows = (estimated_height / line_height).floor() as u16;

    // Clamp to reasonable values
    let cols = cols.clamp(40, 300);
    let rows = rows.clamp(10, 150);

    // Resize terminal if size changed
    if cols != self.last_size.0 || rows != self.last_size.1 {
      self.last_size = (cols, rows);
      if let Some(ref terminal) = self.terminal {
        terminal.resize(cols, rows);
      }
    }

    // Handle error state
    if let Some(ref err) = self.error {
      let error_message =
        if err.contains("No such container") || err.contains("container not found") || err.contains("is not running") {
          "Container is not running or no longer exists".to_string()
        } else if err.contains("OCI runtime exec failed")
          || err.contains("executable file not found")
          || err.contains("no such file or directory")
        {
          "Container does not have a shell available (minimal image)".to_string()
        } else {
          format!("Connection failed: {err}")
        };

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
            .gap(px(16.))
            .child(Icon::new(IconName::CircleX).size(px(48.)).text_color(colors.danger))
            .child(
              div()
                .text_sm()
                .text_color(colors.danger)
                .max_w(px(400.))
                .text_center()
                .child(error_message),
            )
            .child(
              h_flex().gap(px(8.)).child(
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

    // Build terminal lines with per-cell coloring
    let mut line_elements: Vec<gpui::AnyElement> = Vec::new();

    for (row_idx, line) in content.lines.iter().enumerate() {
      let show_cursor = content.cursor_visible && row_idx == content.cursor_row;

      // Build line with per-cell colors
      let mut cell_elements: Vec<gpui::AnyElement> = Vec::new();

      for (col_idx, cell) in line.cells.iter().enumerate() {
        let is_cursor = show_cursor && col_idx == content.cursor_col;

        // Get cell colors
        let cell_fg = rgb_to_hsla(cell.fg.0, cell.fg.1, cell.fg.2);
        let cell_bg = rgb_to_hsla(cell.bg.0, cell.bg.1, cell.bg.2);

        // Apply dim effect by reducing saturation/lightness
        let cell_fg = if cell.dim {
          hsla(cell_fg.h, cell_fg.s * 0.7, cell_fg.l * 0.7, cell_fg.a)
        } else {
          cell_fg
        };

        let char_str = if cell.char == '\0' || cell.char == ' ' {
          "\u{00A0}".to_string() // Non-breaking space for proper sizing
        } else {
          cell.char.to_string()
        };

        let mut cell_div = div()
          .text_size(px(font_size))
          .font_family("monospace")
          .text_color(cell_fg)
          .bg(cell_bg)
          .child(char_str);

        // Apply text styles
        if cell.bold {
          cell_div = cell_div.font_weight(gpui::FontWeight::BOLD);
        }
        if cell.italic {
          cell_div = cell_div.italic();
        }
        if cell.underline {
          cell_div = cell_div.text_decoration_1();
        }
        if cell.strikethrough {
          cell_div = cell_div.line_through();
        }

        // Apply cursor styling based on cursor_style
        if is_cursor {
          cell_div = match cursor_style {
            TerminalCursorStyle::Block => cell_div.bg(cursor_color).text_color(bg_color),
            TerminalCursorStyle::Underline => cell_div.border_b_2().border_color(cursor_color),
            TerminalCursorStyle::Bar => cell_div.border_l_2().border_color(cursor_color),
          };
        }

        cell_elements.push(cell_div.into_any_element());
      }

      // Add cursor at end if needed
      if show_cursor && content.cursor_col >= line.cells.len() {
        let mut end_cursor = div()
          .text_size(px(font_size))
          .font_family("monospace")
          .child("\u{00A0}");

        end_cursor = match cursor_style {
          TerminalCursorStyle::Block => end_cursor.bg(cursor_color).text_color(bg_color),
          TerminalCursorStyle::Underline => end_cursor.border_b_2().border_color(cursor_color),
          TerminalCursorStyle::Bar => end_cursor.border_l_2().border_color(cursor_color),
        };

        cell_elements.push(end_cursor.into_any_element());
      }

      let line_div = div()
        .w_full()
        .h(px(line_height))
        .flex()
        .items_center()
        .children(cell_elements);

      line_elements.push(line_div.into_any_element());
    }

    // Scrollbar - only show if there's content to scroll
    #[allow(clippy::cast_precision_loss)]
    let scrollbar = if content.total_lines > content.rows {
      // Use the actual rendered content height for scrollbar
      let content_height = line_elements.len() as f32 * line_height;
      let track_height = content_height.max(100.0); // Minimum track height

      // Calculate thumb size proportional to visible content
      let visible_ratio = content.rows as f32 / content.total_lines as f32;
      let thumb_height = (visible_ratio * track_height).max(20.0).min(track_height);

      // Calculate scroll position (0 = at bottom, 1 = at top of history)
      let max_scroll = (content.total_lines - content.rows) as f32;
      let scroll_ratio = if max_scroll > 0.0 {
        content.scroll_offset as f32 / max_scroll
      } else {
        0.0
      };
      // Thumb position: at bottom when scroll_ratio=0, at top when scroll_ratio=1
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
      // Terminal content area - flex to fill available space
      .child(
        div()
          .id("terminal-content")
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .relative()
          .children(line_elements)
          .children(scrollbar),
      )
      .into_any_element()
  }
}

/// Convert RGB values (0-255) to HSLA color
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
