//! Read-only "Add Node" dialog: detects the distro and shows
//! copy-pasteable join commands / console links. Never executes anything
//! remotely (roadmap phase 4a).

use gpui::{App, ClipboardItem, Context, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  IconName, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use crate::kubernetes::{JoinGuide, join_guide};
use crate::state::docker_state;

pub struct AddNodeDialog {
  focus_handle: FocusHandle,
  guide: JoinGuide,
}

impl AddNodeDialog {
  fn new(guide: JoinGuide, cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      guide,
    }
  }
}

impl Focusable for AddNodeDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for AddNodeDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let g = &self.guide;

    let mut root = v_flex().w_full().px(px(16.)).py(px(12.)).gap(px(12.)).child(
      h_flex()
        .gap(px(8.))
        .items_center()
        .child(
          div()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(colors.primary.opacity(0.15))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.primary)
            .child(g.distro.label()),
        )
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(g.title.clone()),
        ),
    );

    let steps = v_flex().w_full().gap(px(6.)).children(
      g.steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
          h_flex()
            .w_full()
            .gap(px(8.))
            .items_start()
            .child(
              div()
                .flex_shrink_0()
                .size(px(18.))
                .rounded_full()
                .bg(colors.sidebar)
                .text_xs()
                .text_color(colors.muted_foreground)
                .flex()
                .items_center()
                .justify_center()
                .child(format!("{}", i + 1)),
            )
            .child(div().flex_1().text_sm().text_color(colors.foreground).child(s.clone()))
        })
        .collect::<Vec<_>>(),
    );
    if g.distro.is_managed() {
      root = root.child(
        div()
          .w_full()
          .p(px(8.))
          .rounded(px(6.))
          .bg(colors.warning.opacity(0.12))
          .text_xs()
          .text_color(colors.warning)
          .child("Managed cluster — node count is controlled by your cloud provider, not a join token."),
      );
    }

    root = root.child(steps);

    if let Some(cmd) = g.command.clone() {
      let cmd_for_copy = cmd.clone();
      root = root.child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .p(px(10.))
              .rounded(px(6.))
              .bg(colors.sidebar)
              .font_family("monospace")
              .text_xs()
              .text_color(colors.foreground)
              .child(cmd.clone()),
          )
          .child(
            Button::new("copy-join-cmd")
              .icon(IconName::Copy)
              .ghost()
              .small()
              .on_click(move |_e, _w, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(cmd_for_copy.clone()));
              }),
          ),
      );
    }

    if let Some(url) = g.console_url.clone() {
      root = root.child(
        Button::new("open-console")
          .label("Open cloud console")
          .icon(IconName::Globe)
          .outline()
          .small()
          .on_click(move |_e, _w, cx| {
            cx.open_url(&url);
          }),
      );
    }

    v_flex()
      .w_full()
      .max_h(px(520.))
      .child(div().w_full().flex_1().overflow_y_scrollbar().child(root))
  }
}

/// Open the Add Node dialog for the active cluster.
pub fn open_add_node_dialog(window: &mut Window, cx: &mut App) {
  let (nodes, server) = {
    let state_entity = docker_state(cx);
    let state = state_entity.read(cx);
    let current = state.current_kube_context_name();
    let server = state
      .kube_contexts
      .iter()
      .find(|c| Some(c.name.as_str()) == current.as_deref() || c.is_current)
      .map(|c| c.server.clone())
      .unwrap_or_default();
    (state.nodes.clone(), server)
  };
  let guide = join_guide(&nodes, &server);
  let entity = cx.new(|cx| AddNodeDialog::new(guide, cx));

  window.open_dialog(cx, move |dialog, _w, _cx| {
    dialog
      .title("Add Node")
      .min_w(px(560.))
      .child(entity.clone())
      .footer(move |_st, _, _w, _cx| {
        vec![
          Button::new("close-add-node")
            .label("Close")
            .on_click(move |_e, window, cx| {
              window.close_dialog(cx);
            })
            .into_any_element(),
        ]
      })
  });
}
