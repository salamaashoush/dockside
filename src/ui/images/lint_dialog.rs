//! Hadolint result viewer.
//!
//! Plain non-interactive view that takes a `LintReport` and renders
//! severity badges + a per-finding table mirroring the Vulnerabilities
//! tab on the image detail panel.

use gpui::{App, Context, FocusHandle, Focusable, Hsla, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::docker::LintReport;

pub struct LintReportDialog {
  focus_handle: FocusHandle,
  dockerfile: String,
  report: LintReport,
}

impl LintReportDialog {
  pub fn new(dockerfile: String, report: LintReport, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    Self {
      focus_handle,
      dockerfile,
      report,
    }
  }
}

impl Focusable for LintReportDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

fn level_color(level: &str, colors: &gpui_component::theme::ThemeColor) -> Hsla {
  match level {
    "error" => colors.danger,
    "warning" => colors.warning,
    "info" => colors.link,
    _ => colors.muted_foreground,
  }
}

fn severity_badge(label: &'static str, count: usize, color: Hsla, colors: &gpui_component::theme::ThemeColor) -> gpui::Div {
  v_flex()
    .px(px(10.))
    .py(px(6.))
    .gap(px(2.))
    .rounded(px(6.))
    .bg(if count > 0 { color.opacity(0.15) } else { colors.muted })
    .child(
      div()
        .text_xs()
        .text_color(if count > 0 { color } else { colors.muted_foreground })
        .child(label),
    )
    .child(
      div()
        .text_lg()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(count.to_string()),
    )
}

impl Render for LintReportDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let report = &self.report;

    let counts = h_flex()
      .gap(px(12.))
      .px(px(16.))
      .py(px(12.))
      .border_b_1()
      .border_color(colors.border)
      .child(severity_badge("ERROR", report.error, colors.danger, colors))
      .child(severity_badge("WARNING", report.warning, colors.warning, colors))
      .child(severity_badge("INFO", report.info, colors.link, colors))
      .child(severity_badge("STYLE", report.style, colors.muted_foreground, colors));

    let header = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(8.))
      .gap(px(8.))
      .border_b_1()
      .border_color(colors.border)
      .bg(colors.muted)
      .child(div().w(px(60.)).text_xs().text_color(colors.muted_foreground).child("LINE"))
      .child(div().w(px(80.)).text_xs().text_color(colors.muted_foreground).child("LEVEL"))
      .child(div().w(px(80.)).text_xs().text_color(colors.muted_foreground).child("RULE"))
      .child(div().flex_1().text_xs().text_color(colors.muted_foreground).child("MESSAGE"))
      .child(div().w(px(40.)).text_xs().text_color(colors.muted_foreground).child(""));

    let rows = report.findings.iter().enumerate().map(|(i, f)| {
      let zebra = if i % 2 == 0 {
        colors.background
      } else {
        colors.muted.opacity(0.4)
      };
      let lvl = level_color(&f.level, colors);
      let copy_text = format!("{}: {} (line {}) {}", f.code, f.level, f.line, f.message);
      h_flex()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .gap(px(8.))
        .bg(zebra)
        .child(
          div()
            .w(px(60.))
            .text_xs()
            .font_family("monospace")
            .text_color(colors.foreground)
            .child(f.line.to_string()),
        )
        .child(
          div()
            .w(px(80.))
            .text_xs()
            .text_color(lvl)
            .child(f.level.clone()),
        )
        .child(
          div()
            .w(px(80.))
            .text_xs()
            .font_family("monospace")
            .text_color(colors.foreground)
            .child(f.code.clone()),
        )
        .child(
          div()
            .flex_1()
            .text_xs()
            .text_color(colors.foreground)
            .child(f.message.clone()),
        )
        .child(
          Button::new(("copy-finding", i))
            .icon(Icon::new(AppIcon::Copy))
            .ghost()
            .small()
            .on_click(move |_ev, _window, cx| {
              cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_text.clone()));
            }),
        )
    });

    let body = if report.findings.is_empty() {
      v_flex()
        .w_full()
        .p(px(24.))
        .items_center()
        .child(
          div()
            .text_sm()
            .text_color(colors.success)
            .child(format!("No issues found in {}", self.dockerfile)),
        )
    } else {
      v_flex().w_full().child(counts).child(header).children(rows)
    };

    v_flex()
      .w_full()
      .max_h(px(560.))
      .overflow_y_scrollbar()
      .child(
        div()
          .px(px(16.))
          .py(px(8.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(format!("Hadolint report for {}", self.dockerfile)),
      )
      .child(body)
  }
}
