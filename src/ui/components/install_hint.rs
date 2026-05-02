//! Shared "install missing tool" + generic error panel widgets.
//!
//! Used by image vulnerability scan failures (Trivy) and Dockerfile
//! lint failures (Hadolint) so the same structured warning card +
//! per-platform copy-able commands render everywhere a CLI binary may
//! be missing.

use gpui::{App, ParentElement, Styled, px};
use gpui_component::{
  Icon, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  theme::{ActiveTheme, ThemeColor},
  v_flex,
};

use crate::assets::AppIcon;
use crate::docker::InstallHint;

fn render_command_row(id: usize, command: &str, cx: &App) -> gpui::Div {
  let colors = &cx.theme().colors;
  let cmd = command.to_string();
  let cmd_clone = cmd.clone();
  h_flex()
    .w_full()
    .gap(px(8.))
    .items_center()
    .child(
      gpui::div()
        .flex_1()
        .px(px(10.))
        .py(px(6.))
        .bg(colors.background)
        .rounded(px(4.))
        .border_1()
        .border_color(colors.border)
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .overflow_hidden()
        .child(cmd),
    )
    .child(
      Button::new(("copy-cmd", id))
        .icon(Icon::new(AppIcon::Copy))
        .ghost()
        .small()
        .on_click(move |_ev, _window, cx| {
          cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd_clone.clone()));
        }),
    )
}

/// Structured install-hint panel: warning-style box, headline,
/// platform-appropriate commands (each with a Copy button), and a docs
/// link the user can click.
pub fn render_install_hint(hint: &InstallHint, cx: &App) -> gpui::Div {
  let colors = &cx.theme().colors;
  let mut card = v_flex()
    .w_full()
    .max_w(px(640.))
    .p(px(16.))
    .gap(px(12.))
    .rounded(px(8.))
    .border_1()
    .border_color(colors.warning.opacity(0.5))
    .bg(colors.warning.opacity(0.05))
    .child(
      h_flex()
        .gap(px(8.))
        .items_center()
        .child(
          Icon::new(gpui_component::IconName::Info)
            .size(px(16.))
            .text_color(colors.warning),
        )
        .child(
          gpui::div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(hint.headline.to_string()),
        ),
    );

  if !hint.commands.is_empty() {
    card = card.child(
      gpui::div()
        .text_xs()
        .text_color(colors.muted_foreground)
        .child("Run one of these:"),
    );
    for (i, c) in hint.commands.iter().enumerate() {
      card = card.child(render_command_row(i, c, cx));
    }
  }
  let docs = hint.docs_url.to_string();
  card = card.child(
    h_flex()
      .gap(px(8.))
      .items_center()
      .child(
        gpui::div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Docs:"),
      )
      .child(
        gpui::div()
          .text_xs()
          .font_family("monospace")
          .text_color(colors.link)
          .child(docs.clone()),
      )
      .child(
        Button::new("copy-docs")
          .icon(Icon::new(AppIcon::Copy))
          .ghost()
          .small()
          .on_click(move |_ev, _window, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(docs.clone()));
          }),
      ),
  );
  v_flex().w_full().p(px(24.)).items_center().child(card)
}

/// Generic dressed-up error panel for non-install failures (e.g. trivy
/// returning a non-zero exit code on a junk image).
pub fn render_error_panel(headline: &str, err: &str, colors: &ThemeColor) -> gpui::Div {
  let err_owned = err.to_string();
  let err_clone = err_owned.clone();
  let card = v_flex()
    .w_full()
    .max_w(px(640.))
    .p(px(16.))
    .gap(px(8.))
    .rounded(px(8.))
    .border_1()
    .border_color(colors.danger.opacity(0.5))
    .bg(colors.danger.opacity(0.05))
    .child(
      h_flex()
        .gap(px(8.))
        .items_center()
        .child(
          Icon::new(gpui_component::IconName::CircleX)
            .size(px(16.))
            .text_color(colors.danger),
        )
        .child(
          gpui::div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(headline.to_string()),
        ),
    )
    .child(
      gpui::div()
        .text_xs()
        .font_family("monospace")
        .text_color(colors.muted_foreground)
        .child(err_owned),
    )
    .child(
      Button::new("copy-err")
        .icon(Icon::new(AppIcon::Copy))
        .ghost()
        .small()
        .label("Copy error")
        .on_click(move |_ev, _window, cx| {
          cx.write_to_clipboard(gpui::ClipboardItem::new_string(err_clone.clone()));
        }),
    );
  v_flex().w_full().p(px(24.)).items_center().child(card)
}
