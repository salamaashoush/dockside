use gpui::{
  App, Context, FocusHandle, Focusable, InteractiveElement, KeyDownEvent, MouseButton, ParentElement, Render,
  ScrollWheelEvent, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
  Icon, IconName,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};
use parking_lot::Mutex;
use std::sync::Arc;

use super::{PtyTerminal, TerminalBuffer, TerminalKey, TerminalSessionType};
use crate::state::settings_state;

/// A functional terminal view with keyboard input and mouse scrolling
pub struct TerminalView {
  terminal: Option<PtyTerminal>,
  buffer: Arc<Mutex<TerminalBuffer>>,
  session_type: TerminalSessionType,
  focus_handle: FocusHandle,
  font_size: f32,
  line_height: f32,
}

impl TerminalView {
  pub fn new(session_type: TerminalSessionType, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    let buffer = Arc::new(Mutex::new(TerminalBuffer::default()));

    // Get font size from settings
    let font_size = settings_state(cx).read(cx).settings.terminal_font_size;
    let line_height = font_size * 1.4; // Line height is 1.4x font size

    let mut view = Self {
      terminal: None,
      buffer,
      session_type,
      focus_handle,
      font_size,
      line_height,
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
        self.buffer = terminal.buffer();
        self.terminal = Some(terminal);
        self.buffer.lock().connected = true;
        self.buffer.lock().error = None;
      }
      Err(e) => {
        self.buffer.lock().error = Some(e.to_string());
        self.buffer.lock().connected = false;
      }
    }
    cx.notify();
  }

  fn start_polling(cx: &mut Context<'_, Self>) {
    cx.spawn(async move |this, cx| {
      loop {
        gpui::Timer::after(std::time::Duration::from_millis(16)).await;

        let should_continue = this
          .update(cx, |_this, cx| {
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

  fn send_key(&self, key: TerminalKey) {
    if let Some(terminal) = &self.terminal {
      let _ = terminal.send_key(key);
    }
  }

  fn scroll(&self, delta: i32) {
    if let Some(terminal) = &self.terminal {
      terminal.scroll(delta);
    }
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

    // Calculate rows based on window height
    // Estimate available height: window height minus toolbar/tabs (~120px) minus padding (24px)
    let window_height: f32 = window.viewport_size().height.into();
    let available_height = (window_height - 144.0).max(200.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let display_rows = ((available_height / line_height).round() as usize).clamp(10, 10000);

    let mut buffer = self.buffer.lock();
    buffer.set_display_rows(display_rows);
    let is_connected = buffer.connected;
    let error = buffer.error.clone();
    let content = buffer.get_content();
    drop(buffer);

    // Theme colors
    let colors = cx.theme().colors;
    let bg_color = colors.sidebar;
    let text_color = colors.foreground;
    let cursor_color = colors.link;

    if let Some(err) = error {
      // Error state - show helpful message with icon and reconnect option
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
                    this.connect(cx);
                  })),
              ),
            ),
        )
        .into_any_element();
    }

    if !is_connected {
      // Connecting state
      return div()
        .id("terminal-connecting")
        .size_full()
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

    // Build terminal lines
    let mut line_elements: Vec<gpui::AnyElement> = Vec::new();

    for (row_idx, line) in content.lines.iter().enumerate() {
      let mut line_text = String::new();

      for cell in &line.cells {
        line_text.push(cell.char);
      }

      // Ensure line has content for rendering
      if line_text.is_empty() {
        line_text = " ".to_string();
      }

      // Check if cursor is on this line
      let show_cursor = content.cursor_visible && row_idx == content.cursor_row;

      let line_div = if show_cursor && content.cursor_col < line_text.chars().count() {
        // Split line at cursor position for cursor rendering
        let chars: Vec<char> = line_text.chars().collect();
        let before: String = chars[..content.cursor_col].iter().collect();
        let cursor_char = chars.get(content.cursor_col).copied().unwrap_or(' ');
        let after: String = if content.cursor_col + 1 < chars.len() {
          chars[content.cursor_col + 1..].iter().collect()
        } else {
          String::new()
        };

        div()
          .w_full()
          .h(px(line_height))
          .flex()
          .items_center()
          .child(
            div()
              .text_size(px(font_size))
              .font_family("monospace")
              .text_color(text_color)
              .child(before),
          )
          .child(
            div()
              .text_size(px(font_size))
              .font_family("monospace")
              .bg(cursor_color)
              .text_color(bg_color)
              .child(cursor_char.to_string()),
          )
          .child(
            div()
              .text_size(px(font_size))
              .font_family("monospace")
              .text_color(text_color)
              .child(after),
          )
      } else if show_cursor {
        // Cursor at end of line
        div()
          .w_full()
          .h(px(line_height))
          .flex()
          .items_center()
          .child(
            div()
              .text_size(px(font_size))
              .font_family("monospace")
              .text_color(text_color)
              .child(line_text),
          )
          .child(
            div()
              .text_size(px(font_size))
              .font_family("monospace")
              .bg(cursor_color)
              .text_color(bg_color)
              .child(" "),
          )
      } else {
        div()
          .w_full()
          .h(px(line_height))
          .flex()
          .items_center()
          .text_size(px(font_size))
          .font_family("monospace")
          .text_color(text_color)
          .child(line_text)
      };

      line_elements.push(line_div.into_any_element());
    }

    // Scrollbar - show when there's more content than visible
    #[allow(clippy::cast_precision_loss)] // GUI rendering - precision loss acceptable for line counts
    let scrollbar = if content.total_lines > content.rows {
      let track_height = content.rows as f32 * line_height - 16.0; // Match visible area minus padding
      let visible_ratio = content.rows as f32 / content.total_lines as f32;
      let thumb_height = (visible_ratio * track_height).max(30.0);

      let max_scroll = (content.total_lines - content.rows) as f32;
      let scroll_position = if max_scroll > 0.0 {
        content.scroll_offset as f32 / max_scroll
      } else {
        0.0
      };
      // scroll_offset 0 = bottom, max = top, so invert for thumb position
      let thumb_top = (1.0 - scroll_position) * (track_height - thumb_height);

      Some(
        div()
          .absolute()
          .right(px(4.))
          .top(px(8.))
          .h(px(track_height))
          .w(px(8.))
          .rounded(px(4.))
          .bg(colors.border)
          .child(
            div()
              .absolute()
              .w_full()
              .top(px(thumb_top))
              .h(px(thumb_height))
              .bg(colors.muted_foreground)
              .rounded(px(4.)),
          ),
      )
    } else {
      None
    };

    div()
            .id("terminal-container")
            .track_focus(&self.focus_handle)
            .w_full()
            .h_full()
            .bg(bg_color)
            .rounded(px(8.))
            .p(px(12.))
            .overflow_hidden()
            .relative()
            // Keyboard input
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                let key = match event.keystroke.key.as_str() {
                    "enter" => Some(TerminalKey::Enter),
                    "backspace" => Some(TerminalKey::Backspace),
                    "tab" => Some(TerminalKey::Tab),
                    "escape" => Some(TerminalKey::Escape),
                    "space" => Some(TerminalKey::Char(' ')),
                    "up" => Some(TerminalKey::Up),
                    "down" => Some(TerminalKey::Down),
                    "left" => Some(TerminalKey::Left),
                    "right" => Some(TerminalKey::Right),
                    "home" => Some(TerminalKey::Home),
                    "end" => Some(TerminalKey::End),
                    "pageup" => Some(TerminalKey::PageUp),
                    "pagedown" => Some(TerminalKey::PageDown),
                    "delete" => Some(TerminalKey::Delete),
                    key if key.len() == 1 => {
                        let ch = key.chars().next().unwrap();
                        if event.keystroke.modifiers.control {
                            match ch.to_ascii_lowercase() {
                                'c' => Some(TerminalKey::CtrlC),
                                'd' => Some(TerminalKey::CtrlD),
                                'z' => Some(TerminalKey::CtrlZ),
                                'l' => Some(TerminalKey::CtrlL),
                                _ => None,
                            }
                        } else {
                            Some(TerminalKey::Char(ch))
                        }
                    }
                    _ => None,
                };

                if let Some(k) = key {
                    this.send_key(k);
                }
            }))
            // Mouse wheel scrolling
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, _cx| {
                let delta = event.delta.pixel_delta(px(1.0));
                let y_pixels: f32 = delta.y.into();
                #[allow(clippy::cast_possible_truncation)]
                let lines = (y_pixels / this.line_height).round() as i32;
                if lines != 0 {
                    this.scroll(-lines);
                }
            }))
            // Click to focus
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, window, cx| {
                this.focus_handle.focus(window);
                cx.notify();
            }))
            // Terminal lines
            .children(line_elements)
            .children(scrollbar)
            .into_any_element()
  }
}
