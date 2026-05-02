//! Loading and error state components for list views
//!
//! Displays loading indicators and error states while data is being fetched.

use gpui::{App, ClipboardItem, Div, Styled, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  theme::ActiveTheme,
  v_flex,
};

use super::spinning_loader;
use crate::kubernetes::{K8sStatus, kubeconfig_setup_hint, kubectl_install_hint};

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

/// Render an error state for a Kubernetes resource list. If the underlying
/// problem is missing `kubectl` or a missing kubeconfig we surface install
/// instructions instead of the raw error string.
pub fn render_k8s_error(
  resource_name: &str,
  error_message: &str,
  on_retry: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static,
  cx: &App,
) -> Div {
  match K8sStatus::diagnose(error_message) {
    K8sStatus::KubectlMissing => render_setup_panel(
      "Kubernetes Not Installed",
      "kubectl was not found on this system. Install it to manage Kubernetes resources.",
      kubectl_install_hint(),
      "install-kubectl",
      on_retry,
      cx,
    ),
    K8sStatus::KubeconfigMissing => render_setup_panel(
      "Kubernetes Not Configured",
      "kubectl is installed but no kubeconfig was found. Configure or start a cluster to continue.",
      kubeconfig_setup_hint(),
      "configure-k8s",
      on_retry,
      cx,
    ),
    K8sStatus::ClusterUnreachable(_) => render_error(resource_name, error_message, on_retry, cx),
  }
}

fn render_setup_panel(
  title: &'static str,
  description: &'static str,
  hint: (&'static str, &'static str),
  copy_id: &'static str,
  on_retry: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static,
  cx: &App,
) -> Div {
  let colors = &cx.theme().colors;
  let (command, command_description) = hint;
  let cmd_for_copy = command.to_string();

  v_flex()
    .flex_1()
    .w_full()
    .items_center()
    .justify_center()
    .gap(px(16.))
    .py(px(48.))
    .px(px(24.))
    .child(
      div()
        .size(px(64.))
        .rounded(px(12.))
        .bg(colors.warning.opacity(0.1))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(IconName::Info).size(px(32.)).text_color(colors.warning)),
    )
    .child(
      div()
        .text_xl()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.secondary_foreground)
        .child(title),
    )
    .child(
      div()
        .max_w(px(520.))
        .text_sm()
        .text_color(colors.muted_foreground)
        .text_center()
        .child(description),
    )
    .child(
      v_flex()
        .max_w(px(640.))
        .w_full()
        .gap(px(8.))
        .mt(px(8.))
        .child(
          h_flex()
            .w_full()
            .gap(px(8.))
            .items_center()
            .child(
              div()
                .flex_1()
                .px(px(10.))
                .py(px(8.))
                .bg(colors.background)
                .border_1()
                .border_color(colors.border)
                .rounded(px(4.))
                .font_family("monospace")
                .text_xs()
                .text_color(colors.foreground)
                .overflow_hidden()
                .child(command),
            )
            .child(
              Button::new(copy_id)
                .icon(IconName::Copy)
                .ghost()
                .xsmall()
                .on_click(move |_ev, _window, cx| {
                  cx.write_to_clipboard(ClipboardItem::new_string(cmd_for_copy.clone()));
                }),
            ),
        )
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(command_description),
        ),
    )
    .child(
      h_flex()
        .mt(px(16.))
        .child(Button::new("retry-k8s").label("Retry").primary().on_click(on_retry)),
    )
}
