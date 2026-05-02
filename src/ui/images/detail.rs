use gpui::{App, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable,
  button::{Button, ButtonVariants},
  h_flex,
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::docker::ImageInfo;
use crate::state::ImageInspectData;
use crate::ui::components::{render_error_panel, render_install_hint};

type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct ImageDetail {
  image: Option<ImageInfo>,
  inspect_data: Option<ImageInspectData>,
  active_tab: usize,
  on_tab_change: Option<TabChangeCallback>,
}

impl ImageDetail {
  pub fn new() -> Self {
    Self {
      image: None,
      inspect_data: None,
      active_tab: 0,
      on_tab_change: None,
    }
  }

  pub fn image(mut self, image: Option<ImageInfo>) -> Self {
    self.image = image;
    self
  }

  pub fn inspect_data(mut self, data: Option<ImageInspectData>) -> Self {
    self.inspect_data = data;
    self
  }

  pub fn active_tab(mut self, tab: usize) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&usize, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  fn render_empty(cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .items_center()
      .justify_center()
      .child(
        v_flex()
          .items_center()
          .gap(px(16.))
          .child(
            Icon::new(AppIcon::Image)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select an image to view details"),
          ),
      )
  }

  fn render_info_tab(&self, image: &ImageInfo, cx: &App) -> gpui::Div {
    let _colors = &cx.theme().colors;

    // Basic info rows
    let mut basic_info = vec![
      ("ID", image.short_id().to_string()),
      ("Tag", image.display_name()),
      ("Size", image.display_size()),
    ];

    if let Some(created) = image.created {
      basic_info.insert(2, ("Created", created.format("%Y-%m-%d %H:%M:%S").to_string()));
    }

    // Platform
    if let (Some(os), Some(arch)) = (&image.os, &image.architecture) {
      basic_info.push(("Platform", format!("{os}/{arch}")));
    }

    let mut content = v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .gap(px(12.))
      .child(Self::render_section(None, basic_info, cx));

    // Config section if we have inspect data
    if let Some(ref data) = self.inspect_data {
      let mut config_rows = Vec::new();

      if let Some(ref cmd) = data.config_cmd {
        config_rows.push(("Command", cmd.join(" ")));
      }

      if let Some(ref workdir) = data.config_workdir {
        config_rows.push(("Working Directory", workdir.clone()));
      }

      if let Some(ref entrypoint) = data.config_entrypoint {
        config_rows.push(("Entrypoint", entrypoint.join(" ")));
      }

      if !config_rows.is_empty() {
        content = content.child(Self::render_section(Some("Config"), config_rows, cx));
      }

      // Environment section
      if !data.config_env.is_empty() {
        content = content.child(Self::render_env_section(&data.config_env, cx));
      }

      // Exposed ports
      if !data.config_exposed_ports.is_empty() {
        let ports_str = data.config_exposed_ports.join(", ");
        content = content.child(Self::render_section(
          Some("Exposed Ports"),
          vec![("Ports", ports_str)],
          cx,
        ));
      }

      // Used by section
      if !data.used_by.is_empty() {
        content = content.child(Self::render_used_by_section(&data.used_by, cx));
      }
    }

    // Labels section if not empty
    if !image.labels.is_empty() {
      content = content.child(Self::render_labels_section(image, cx));
    }

    content
  }

  fn render_section(header: Option<&str>, rows: Vec<(&str, String)>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut section = v_flex().gap(px(1.));

    if let Some(title) = header {
      section = section.child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child(title.to_string()),
      );
    }

    let rows_container = v_flex()
      .bg(colors.background)
      .rounded(px(8.))
      .overflow_hidden()
      .children(
        rows
          .into_iter()
          .enumerate()
          .map(|(i, (label, value))| Self::render_section_row(label, value, i == 0, cx)),
      );

    section.child(rows_container)
  }

  fn render_section_row(label: &str, value: String, is_first: bool, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut row = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .items_center()
      .justify_between()
      .child(
        div()
          .text_sm()
          .text_color(colors.secondary_foreground)
          .child(label.to_string()),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.foreground)
          .max_w(px(250.))
          .overflow_hidden()
          .text_ellipsis()
          .child(value),
      );

    if !is_first {
      row = row.border_t_1().border_color(colors.border);
    }

    row
  }

  fn render_env_section(env: &[(String, String)], cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Environment"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Value"),
                            ),
                    )
                    // Env rows
                    .children(env.iter().enumerate().map(|(i, (key, value))| {
                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(key.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(value.clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_used_by_section(containers: &[String], cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Used By"),
      )
      .child(
        v_flex()
          .bg(colors.background)
          .rounded(px(8.))
          .overflow_hidden()
          .children(containers.iter().enumerate().map(|(i, name)| {
            let mut row = h_flex()
              .w_full()
              .px(px(16.))
              .py(px(10.))
              .items_center()
              .gap(px(8.))
              .child(Icon::new(AppIcon::Image).text_color(colors.secondary_foreground))
              .child(div().text_sm().text_color(colors.foreground).child(name.clone()));

            if i > 0 {
              row = row.border_t_1().border_color(colors.border);
            }
            row
          })),
      )
  }

  fn render_labels_section(image: &ImageInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut labels: Vec<_> = image.labels.iter().collect();
    labels.sort_by(|a, b| a.0.cmp(b.0));

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Labels"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Value"),
                            ),
                    )
                    // Label rows
                    .children(labels.iter().enumerate().map(|(i, (key, value))| {
                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*key).clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*value).clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_layers_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let history = self
      .inspect_data
      .as_ref()
      .map(|d| d.history.clone())
      .unwrap_or_default();

    if history.is_empty() {
      return v_flex().w_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No layer history available."),
      );
    }

    let header = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(8.))
      .gap(px(8.))
      .border_b_1()
      .border_color(colors.border)
      .bg(colors.muted)
      .child(
        div()
          .w(px(70.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("SIZE"),
      )
      .child(
        div()
          .w(px(140.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("CREATED"),
      )
      .child(
        div()
          .flex_1()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("COMMAND"),
      );

    let rows = history.into_iter().enumerate().map(|(i, entry)| {
      let created = entry
        .created
        .map(|c| c.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
      let zebra = if i % 2 == 0 {
        colors.background
      } else {
        colors.muted.opacity(0.4)
      };
      h_flex()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .gap(px(8.))
        .bg(zebra)
        .child(
          div()
            .w(px(70.))
            .text_xs()
            .text_color(colors.foreground)
            .child(entry.display_size()),
        )
        .child(
          div()
            .w(px(140.))
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(created),
        )
        .child(
          div()
            .flex_1()
            .text_xs()
            .font_family("monospace")
            .text_color(colors.foreground)
            .child(entry.short_command()),
        )
    });

    v_flex().w_full().child(header).children(rows)
  }

  fn render_vulns_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let data = self.inspect_data.as_ref();

    if data.is_none_or(|d| d.scan.is_none() && !d.scan_loading && d.scan_error.is_none()) {
      let image = self.image.clone();
      return v_flex()
        .w_full()
        .p(px(32.))
        .gap(px(16.))
        .items_center()
        .child(
          Icon::new(IconName::Eye)
            .size(px(40.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("No vulnerability scan yet for this image."),
        )
        .child(
          Button::new("vulns-empty-scan")
            .icon(Icon::new(IconName::Eye))
            .label("Scan with Trivy")
            .primary()
            .on_click(move |_ev, _window, cx| {
              if let Some(ref img) = image {
                let image_ref = img.repo_tags.first().cloned().unwrap_or_else(|| img.id.clone());
                crate::services::scan_image(img.id.clone(), image_ref, cx);
              }
            }),
        );
    }

    let d = data.unwrap();
    if d.scan_loading {
      return v_flex().w_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Running Trivy..."),
      );
    }
    if let Some(err) = d.scan_error.as_ref() {
      // Specially render the missing-binary case using an install-hint
      // panel modeled after the Setup dialog: headline + per-platform
      // copy-able command list + docs link. Anything else falls back to
      // the raw error message, also dressed up.
      if err == crate::docker::ERR_TRIVY_NOT_INSTALLED {
        return render_install_hint(&crate::docker::trivy_install_hint(), cx);
      }
      return render_error_panel("Scan failed", err, colors);
    }

    let summary = d.scan.as_ref().unwrap();
    let counts = h_flex()
      .gap(px(12.))
      .px(px(16.))
      .py(px(12.))
      .border_b_1()
      .border_color(colors.border)
      .child(severity_badge("CRITICAL", summary.critical, colors.danger, cx))
      .child(severity_badge("HIGH", summary.high, colors.warning, cx))
      .child(severity_badge("MEDIUM", summary.medium, colors.muted_foreground, cx))
      .child(severity_badge("LOW", summary.low, colors.muted_foreground, cx))
      .child(severity_badge("UNKNOWN", summary.unknown, colors.muted_foreground, cx));

    let header = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(8.))
      .gap(px(8.))
      .border_b_1()
      .border_color(colors.border)
      .bg(colors.muted)
      .child(
        div()
          .w(px(110.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("CVE"),
      )
      .child(
        div()
          .w(px(80.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("SEVERITY"),
      )
      .child(
        div()
          .w(px(160.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("PACKAGE"),
      )
      .child(
        div()
          .w(px(140.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("INSTALLED"),
      )
      .child(
        div()
          .w(px(140.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("FIXED"),
      )
      .child(
        div()
          .flex_1()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("TITLE"),
      );

    let rows = summary.vulns.iter().enumerate().take(500).map(|(i, v)| {
      let zebra = if i % 2 == 0 {
        colors.background
      } else {
        colors.muted.opacity(0.4)
      };
      let sev_color = match v.severity.as_str() {
        "CRITICAL" => colors.danger,
        "HIGH" => colors.warning,
        _ => colors.muted_foreground,
      };
      h_flex()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .gap(px(8.))
        .bg(zebra)
        .child(
          div()
            .w(px(110.))
            .text_xs()
            .font_family("monospace")
            .text_color(colors.foreground)
            .child(v.id.clone()),
        )
        .child(
          div()
            .w(px(80.))
            .text_xs()
            .text_color(sev_color)
            .child(v.severity.clone()),
        )
        .child(
          div()
            .w(px(160.))
            .text_xs()
            .font_family("monospace")
            .text_color(colors.foreground)
            .child(v.package.clone()),
        )
        .child(
          div()
            .w(px(140.))
            .text_xs()
            .font_family("monospace")
            .text_color(colors.muted_foreground)
            .child(v.installed_version.clone()),
        )
        .child(
          div()
            .w(px(140.))
            .text_xs()
            .font_family("monospace")
            .text_color(if v.fixed_version.is_some() {
              colors.success
            } else {
              colors.muted_foreground
            })
            .child(v.fixed_version.clone().unwrap_or_else(|| "—".to_string())),
        )
        .child(
          div()
            .flex_1()
            .text_xs()
            .text_color(colors.foreground)
            .child(v.title.clone()),
        )
    });

    v_flex().w_full().child(counts).child(header).children(rows)
  }

  pub fn render(self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(image) = &self.image else {
      return Self::render_empty(cx).into_any_element();
    };

    let on_tab_change = self.on_tab_change.clone();

    let tabs = ["Info", "Layers", "Vulnerabilities"];

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .items_center()
      .flex_shrink_0()
      .child(
        TabBar::new("image-tabs")
          .flex_1()
          .children(tabs.iter().enumerate().map(|(i, label)| {
            let on_tab_change = on_tab_change.clone();
            Tab::new()
              .label((*label).to_string())
              .selected(self.active_tab == i)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&i, window, cx);
                }
              })
          })),
      );

    // Content based on active tab
    let content = match self.active_tab {
      1 => self.render_layers_tab(cx),
      2 => self.render_vulns_tab(cx),
      _ => self.render_info_tab(image, cx),
    };

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar)
      .child(
        div()
          .id("image-detail-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      )
      .into_any_element()
  }
}

fn severity_badge(label: &'static str, count: usize, color: gpui::Hsla, cx: &App) -> gpui::Div {
  let colors = &cx.theme().colors;
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
