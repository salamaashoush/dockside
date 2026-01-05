//! Loading and error state components for list views
//!
//! Displays loading indicators and error states while data is being fetched.

use gpui::{App, Div, Styled, div, prelude::*, px};
use gpui_component::{
  Icon, IconName,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};

use super::spinning_loader;

/// Render a loading state for a list view
pub fn render_loading(resource_name: &str, cx: &App) -> Div {
  let colors = &cx.theme().colors;

  v_flex()
    .flex_1()
    .w_full()
    .items_center()
    .justify_center()
    .gap(px(16.))
    .py(px(48.))
    .child(
      div()
        .size(px(48.))
        .rounded(px(12.))
        .bg(colors.sidebar)
        .flex()
        .items_center()
        .justify_center()
        .child(spinning_loader(px(24.), colors.muted_foreground)),
    )
    .child(
      div()
        .text_lg()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .child(format!("Loading {resource_name}...")),
    )
}

/// Render an error state for a list view
pub fn render_error(
  resource_name: &str,
  error_message: &str,
  on_retry: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static,
  cx: &App,
) -> Div {
  let colors = &cx.theme().colors;

  v_flex()
    .flex_1()
    .w_full()
    .items_center()
    .justify_center()
    .gap(px(16.))
    .py(px(48.))
    .child(
      div()
        .size(px(64.))
        .rounded(px(12.))
        .bg(colors.danger.opacity(0.1))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(IconName::CircleX).size(px(32.)).text_color(colors.danger)),
    )
    .child(
      div()
        .text_xl()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.secondary_foreground)
        .child(format!("Failed to load {resource_name}")),
    )
    .child(
      div()
        .max_w(px(400.))
        .text_sm()
        .text_color(colors.muted_foreground)
        .text_center()
        .child(error_message.to_string()),
    )
    .child(
      h_flex()
        .mt(px(8.))
        .child(Button::new("retry").label("Retry").primary().on_click(on_retry)),
    )
}
