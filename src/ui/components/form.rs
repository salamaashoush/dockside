//! Shared form primitives so every dialog uses one consistent field
//! layout: a small muted label above a full-width control, and section
//! headers between groups. Use these instead of bare placeholder-only
//! inputs.

use gpui::{App, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder, px};
use gpui_component::theme::ActiveTheme;

/// A labelled form field: `label` (xs, muted) stacked above `control`,
/// full width. Optional `hint` renders muted under the control.
pub fn form_field(
  label: impl Into<String>,
  control: impl IntoElement,
  hint: Option<&str>,
  cx: &App,
) -> impl IntoElement {
  let colors = cx.theme().colors;
  let hint = hint.map(ToString::to_string);
  div()
    .w_full()
    .flex()
    .flex_col()
    .gap(px(4.))
    .child(
      div()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .child(label.into()),
    )
    .child(control)
    .when_some(hint, |el, h| {
      el.child(div().text_xs().text_color(colors.muted_foreground).child(h))
    })
}

/// A section heading used to group related fields within a dialog.
pub fn form_section(title: impl Into<String>, cx: &App) -> impl IntoElement {
  let colors = cx.theme().colors;
  div()
    .w_full()
    .pt(px(4.))
    .text_sm()
    .font_weight(gpui::FontWeight::SEMIBOLD)
    .text_color(colors.foreground)
    .child(title.into())
}
