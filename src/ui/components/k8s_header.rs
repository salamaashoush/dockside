//! The single header bar shared by every Kubernetes view so the whole
//! k8s surface reads as one app instead of separate bolted-on screens.
//!
//! Layout: `[ left (tabs or title) ............ actions  namespace  context ]`
//! The context selector is always rightmost (the "which cluster am I on"
//! anchor, like Lens' cluster badge) and present on every k8s view.

use gpui::{App, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder, px};
use gpui_component::{h_flex, theme::ActiveTheme};

use super::{render_context_selector, render_namespace_selector};
use crate::state::docker_state;

/// Build the shared k8s header bar.
///
/// - `left`: the tab bar (group views) or a title block (Overview / Clusters).
/// - `show_namespace`: include the namespace selector (false for the
///   Clusters manager, which is not namespace-scoped).
/// - `actions`: optional right-aligned buttons (refresh, add…). Pass an
///   empty `div()` when there are none.
pub fn render_k8s_header(
  left: impl IntoElement,
  show_namespace: bool,
  actions: impl IntoElement,
  cx: &App,
) -> impl IntoElement {
  let colors = cx.theme().colors;
  let state = docker_state(cx).read(cx);
  // Connection health, visible on every k8s view: green = reachable,
  // red = last call errored, grey = not loaded yet.
  let status = if state.k8s_error.is_some() {
    colors.danger
  } else if state.k8s_available {
    colors.success
  } else {
    colors.muted_foreground
  };
  h_flex()
    .w_full()
    .min_h(px(48.))
    .items_center()
    .flex_shrink_0()
    .bg(colors.tab_bar)
    .border_b_1()
    .border_color(colors.border)
    .child(div().flex_1().min_w_0().overflow_hidden().child(left))
    .child(
      h_flex()
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .flex_shrink_0()
        .child(actions)
        .when(show_namespace, |el| el.child(render_namespace_selector(cx)))
        .child(div().size(px(8.)).rounded_full().bg(status).flex_shrink_0())
        .child(render_context_selector(cx)),
    )
}

/// A plain title block for non-tabbed k8s views (Clusters manager,
/// future single-resource screens), styled to sit in the shared header
/// exactly where a tab bar would.
pub fn k8s_header_title(title: impl Into<String>, subtitle: impl Into<String>, cx: &App) -> impl IntoElement {
  let colors = cx.theme().colors;
  let title = title.into();
  let subtitle = subtitle.into();
  div()
    .px(px(16.))
    .py(px(6.))
    .child(
      div()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(title),
    )
    .when(!subtitle.is_empty(), |el| {
      el.child(div().text_xs().text_color(colors.muted_foreground).child(subtitle))
    })
}
